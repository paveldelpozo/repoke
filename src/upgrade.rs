use std::fmt;
use std::io::{self, BufRead, BufReader, IsTerminal, Read};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::thread;

use indicatif::ProgressBar;
use inquire::{InquireError, MultiSelect};
use serde::Serialize;

use crate::UpgradeArgs;
use crate::check::{self, Found};
use crate::config::Config;
use crate::manager::Package;
use crate::progress::Progress;

const EXIT_CANCELLED: u8 = 130;

/// With `--json` each event is printed as one line, so an agent can follow progress as it happens.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum Event<'a> {
    CheckFailed {
        manager: &'a str,
        error: &'a str,
    },
    Running {
        manager: &'a str,
        command: &'a str,
        packages: &'a [Package],
    },
    Planned {
        manager: &'a str,
        command: &'a str,
        packages: &'a [Package],
    },
    Upgraded {
        manager: &'a str,
        packages: &'a [Package],
    },
    Failed {
        manager: &'a str,
        error: &'a str,
        packages: &'a [Package],
    },
    Skipped {
        manager: &'a str,
        reason: &'a str,
        packages: &'a [Package],
    },
    Summary(Tally),
}

#[derive(Default, Serialize)]
struct Tally {
    upgraded: usize,
    failed: usize,
    skipped: usize,
    planned: usize,
}

pub fn run(args: &UpgradeArgs, config: &Config) -> Result<ExitCode, String> {
    let interactive =
        !args.yes && !args.json && io::stdin().is_terminal() && io::stdout().is_terminal();
    if !args.yes && !interactive && !args.dry_run {
        return Err("not running in a terminal, pass --yes to upgrade without asking".into());
    }
    let progress = Progress::new(args.json);

    let mut checks_failed = false;
    let mut pending = Vec::new();
    for (found, result) in check::gather(&args.selection, config, &progress)? {
        if let Some(error) = &result.error {
            checks_failed = true;
            emit(
                args.json,
                &Event::CheckFailed {
                    manager: result.id,
                    error,
                },
            );
        } else if !result.packages.is_empty() {
            pending.push((found, result.packages));
        }
    }

    if interactive && !pending.is_empty() {
        pending = match choose(pending) {
            Ok(picked) => picked,
            Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                println!("Cancelled, nothing upgraded.");
                return Ok(ExitCode::from(EXIT_CANCELLED));
            }
            Err(e) => return Err(e.to_string()),
        };
    }

    let mut tally = Tally::default();
    for (found, packages) in &pending {
        let m = found.manager;
        let manager = m.id();
        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        let path = found.path.display().to_string();
        let mut command = vec![path.clone()];
        command.extend(m.upgrade_args(&names));

        let root = m.needs_root(&found.path);
        if root {
            if args.no_sudo {
                tally.skipped += packages.len();
                emit(
                    args.json,
                    &Event::Skipped {
                        manager,
                        reason: "needs root and --no-sudo was given",
                        packages,
                    },
                );
                continue;
            }
            // -n never prompts: interactive runs ask for the password up front with `sudo -v`.
            command.splice(0..0, ["sudo".to_string(), "-n".to_string()]);
        }
        let shown = command.join(" ").replacen(&path, m.binary(), 1);

        if args.dry_run {
            tally.planned += packages.len();
            emit(
                args.json,
                &Event::Planned {
                    manager,
                    command: &shown,
                    packages,
                },
            );
            continue;
        }

        emit(
            args.json,
            &Event::Running {
                manager,
                command: &shown,
                packages,
            },
        );
        let outcome = if root && interactive {
            authenticate(&progress)
        } else {
            Ok(())
        }
        .and_then(|()| {
            let bar = progress.spinner(manager, &format!("upgrading {}", names.join(", ")));
            let outcome = execute(&command, &bar);
            bar.finish_and_clear();
            outcome
        });
        match outcome {
            Ok(()) => {
                tally.upgraded += packages.len();
                emit(args.json, &Event::Upgraded { manager, packages });
            }
            Err(error) => {
                tally.failed += packages.len();
                emit(
                    args.json,
                    &Event::Failed {
                        manager,
                        error: &error,
                        packages,
                    },
                );
            }
        }
    }

    let failed = checks_failed || tally.failed > 0;
    emit(args.json, &Event::Summary(tally));
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn authenticate(progress: &Progress) -> Result<(), String> {
    let status = progress
        .suspend(|| Command::new("sudo").arg("-v").status())
        .map_err(|e| format!("could not run sudo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("sudo authentication failed".into())
    }
}

/// The manager's output is streamed into the spinner so a long install shows what it is doing.
fn execute(command: &[String], bar: &ProgressBar) -> Result<(), String> {
    let (program, args) = command.split_first().expect("command has a program");
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {program}: {e}"))?;

    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().expect("stdout is piped")),
        Box::new(child.stderr.take().expect("stderr is piped")),
    ];
    let lines = Mutex::new(Vec::new());
    let shared = &lines;
    thread::scope(|s| {
        for pipe in pipes {
            s.spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    let line = line.trim();
                    if !line.is_empty() {
                        bar.set_message(line.to_string());
                        shared.lock().expect("output lock").push(line.to_string());
                    }
                }
            });
        }
    });

    let status = child
        .wait()
        .map_err(|e| format!("could not wait for {program}: {e}"))?;
    if status.success() {
        return Ok(());
    }
    let lines = lines.into_inner().expect("output lock");
    Err(format!(
        "exited with {}: {}",
        status
            .code()
            .map_or("a signal".to_string(), |c| c.to_string()),
        lines[lines.len().saturating_sub(20)..].join("\n")
    ))
}

