use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, ErrorKind, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use console::style;
use inquire::{InquireError, MultiSelect, Select};
use serde::Serialize;

use crate::InitArgs;
use crate::catalog::{self, Tool};
use crate::check::{self, Found};
use crate::config::Config;
use crate::manager::{self, Installed, Manager, Package};
use crate::managers;
use crate::progress::Progress;
use crate::run::{Action, EXIT_CANCELLED, Engine, Options, choose, plural};
use crate::snapshot::{self, ConfigFile, Entry, FILES, Snapshot};

/// Managers that Homebrew can install when a snapshot needs them, and the formula for each.
const VIA_BREW: [(&str, &str); 7] = [
    ("fnm", "fnm"),
    ("pnpm", "pnpm"),
    ("uv", "uv"),
    ("mas", "mas"),
    ("mise", "mise"),
    ("composer", "composer"),
    ("gem", "ruby"),
];

fn hint(id: &str) -> &'static str {
    match id {
        "brew" => "install Homebrew from https://brew.sh",
        "rustup" | "cargo" => "install Rust from https://rustup.rs",
        "nvm" => "install nvm from https://github.com/nvm-sh/nvm",
        "npm" => "install Node.js first, for example with fnm",
        "pip" => "install Python first",
        "scoop" => "install Scoop from https://scoop.sh",
        "choco" => "install Chocolatey from https://chocolatey.org/install",
        _ => "install it with your system's package manager",
    }
}

#[derive(Clone)]
struct Pick {
    installed: Installed,
    /// The manager isn't installed yet and will be set up first.
    pending_manager: bool,
}

pub fn run(args: &InitArgs, config: &Config) -> Result<ExitCode, String> {
    let started = Instant::now();
    let interactive =
        !args.yes && !args.json && io::stdin().is_terminal() && io::stdout().is_terminal();
    if !args.yes && !interactive && !args.dry_run {
        return Err("not running in a terminal, pass --yes to install without asking".into());
    }
    let _lock = match crate::run::lock(args.dry_run) {
        Ok(lock) => lock,
        Err(busy) => return Ok(busy),
    };
    let progress = Progress::new(args.json);
    let engine = Engine::new(
        Options {
            action: Action::Install,
            json: args.json,
            dry_run: args.dry_run,
            no_sudo: args.no_sudo,
            interactive,
        },
        &progress,
    );
    let outcome = match &args.from {
        Some(dir) => from_snapshot(args, config, dir, interactive, &progress, engine, started),
        None => from_catalog(args, config, interactive, &progress, engine, started),
    };
    match outcome {
        Err(Cancel) => {
            println!("Cancelled, nothing installed.");
            Ok(ExitCode::from(EXIT_CANCELLED))
        }
        Ok(result) => result,
    }
}

struct Cancel;

type Outcome = Result<Result<ExitCode, String>, Cancel>;

