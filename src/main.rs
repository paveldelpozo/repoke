mod check;
mod config;
mod manager;
mod managers;
mod progress;
mod upgrade;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;

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
    Check {
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        selection: Selection,
    },
    /// Upgrade pending packages, picking them in a list or all at once with --yes
    Upgrade(UpgradeArgs),
}

#[derive(Args)]
struct Selection {
    /// Only these managers (comma separated ids, see `repoke list`)
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Skip these managers
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,
    /// Leave these packages alone, as `name` or `manager:name`
    #[arg(long, value_delimiter = ',')]
    hold: Vec<String>,
    /// Refresh package indexes first (brew update)
    #[arg(long)]
    refresh: bool,
}

#[derive(Args)]
struct UpgradeArgs {
    #[command(flatten)]
    selection: Selection,
    /// Upgrade everything selected without asking; required outside a terminal
    #[arg(long, short)]
    yes: bool,
    /// Print the commands that would run, change nothing
    #[arg(long)]
    dry_run: bool,
    /// Skip managers that need root instead of calling sudo
    #[arg(long)]
    no_sudo: bool,
    /// Print one JSON event per line
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
struct Detected {
    id: &'static str,
    name: &'static str,
    installed: bool,
    path: Option<PathBuf>,
    version: Option<String>,
}

fn main() -> ExitCode {
    let command = Cli::parse().command;
    if let Command::List { json } = command {
        return list(json);
    }
    let outcome = config::load().and_then(|config| match &command {
        Command::Check { json, selection } => check::run(selection, &config, *json),
        Command::Upgrade(args) => upgrade::run(args, &config),
        Command::List { .. } => unreachable!("handled above"),
    });
    outcome.unwrap_or_else(|e| {
        eprintln!("repoke: {e}");
        ExitCode::from(EXIT_USAGE)
    })
}

fn list(json: bool) -> ExitCode {
    let progress = progress::Progress::new(json);
    let bar = progress.spinner("", "looking for package managers");
    let detected: Vec<Detected> = check::parallel(managers::all(), |m| {
        let path = m.locate();
        Detected {
            id: m.id(),
            name: m.name(),
            installed: path.is_some(),
            version: path.as_deref().and_then(|p| m.version(p)),
            path,
        }
    });
    bar.finish_and_clear();

    if json {
        print_json(&detected);
    } else {
        let id_width = detected.iter().map(|d| d.id.len()).max().unwrap_or(0);
        let name_width = detected.iter().map(|d| d.name.len()).max().unwrap_or(0);
        for d in &detected {
            let state = match &d.version {
                Some(version) => version.as_str(),
                None if d.installed => "installed",
                None => "not found",
            };
            println!("{:<id_width$}  {:<name_width$}  {state}", d.id, d.name);
        }
    }
    ExitCode::SUCCESS
}

fn print_json<T: Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serializable output")
    );
}
