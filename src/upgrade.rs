use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::time::Instant;

use inquire::InquireError;

use crate::UpgradeArgs;
use crate::check;
use crate::config::{self, Config};
use crate::progress::Progress;
use crate::run::{self, Action, EXIT_CANCELLED, Engine, Event, Options, choose};

pub fn run(args: &UpgradeArgs, config: &Config) -> Result<ExitCode, String> {
    let started = Instant::now();
    let interactive =
        !args.yes && !args.json && io::stdin().is_terminal() && io::stdout().is_terminal();
    if !args.yes && !interactive && !args.dry_run {
        return Err("not running in a terminal, pass --yes to upgrade without asking".into());
    }
    let _lock = match run::lock(args.dry_run) {
        Ok(lock) => lock,
        Err(busy) => return Ok(busy),
    };
    let progress = Progress::new(args.json);
    let mut engine = Engine::new(
        Options {
            action: Action::Upgrade,
            json: args.json,
            dry_run: args.dry_run,
            no_sudo: args.no_sudo,
            interactive,
        },
        &progress,
    );

    let mut checks_failed = false;
    let mut pending = Vec::new();
    for (found, result) in check::gather(&args.selection, config, &progress)? {
        if let Some(error) = &result.error {
            checks_failed = true;
            engine.report.emit(&Event::CheckFailed {
                manager: result.name,
                error,
            });
        } else {
            // `held` matches `name` or `manager:name`, the same forms --package takes.
            let packages: Vec<_> = result
                .packages
                .into_iter()
                .filter(|p| {
                    args.packages.is_empty() || config::held(&args.packages, result.id, &p.name)
                })
                .collect();
            if !packages.is_empty() {
                pending.push((found, packages));
            }
        }
    }

    if interactive && !pending.is_empty() {
        let (nw, cw) =
            pending
                .iter()
                .flat_map(|(_, packages)| packages)
                .fold((0, 0), |(n, c), p| {
                    (
                        n.max(p.name.chars().count()),
                        c.max(p.current.chars().count()),
                    )
                });
        let label = |p: &crate::manager::Package| {
            format!("{:<nw$}  {:>cw$} → {}", p.name, p.current, p.latest)
        };
        pending = match choose("What should be upgraded?", pending, true, label) {
            Ok(picked) => picked,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                println!("Cancelled, nothing upgraded.");
                return Ok(ExitCode::from(EXIT_CANCELLED));
            }
            Err(e) => return Err(e.to_string()),
        };
    }

    engine
        .report
        .fit(pending.iter().flat_map(|(_, packages)| packages));
    for (found, mut packages) in pending {
        let held = found.manager.incompatible(&found.path, &packages);
        packages.retain(|p| !held.iter().any(|(h, _)| h == p));
        if !packages.is_empty() {
            engine.run_manager(&found, &packages, |found, packages| {
                found.manager.upgrade_steps(&found.path, packages)
            });
        }
        engine.held_back(found.manager, &held);
    }
    Ok(engine.finish(started, checks_failed))
}