fn prompt<T>(result: Result<T, InquireError>) -> Result<Result<T, String>, Cancel> {
    match result {
        Ok(value) => Ok(Ok(value)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Err(Cancel),
        Err(e) => Ok(Err(e.to_string())),
    }
}

fn from_snapshot(
    args: &InitArgs,
    config: &Config,
    dir: &Path,
    interactive: bool,
    progress: &Progress,
    mut engine: Engine,
    started: Instant,
) -> Outcome {
    let snap = match Snapshot::read(dir) {
        Ok(snap) => snap,
        Err(e) => return Ok(Err(e)),
    };
    let selected = match check::select(&args.managers, config) {
        Ok(selected) => selected,
        Err(e) => return Ok(Err(e)),
    };
    let here = managers::all();

    // Snapshot entries in `repoke list` order, so Homebrew runs before the managers it installs.
    let mut entries: Vec<(&'static dyn Manager, &Entry)> = Vec::new();
    let mut unavailable = Vec::new();
    for (id, entry) in &snap.managers {
        match selected.iter().find(|m| m.id() == id) {
            Some(m) => entries.push((*m, entry)),
            None if !here.iter().any(|m| m.id() == id) => unavailable.push(id.as_str()),
            None => {}
        }
    }
    let order = |m: &dyn Manager| here.iter().position(|h| h.id() == m.id());
    entries.sort_by_key(|(m, _)| order(*m));

    if !args.json {
        let host = snap.host.as_deref().unwrap_or("another machine");
        println!(
            "Snapshot of {host} ({}), saved {}.",
            snap.os,
            snapshot::date(snap.created)
        );
        if !unavailable.is_empty() {
            println!(
                "{}",
                style(format!(
                    "Not available on {}, skipped: {}.",
                    env::consts::OS,
                    unavailable.join(", ")
                ))
                .yellow()
            );
        }
    }

    let installed = check::parallel(entries.iter().copied(), |(m, entry)| {
        let path = m.locate();
        let have: HashSet<Key> = path
            .as_deref()
            .map(|p| {
                let bar = progress.spinner(m.id(), "checking what is already installed");
                let listed = m.installed(p).unwrap_or_default();
                bar.finish_and_clear();
                listed.iter().map(key).collect()
            })
            .unwrap_or_default();
        let missing: Vec<Pick> = entry
            .packages
            .iter()
            .filter(|p| !have.contains(&key(p)))
            .map(|p| Pick {
                installed: p.clone(),
                pending_manager: path.is_none(),
            })
            .collect();
        let found = Found {
            manager: m,
            path: path.unwrap_or_else(|| PathBuf::from(m.binary())),
        };
        (found, missing)
    });
    let mut pending: Vec<(Found, Vec<Pick>)> = installed
        .into_iter()
        .filter(|(_, picks)| !picks.is_empty())
        .collect();

    if interactive && !pending.is_empty() {
        let width = pending
            .iter()
            .flat_map(|(_, picks)| picks)
            .map(|p| p.installed.name.chars().count())
            .max()
            .unwrap_or(0);
        let label = |p: &Pick| {
            let mut text = format!("{:<width$}", p.installed.name);
            if let Some(version) = &p.installed.version {
                text.push_str(&format!("  {version}"));
            }
            if let Some(kind) = p.installed.kind.as_deref().filter(|k| *k != "default") {
                text.push_str(&format!("  ({kind})"));
            }
            if p.pending_manager {
                text.push_str("  (manager not installed yet)");
            }
            text
        };
        pending = match prompt(choose("What should be installed?", pending, true, label))? {
            Ok(picked) => picked,
            Err(e) => return Ok(Err(e)),
        };
    }

    let packages = |picks: &[Pick]| -> Vec<Package> {
        picks.iter().map(|p| Package::from(&p.installed)).collect()
    };
    let as_installed =
        |picks: &[Pick]| -> Vec<Installed> { picks.iter().map(|p| p.installed.clone()).collect() };
    engine.report.fit(
        pending
            .iter()
            .flat_map(|(_, picks)| packages(picks))
            .collect::<Vec<_>>()
            .iter(),
    );

    for (found, picks) in &pending {
        let m = found.manager;
        let mut found = Found {
            manager: m,
            path: found.path.clone(),
        };
        if picks.iter().any(|p| p.pending_manager) {
            match set_up(m, args, interactive, &mut engine) {
                Ok(Some(path)) => found.path = path,
                Ok(None) if args.dry_run => {}
                Ok(None) => {
                    let reason = format!("{} is not installed: {}", m.name(), hint(m.id()));
                    engine.skip(m, &packages(picks), &reason);
                    continue;
                }
                Err(cancel) => return Err(cancel),
            }
        }
        let held = m.incompatible(&found.path, &packages(picks));
        let picks: Vec<Pick> = picks
            .iter()
            .filter(|p| !held.iter().any(|(h, _)| *h == Package::from(&p.installed)))
            .cloned()
            .collect();
        if !picks.is_empty() {
            engine.run_manager(&found, &picks, |found, picks| {
                found
                    .manager
                    .install_steps(&found.path, &as_installed(picks))
            });
        }
        engine.held_back(m, &held);
    }

    let configs: Vec<&ConfigFile> = if args.no_config {
        Vec::new()
    } else {
        entries
            .iter()
            .flat_map(|(_, entry)| &entry.config)
            .collect()
    };
    let configs_failed = match restore_configs(dir, &configs, args, interactive)? {
        Ok(failed) => failed,
        Err(e) => return Ok(Err(e)),
    };
    Ok(Ok(engine.finish(started, configs_failed)))
}

type Key = (String, Option<String>);

/// Package identity for "already installed": the default Node line or toolchain is not a
/// different package.
fn key(p: &Installed) -> Key {
    (p.name.clone(), p.kind.clone().filter(|k| k != "default"))
}

/// Installs a missing manager with Homebrew when possible. `Ok(None)` means it is still missing.
fn set_up(
    m: &'static dyn Manager,
    args: &InitArgs,
    interactive: bool,
    engine: &mut Engine,
) -> Result<Option<PathBuf>, Cancel> {
    if let Some(path) = m.locate() {
        return Ok(Some(path));
    }
    let formula = VIA_BREW
        .iter()
        .find(|(id, _)| *id == m.id())
        .map(|(_, f)| *f);
    let brew = managers::all()
        .into_iter()
        .find(|b| b.id() == "brew")
        .and_then(|b| b.locate().map(|path| Found { manager: b, path }));
    let (Some(formula), Some(brew)) = (formula, brew) else {
        return Ok(None);
    };
    let wanted = if interactive && !args.dry_run {
        prompt(
            inquire::Confirm::new(&format!(
                "{} is not installed. Install it with Homebrew (brew install {formula})?",
                m.name()
            ))
            .with_default(true)
            .prompt(),
        )?
        .unwrap_or_default()
    } else {
        true
    };
    if wanted {
        let formula = [Installed::new(formula)];
        engine.run_manager(&brew, &formula, |found, items| {
            found.manager.install_steps(&found.path, items)
        });
    }
    Ok(m.locate())
}

#[derive(Serialize)]
struct ConfigEvent<'a> {
    event: &'static str,
    path: &'a str,
    result: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// Returns whether any file could not be restored.
fn restore_configs(
    dir: &Path,
    files: &[&ConfigFile],
    args: &InitArgs,
    interactive: bool,
) -> Result<Result<bool, String>, Cancel> {
    if files.is_empty() {
        return Ok(Ok(false));
    }
    let Some(home) = snapshot::home() else {
        return Ok(Err("cannot find the home directory".into()));
    };
    if !args.json {
        println!("\n{}", style("Config files").bold());
    }
    let mut failed = false;
    for file in files {
        let target = home.join(&file.path);
        let shown = snapshot::tilde(&target, &home);
        let (result, detail) = match restore(dir, file, &target, &shown, args, interactive)? {
            Ok(outcome) => outcome,
            Err(e) => {
                failed = true;
                ("failed", Some(e))
            }
        };
        let placeholders = file.redacted
            && fs::read_to_string(&target).map_or(true, |text| text.contains(snapshot::REDACTED));
        let note = placeholders
            .then_some("values that looked like secrets were removed when exporting, fill them in");
        if args.json {
            let event = ConfigEvent {
                event: "config",
                path: &shown,
                result,
                detail: detail.or(note.map(String::from)),
            };
            println!(
                "{}",
                serde_json::to_string(&event).expect("serializable event")
            );
            continue;
        }
        let mark = match result {
            "restored" | "replaced" | "would_restore" | "would_replace" => style("✓").green(),
            "same" => style("=").dim(),
            "failed" => style("✗").red(),
            _ => style("-").yellow(),
        };
        let text = match result {
            "restored" => "restored",
            "replaced" => "replaced",
            "same" => "already the same",
            "kept" => "kept yours, it differs from the snapshot",
            "would_restore" => "would be restored",
            "would_replace" => "differs, would be replaced",
            "dotfiles" => "comes from your dotfiles",
            _ => "not restored",
        };
        println!("  {mark} {shown}  {text}");
        if let Some(detail) = detail {
            println!("      {}", style(detail).dim());
        }
        if let Some(note) = note.filter(|_| result != "same") {
            println!("      {}", style(note).yellow());
        }
    }
    Ok(Ok(failed))
}

type Restored = (&'static str, Option<String>);

fn restore(
    dir: &Path,
    file: &ConfigFile,
    target: &Path,
    shown: &str,
    args: &InitArgs,
    interactive: bool,
) -> Result<Result<Restored, String>, Cancel> {
    if let Some(dotfiles) = &file.dotfiles {
        let detail = match &dotfiles.remote {
            Some(remote) => format!("clone {remote} and link {shown} to {}", dotfiles.target),
            None => format!("link {shown} to {}", dotfiles.target),
        };
        return Ok(Ok(("dotfiles", Some(detail))));
    }
    let source = dir.join(FILES).join(&file.path);
    let mut new = match fs::read(&source) {
        Ok(new) => new,
        Err(e) => return Ok(Err(format!("{}: {e}", source.display()))),
    };
    let current = match fs::read(target) {
        Ok(current) => Some(current),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Ok(Err(format!("{shown}: {e}"))),
    };

    let Some(current) = current else {
        if args.dry_run {
            return Ok(Ok(("would_restore", None)));
        }
        return Ok(copy(&source, target).map(|()| ("restored", None)));
    };
    if file.redacted
        && let (Ok(snap), Ok(here)) = (
            String::from_utf8(new.clone()),
            std::str::from_utf8(&current),
        )
    {
        new = snapshot::merge_secrets(&snap, here).into_bytes();
    }
    if current == new {
        return Ok(Ok(("same", None)));
    }
    if args.dry_run {
        return Ok(Ok(("would_replace", None)));
    }
    let replace = if args.overwrite {
        true
    } else if interactive {
        print_diff(target, &source);
        let options = vec!["Keep mine", "Use the snapshot's (mine is kept as a backup)"];
        match prompt(Select::new(&format!("{shown} differs from the snapshot."), options).prompt())?
        {
            Ok(choice) => choice != "Keep mine",
            Err(e) => return Ok(Err(e)),
        }
    } else {
        false
    };
    if !replace {
        return Ok(Ok(("kept", None)));
    }

    let backup = PathBuf::from(format!(
        "{}.repoke-{}.bak",
        target.display(),
        snapshot::date(snapshot::now())
    ));
    if let Err(e) = fs::copy(target, &backup) {
        return Ok(Err(format!("could not back up {shown}: {e}")));
    }
    let detail = format!("yours is at {}", backup.display());
    // Written over the existing file, so it keeps the permissions it has.
    Ok(fs::write(target, &new)
        .map(|()| ("replaced", Some(detail)))
        .map_err(|e| format!("{shown}: {e}")))
}

/// `fs::copy` keeps the snapshot copy's permissions, which came from the original file.
fn copy(source: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::copy(source, target)
        .map(drop)
        .map_err(|e| format!("{}: {e}", target.display()))
}

fn print_diff(current: &Path, snapshot: &Path) {
    let diff = manager::capture(
        "diff",
        ["-u".as_ref(), current.as_os_str(), snapshot.as_os_str()],
    );
    let Ok(out) = diff else {
        return;
    };
    for line in out.stdout.lines().skip(2).take(40) {
        let styled = match line.chars().next() {
            Some('+') => style(line).green(),
            Some('-') => style(line).red(),
            Some('@') => style(line).cyan(),
            _ => style(line).dim(),
        };
        println!("    {styled}");
    }
}

fn from_catalog(
    args: &InitArgs,
    config: &Config,
    interactive: bool,
    progress: &Progress,
    mut engine: Engine,
    started: Instant,
) -> Outcome {
    let found = match check::find(&args.managers, config) {
        Ok(found) if found.is_empty() => {
            return Ok(Err(
                "no package manager found here; install one first (Homebrew: https://brew.sh)"
                    .into(),
            ));
        }
        Ok(found) => found,
        Err(e) => return Ok(Err(e)),
    };

    let resolved: Vec<(Tool, usize, Installed)> = catalog::tools()
        .into_iter()
        .filter_map(|tool| catalog::resolve(&tool, &found).map(|(i, p)| (tool, i, p)))
        .collect();
    let used: Vec<usize> = {
        let mut used: Vec<usize> = resolved.iter().map(|(_, i, _)| *i).collect();
        used.sort_unstable();
        used.dedup();
        used
    };
    let have: Vec<(usize, HashSet<Key>)> = check::parallel(used, |i| {
        let f = &found[i];
        let bar = progress.spinner(f.manager.id(), "checking what is already installed");
        let listed = f.manager.installed(&f.path).unwrap_or_default();
        bar.finish_and_clear();
        (i, listed.iter().map(key).collect())
    });
    let is_installed =
        |i: usize, p: &Installed| have.iter().any(|(j, set)| *j == i && set.contains(&key(p)));

    let (installed, available): (Vec<_>, Vec<_>) = resolved
        .into_iter()
        .partition(|(_, i, p)| is_installed(*i, p));

    let already: Vec<&str> = installed
        .iter()
        .map(|(t, _, _)| t.id.as_str())
        .filter(|id| args.tools.is_empty() || args.tools.iter().any(|t| t == id))
        .collect();
    if !already.is_empty() && !args.json {
        println!(
            "{}",
            style(format!("Already installed: {}.", already.join(", "))).dim()
        );
    }

    let chosen: Vec<(Tool, usize, Installed)> = if !args.tools.is_empty() {
        if let Some(unknown) = args.tools.iter().find(|id| {
            !available
                .iter()
                .chain(&installed)
                .any(|(t, _, _)| &t.id == *id)
        }) {
            return Ok(Err(format!(
                "unknown tool '{unknown}', or no manager here provides it"
            )));
        }
        available
            .into_iter()
            .filter(|(t, _, _)| args.tools.contains(&t.id))
            .collect()
    } else if interactive {
        if available.is_empty() {
            Vec::new()
        } else {
            match prompt(pick_tools(available, &found))? {
                Ok(chosen) => chosen,
                Err(e) => return Ok(Err(e)),
            }
        }
    } else {
        return Ok(Err(
            "pass --tools to pick catalog tools without a terminal, or --from to restore a snapshot"
                .into(),
        ));
    };

    let mut pending: Vec<(usize, Vec<Installed>)> = Vec::new();
    for (_, i, package) in chosen {
        match pending.iter_mut().find(|(j, _)| *j == i) {
            Some((_, packages)) => packages.push(package),
            None => pending.push((i, vec![package])),
        }
    }
    pending.sort_by_key(|(i, _)| *i);
    let shown: Vec<Package> = pending
        .iter()
        .flat_map(|(_, packages)| packages.iter().map(Package::from))
        .collect();
    engine.report.fit(shown.iter());
    for (i, mut packages) in pending {
        let found = &found[i];
        let held = found.manager.incompatible(
            &found.path,
            &packages.iter().map(Package::from).collect::<Vec<_>>(),
        );
        packages.retain(|p| !held.iter().any(|(h, _)| *h == Package::from(p)));
        if !packages.is_empty() {
            engine.run_manager(found, &packages, |found, packages| {
                found.manager.install_steps(&found.path, packages)
            });
        }
        engine.held_back(found.manager, &held);
    }
    Ok(Ok(engine.finish(started, false)))
}

struct ToolChoice {
    tool: Tool,
    manager: usize,
    package: Installed,
    label: String,
}

impl std::fmt::Display for ToolChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn pick_tools(
    available: Vec<(Tool, usize, Installed)>,
    found: &[Found],
) -> Result<Vec<(Tool, usize, Installed)>, InquireError> {
    let cw = available
        .iter()
        .map(|(t, _, _)| t.category.len())
        .max()
        .unwrap_or(0);
    let iw = available
        .iter()
        .map(|(t, _, _)| t.id.len())
        .max()
        .unwrap_or(0);
    let dw = available
        .iter()
        .map(|(t, _, _)| t.description.len())
        .max()
        .unwrap_or(0);
    let options: Vec<ToolChoice> = available
        .into_iter()
        .map(|(tool, manager, package)| ToolChoice {
            label: format!(
                "{:<cw$}  {:<iw$}  {:<dw$}  via {}",
                tool.category,
                tool.id,
                tool.description,
                found[manager].manager.id()
            ),
            tool,
            manager,
            package,
        })
        .collect();
    let picked = MultiSelect::new("What should be installed?", options)
        .with_page_size(20)
        .with_formatter(&|picked| plural(picked.len(), "tool"))
        .prompt()?;
    Ok(picked
        .into_iter()
        .map(|c| (c.tool, c.manager, c.package))
        .collect())
}
