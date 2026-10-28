mod manager;
mod managers;

use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use manager::{Manager, Package};

/// Same convention as `dnf check-update`: an agent can tell "updates pending" from "failed".
const EXIT_UPDATES: u8 = 100;
const EXIT_USAGE: u8 = 2;

#[derive(Parser)]
#[command(
    version,
    about = "Finds the package managers on this system and keeps them up to date"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the package managers repoke supports here and whether they are installed
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show pending updates without changing anything; exits 100 when there are some
    Check(CheckArgs),
}

#[derive(Args)]
struct CheckArgs {
    #[arg(long)]
    json: bool,
    /// Only check these managers (comma separated ids, see `repoke list`)
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Skip these managers
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,
}

#[derive(Serialize)]
struct Detected {
    id: &'static str,
    name: &'static str,
    installed: bool,
    path: Option<PathBuf>,
    version: Option<String>,
}

#[derive(Serialize)]
struct CheckResult {
    id: &'static str,
    name: &'static str,
    version: Option<String>,
    packages: Vec<Package>,
    error: Option<String>,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::List { json } => list(json),
        Command::Check(args) => check(&args),
    }
}

fn list(json: bool) -> ExitCode {
    let detected: Vec<Detected> = parallel(managers::all(), |m| {
        let path = manager::locate(m.binary());
        Detected {
            id: m.id(),
            name: m.name(),
            installed: path.is_some(),
            version: path.as_deref().and_then(|p| m.version(p)),
            path,
        }
    });

    if json {
        print_json(&detected);
    } else {
        for d in &detected {
            match &d.version {
                Some(version) => println!("{:<6} {:<15} {version}", d.id, d.name),
                None if d.installed => println!("{:<6} {:<15} installed", d.id, d.name),
                None => println!("{:<6} {:<15} not found", d.id, d.name),
            }
        }
    }
    ExitCode::SUCCESS
}

fn check(args: &CheckArgs) -> ExitCode {
    let selected = match select(&args.only, &args.exclude) {
        Ok(selected) => selected,
        Err(e) => {
            eprintln!("repoke: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let installed: Vec<_> = selected
        .into_iter()
        .filter_map(|m| manager::locate(m.binary()).map(|path| (m, path)))
        .collect();

    let results: Vec<CheckResult> = parallel(installed, |(m, path)| {
        let (packages, error) = match m.outdated(&path) {
            Ok(packages) => (packages, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        CheckResult {
            id: m.id(),
            name: m.name(),
            version: m.version(&path),
            packages,
            error,
        }
    });

    if args.json {
        print_json(&results);
    } else {
        print_check(&results);
    }

    if results.iter().any(|r| r.error.is_some()) {
        ExitCode::FAILURE
    } else if results.iter().any(|r| !r.packages.is_empty()) {
        ExitCode::from(EXIT_UPDATES)
    } else {
        ExitCode::SUCCESS
    }
}

fn select(only: &[String], exclude: &[String]) -> Result<Vec<&'static dyn Manager>, String> {
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

fn print_check(results: &[CheckResult]) {
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

fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serializable output")
    );
}

/// Each manager shells out and waits on the network, so they run side by side.
fn parallel<I, T, R>(items: I, f: impl Fn(T) -> R + Sync) -> Vec<R>
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