struct Choice {
    manager: usize,
    package: Package,
    label: String,
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn choose(pending: Vec<(Found, Vec<Package>)>) -> Result<Vec<(Found, Vec<Package>)>, InquireError> {
    let width = pending
        .iter()
        .flat_map(|(_, packages)| packages)
        .map(|p| p.name.len())
        .max()
        .unwrap_or(0);
    let options: Vec<Choice> = pending
        .iter()
        .enumerate()
        .flat_map(|(i, (found, packages))| {
            packages.iter().map(move |p| Choice {
                manager: i,
                label: format!(
                    "{:<5} {:<width$}  {} → {}",
                    found.manager.id(),
                    p.name,
                    p.current,
                    p.latest
                ),
                package: p.clone(),
            })
        })
        .collect();

    let picked = MultiSelect::new("What should be upgraded?", options)
        .with_all_selected_by_default()
        .with_page_size(20)
        .prompt()?;

    Ok(pending
        .into_iter()
        .enumerate()
        .filter_map(|(i, (found, _))| {
            let packages: Vec<Package> = picked
                .iter()
                .filter(|c| c.manager == i)
                .map(|c| c.package.clone())
                .collect();
            (!packages.is_empty()).then_some((found, packages))
        })
        .collect())
}

fn emit(json: bool, event: &Event) {
    if json {
        println!(
            "{}",
            serde_json::to_string(event).expect("serializable event")
        );
        return;
    }
    match event {
        Event::CheckFailed { manager, error } => eprintln!("{manager}: check failed, {error}"),
        Event::Running { command, .. } => println!("==> {command}"),
        Event::Planned { command, .. } => println!("would run: {command}"),
        Event::Upgraded { manager, packages } => {
            println!("{manager}: upgraded {}", names(packages))
        }
        Event::Failed { manager, error, .. } => eprintln!("{manager}: upgrade failed, {error}"),
        Event::Skipped {
            manager, reason, ..
        } => println!("{manager}: skipped, {reason}"),
        Event::Summary(tally) => {
            let parts: Vec<String> = [
                (tally.upgraded, "upgraded"),
                (tally.failed, "failed"),
                (tally.skipped, "skipped"),
                (tally.planned, "would be upgraded"),
            ]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, label)| format!("{n} {label}"))
            .collect();
            if parts.is_empty() {
                println!("Nothing to upgrade.");
            } else {
                println!("Packages: {}.", parts.join(", "));
            }
        }
    }
}

fn names(packages: &[Package]) -> String {
    packages
        .iter()
        .map(|p| p.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Vec<String> {
        ["sh", "-c", script].map(String::from).to_vec()
    }

    #[test]
    fn execute_reports_exit_code_and_output_tail() {
        let error = execute(
            &sh("echo working; echo broken >&2; exit 3"),
            &ProgressBar::hidden(),
        )
        .unwrap_err();
        assert!(error.starts_with("exited with 3:"), "{error}");
        assert!(
            error.contains("working") && error.contains("broken"),
            "{error}"
        );
    }

    #[test]
    fn execute_succeeds_on_zero_exit() {
        assert!(execute(&sh("echo done"), &ProgressBar::hidden()).is_ok());
    }
}
