mod catalog;
mod check;
mod config;
mod export;
mod init;
mod manager;
mod managers;
mod progress;
mod run;
mod schedule;
mod self_update;
mod snapshot;
mod upgrade;

use std::io::IsTerminal;
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
    /// Save what is installed, and optionally its config, to a folder another machine can restore
    Export(ExportArgs),
    /// Install a saved snapshot (--from) or pick tools from a catalog, on a new machine
    Init(InitArgs),
    /// Update repoke itself, through Homebrew or cargo when it was installed with them
    SelfUpdate(SelfUpdateArgs),
    /// Keep the machine up to date on a schedule, through the system scheduler
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
}

#[derive(Subcommand)]
enum ScheduleAction {
    /// Register the periodic run, replacing any earlier one
    Install(ScheduleInstallArgs),
    /// Show the schedule and how the last run went
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Unregister the periodic run
    Remove {
        /// Print what would be removed
        #[arg(long)]
        dry_run: bool,
    },
    /// Run the scheduled job now, as the scheduler does: log it and notify
    Run,
}

#[derive(Args)]
struct SelfUpdateArgs {
    /// Only say whether a newer version exists; exits 100 when it does
    #[arg(long)]
    check: bool,
    /// Update without asking
    #[arg(long, short)]
    yes: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ScheduleInstallArgs {
    #[command(flatten)]
    managers: Managers,
    #[arg(long, value_enum, default_value = "daily")]
    every: schedule::Every,
    /// Local time, 24-hour clock; weekly runs are on Mondays. Missed while asleep, it runs on wake
    #[arg(long, default_value = "03:00")]
    at: String,
    /// Only check and notify; don't upgrade
    #[arg(long)]
    check_only: bool,
    /// Print the scheduler files and commands, register nothing
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args)]
struct Managers {
    /// Only these managers (comma separated ids, see `repoke list`)
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Skip these managers
    #[arg(long, value_delimiter = ',')]
    exclude: Vec<String>,
}

#[derive(Args)]
struct Selection {
    #[command(flatten)]
    managers: Managers,
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

#[derive(Args)]
struct ExportArgs {
    #[command(flatten)]
    managers: Managers,
    /// Folder to write (default: repoke-<host>-<date>)
    #[arg(long, short)]
    output: Option<PathBuf>,
    /// Include every manager's config files without asking
    #[arg(long, conflicts_with = "no_config")]
    config: bool,
    /// Leave all config files out
    #[arg(long)]
    no_config: bool,
    /// Don't ask anything: config only with --config, and secret-looking values are removed
    #[arg(long, short)]
    yes: bool,
    /// Replace a snapshot already in the output folder
    #[arg(long)]
    force: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct InitArgs {
    #[command(flatten)]
    managers: Managers,
    /// Snapshot folder written by `repoke export`
    #[arg(long)]
    from: Option<PathBuf>,
    /// Catalog tools to install without the picker (comma separated ids, see the picker)
    #[arg(long, value_delimiter = ',', conflicts_with = "from")]
    tools: Vec<String>,
    /// Install everything without asking; required outside a terminal
    #[arg(long, short)]
    yes: bool,
    /// Show what would be installed and restored, change nothing
    #[arg(long)]
    dry_run: bool,
    /// Skip managers that need root instead of calling sudo
    #[arg(long)]
    no_sudo: bool,
    /// Don't restore config files
    #[arg(long)]
    no_config: bool,
    /// Replace config files that differ without asking (yours are backed up first)
    #[arg(long)]
    overwrite: bool,
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
    if std::env::var_os("NO_COLOR").is_some() {
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }
    let command = Cli::parse().command;
    let code = dispatch(&command);
    if notices_updates(&command) {
        self_update::notice();
    }
    code
}

/// Only for people at a terminal: never under --json, in a pipe or in the scheduled job.
fn notices_updates(command: &Command) -> bool {
    let json = match command {
        Command::List { json } | Command::Check { json, .. } => *json,
        Command::Upgrade(args) => args.json,
        Command::Export(args) => args.json,
        Command::Init(args) => args.json,
        Command::SelfUpdate(_) | Command::Schedule { .. } => true,
    };
    !json && std::io::stdout().is_terminal() && std::io::stderr().is_terminal()
}

fn dispatch(command: &Command) -> ExitCode {
    let scheduled = match command {
        Command::List { json } => return list(*json),
        Command::Schedule { action } => Some(match action {
            ScheduleAction::Install(args) => schedule::install(args),
            ScheduleAction::Status { json } => schedule::status(*json),
            ScheduleAction::Remove { dry_run } => schedule::remove(*dry_run),
            ScheduleAction::Run => schedule::run_job(),
        }),
        _ => None,
    };
    if let Some(outcome) = scheduled {
        return outcome.unwrap_or_else(|e| {
            eprintln!("repoke: {e}");
            ExitCode::from(EXIT_USAGE)
        });
    }
    let outcome = config::load().and_then(|config| match &command {
        Command::Check { json, selection } => check::run(selection, &config, *json),
        Command::Upgrade(args) => upgrade::run(args, &config),
        Command::Export(args) => export::run(args, &config),
        Command::Init(args) => init::run(args, &config),
        Command::SelfUpdate(args) => self_update::run(args),
        Command::List { .. } | Command::Schedule { .. } => unreachable!("handled above"),
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
