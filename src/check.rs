use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use serde::Serialize;

use crate::config::{self, Config};
use crate::manager::{Manager, Package};
use crate::progress::Progress;
use crate::{EXIT_UPDATES, Managers, Selection, managers, print_json};

pub struct Found {
    pub manager: &'static dyn Manager,
    pub path: PathBuf,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub id: &'static str,
    pub name: &'static str,
    pub version: Option<String>,
    pub packages: Vec<Package>,
    pub error: Option<String>,
}

pub fn run(selection: &Selection, config: &Config, json: bool) -> Result<ExitCode, String> {
    let results: Vec<CheckResult> = gather(selection, config, &Progress::new(json))?
        .into_iter()
        .map(|(_, result)| result)
        .collect();

    if json {
        print_json(&results);
    } else {
        print(&results);
    }

    Ok(if results.iter().any(|r| r.error.is_some()) {
        ExitCode::FAILURE
    } else if results.iter().any(|r| !r.packages.is_empty()) {
        ExitCode::from(EXIT_UPDATES)
    } else {
        ExitCode::SUCCESS
    })
}

/// Finds the selected managers and asks each one, in parallel, what is outdated.
pub fn gather(
    selection: &Selection,
    config: &Config,
    progress: &Progress,
) -> Result<Vec<(Found, CheckResult)>, String> {
    let found = find(&selection.managers, config)?;
    let hold: Vec<String> = config.hold.iter().chain(&selection.hold).cloned().collect();

    Ok(parallel(found, |found| {
        let m = found.manager;
        let bar = progress.spinner(m.id(), "checking for updates");
        let checked = crate::manager::within(m.timeout(), || {
            if selection.refresh && m.has_refresh() {
                bar.set_message("refreshing the package index");
                m.refresh(&found.path)?;
                bar.set_message("checking for updates");
            }
            m.outdated(&found.path)
        });
        let (packages, error) = match checked {
            Ok(packages) => (
                packages
                    .into_iter()
                    .filter(|p| !config::held(&hold, m.id(), &p.name))
                    .collect(),
                None,
            ),
            Err(e) => (Vec::new(), Some(e)),
        };
        let result = CheckResult {
            id: m.id(),
            name: m.name(),
            version: m.version(&found.path),
            packages,
            error,
        };
        bar.finish_and_clear();
        (found, result)
    }))
}

/// The selected managers that are installed here.
pub fn find(managers: &Managers, config: &Config) -> Result<Vec<Found>, String> {
    Ok(select(managers, config)?
        .into_iter()
        .filter_map(|m| m.locate().map(|path| Found { manager: m, path }))
        .collect())
}

/// The selected managers supported on this OS, installed or not.
pub fn select(managers: &Managers, config: &Config) -> Result<Vec<&'static dyn Manager>, String> {
    // An explicit --only wins over the managers the config file excludes.
    let mut exclude = managers.exclude.clone();
    if managers.only.is_empty() {
        exclude.extend(config.exclude.iter().cloned());
    }
    by_id(&managers.only, &exclude)
}

fn by_id(only: &[String], exclude: &[String]) -> Result<Vec<&'static dyn Manager>, String> {
    let all = managers::all();
    if let Some(unknown) = only
        .iter()
        .chain(exclude)
        .find(|id| !all.iter().any(|m| m.id() == id.as_str()))
    {
        let known: Vec<_> = all.iter().map(|m| m.id()).collect();
        return Err(format!(
            "unknown manager '{unknown}' (known: {})",
            known.join(", ")
        ));
    }
    Ok(all
        .into_iter()
        .filter(|m| only.is_empty() || only.iter().any(|id| id == m.id()))
        .filter(|m| !exclude.iter().any(|id| id == m.id()))
        .collect())
}

fn print(results: &[CheckResult]) {
    if results.is_empty() {
        println!("No supported package managers found.");
        return;
    }
    for r in results {
        match (&r.error, r.packages.len()) {
            (Some(e), _) => println!("{}: check failed, {e}", r.name),
            (None, 0) => println!("{}: up to date", r.name),
            (None, n) => {
                println!("{}: {n} update{}", r.name, if n == 1 { "" } else { "s" });
                let width = r.packages.iter().map(|p| p.name.len()).max().unwrap_or(0);
                for p in &r.packages {
                    println!("  {:<width$}  {} → {}", p.name, p.current, p.latest);
                }
            }
        }
    }
}

/// Each manager shells out and waits on the network, so they run side by side.
pub fn parallel<I, T, R>(items: I, f: impl Fn(T) -> R + Sync) -> Vec<R>
where
    I: IntoIterator<Item = T>,
    T: Send,
    R: Send,
{
    thread::scope(|s| {
        let handles: Vec<_> = items.into_iter().map(|item| s.spawn(|| f(item))).collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("manager thread panicked"))
            .collect()
    })
}
