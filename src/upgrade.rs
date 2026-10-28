use std::fmt;
use std::io::{self, BufRead, BufReader, IsTerminal, Read};
use std::process::{Command, ExitCode, Stdio};
use std::slice;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use console::style;
use indicatif::ProgressBar;
use inquire::{InquireError, MultiSelect};
use serde::Serialize;

use crate::UpgradeArgs;
use crate::check::{self, Found};
use crate::config::Config;
use crate::manager::{Package, Step};
use crate::progress::Progress;

const EXIT_CANCELLED: u8 = 130;
/// Lines of a failed command's output shown under the package; `--json` carries all of them.
const ERROR_LINES: usize = 10;

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
        commands: &'a [String],
        packages: &'a [Package],
    },
    Upgraded {
        manager: &'a str,
        packages: &'a [Package],
        seconds: f64,
    },
    Failed {
        manager: &'a str,
        error: &'a str,
        packages: &'a [Package],
        seconds: f64,
    },
    Skipped {
        manager: &'a str,
        reason: &'a str,
        packages: &'a [Package],
    },
    Retrying {
        manager: &'a str,
        error: &'a str,
    },
    Summary(&'a Summary),
}

#[derive(Default, Serialize)]
struct Tally {
    upgraded: usize,
    failed: usize,
    skipped: usize,
    planned: usize,
    seconds: f64,
}

impl Tally {
    fn add(&mut self, other: &Tally) {
        self.upgraded += other.upgraded;
        self.failed += other.failed;
        self.skipped += other.skipped;
        self.planned += other.planned;
        self.seconds += other.seconds;
    }
}

#[derive(Serialize)]
struct ManagerTally {
    id: &'static str,
    #[serde(skip)]
    name: &'static str,
    #[serde(flatten)]
    tally: Tally,
}

#[derive(Serialize)]
struct Summary {
    #[serde(flatten)]
    total: Tally,
    /// Whole run, checks and selection included.
    elapsed_seconds: f64,
    managers: Vec<ManagerTally>,
}

pub fn run(args: &UpgradeArgs, config: &Config) -> Result<ExitCode, String> {
    let started = Instant::now();
    let interactive =
        !args.yes && !args.json && io::stdin().is_terminal() && io::stdout().is_terminal();
    if !args.yes && !interactive && !args.dry_run {
        return Err("not running in a terminal, pass --yes to upgrade without asking".into());
    }
    let progress = Progress::new(args.json);
    let mut report = Reporter::new(args.json);

    let mut checks_failed = false;
    let mut pending = Vec::new();
    for (found, result) in check::gather(&args.selection, config, &progress)? {
        if let Some(error) = &result.error {
            checks_failed = true;
            report.emit(&Event::CheckFailed {
                manager: result.name,
                error,
            });
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
    report.fit(pending.iter().flat_map(|(_, packages)| packages));

    let mut runner = Runner {
        report,
        progress: &progress,
        managers: Vec::new(),
    };
    for (found, packages) in &pending {
        let m = found.manager;
        runner.start(m.id(), m.name());
        let root = m.needs_root(&found.path);
        if root && args.no_sudo {
            runner.tally().skipped += packages.len();
            runner.report.emit(&Event::Skipped {
                manager: m.id(),
                reason: "needs root and --no-sudo was given",
                packages,
            });
            continue;
        }
        if root
            && interactive
            && !args.dry_run
            && let Err(error) = authenticate(&progress)
        {
            runner.record(packages, Duration::ZERO, Err(error));
            continue;
        }

        for step in m.upgrade_steps(&found.path, packages) {
            if args.dry_run {
                runner.tally().planned += step.packages.len();
                let commands: Vec<String> = step
                    .commands
                    .iter()
                    .map(|args| command_line(found, args, root).1)
                    .collect();
                runner.report.emit(&Event::Planned {
                    manager: m.id(),
                    commands: &commands,
                    packages: &step.packages,
                });
                continue;
            }
            match runner.attempt(found, &step, root) {
                // Managers like npm install all or nothing, so one bad package would sink the rest.
                (_, Err(error)) if step.packages.len() > 1 => {
                    runner.report.emit(&Event::Retrying {
                        manager: m.id(),
                        error: &error,
                    });
                    for package in &step.packages {
                        for single in m.upgrade_steps(&found.path, slice::from_ref(package)) {
                            let (elapsed, outcome) = runner.attempt(found, &single, root);
                            runner.record(&single.packages, elapsed, outcome);
                        }
                    }
                }
                (elapsed, outcome) => runner.record(&step.packages, elapsed, outcome),
            }
        }
    }

    let mut total = Tally::default();
    for m in &runner.managers {
        total.add(&m.tally);
    }
    let failed = checks_failed || total.failed > 0;
    let summary = Summary {
        total,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        managers: runner.managers,
    };
    runner.report.emit(&Event::Summary(&summary));
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

struct Runner<'a> {
    report: Reporter,
    progress: &'a Progress,
    managers: Vec<ManagerTally>,
}

impl Runner<'_> {
    fn start(&mut self, id: &'static str, name: &'static str) {
        self.report.header(name);
        self.managers.push(ManagerTally {
            id,
            name,
            tally: Tally::default(),
        });
    }

    fn tally(&mut self) -> &mut Tally {
        &mut self
            .managers
            .last_mut()
            .expect("a manager was started")
            .tally
    }

    fn manager(&self) -> &'static str {
        self.managers.last().expect("a manager was started").id
    }

    /// Runs the step's commands in order and stops at the first one that fails.
    fn attempt(&self, found: &Found, step: &Step, root: bool) -> (Duration, Result<(), String>) {
        let started = Instant::now();
        for args in &step.commands {
            let (command, shown) = command_line(found, args, root);
            self.report.emit(&Event::Running {
                manager: self.manager(),
                command: &shown,
                packages: &step.packages,
            });
            let bar = self.progress.spinner(self.manager(), &shown);
            let outcome = execute(&command, &bar);
            bar.finish_and_clear();
            if let Err(error) = outcome {
                return (started.elapsed(), Err(format!("{shown} {error}")));
            }
        }
        (started.elapsed(), Ok(()))
    }

    fn record(&mut self, packages: &[Package], elapsed: Duration, outcome: Result<(), String>) {
        let manager = self.manager();
        let seconds = elapsed.as_secs_f64();
        self.tally().seconds += seconds;
        match outcome {
            Ok(()) => {
                self.tally().upgraded += packages.len();
                self.report.emit(&Event::Upgraded {
                    manager,
                    packages,
                    seconds,
                });
            }
            Err(error) => {
                self.tally().failed += packages.len();
                self.report.emit(&Event::Failed {
                    manager,
                    error: &error,
                    packages,
                    seconds,
                });
            }
        }
    }
}

/// Prints events as JSON lines, or as a list grouped by manager for people.
struct Reporter {
    json: bool,
    name_width: usize,
    current_width: usize,
}

impl Reporter {
    fn new(json: bool) -> Self {
        Self {
            json,
            name_width: 0,
            current_width: 0,
        }
    }

    fn fit<'a>(&mut self, packages: impl Iterator<Item = &'a Package>) {
        for p in packages {
            self.name_width = self.name_width.max(p.name.chars().count());
            self.current_width = self.current_width.max(p.current.chars().count());
        }
    }

    fn header(&self, name: &str) {
        if !self.json {
            println!("\n{}", style(name).bold());
        }
    }

    fn emit(&self, event: &Event) {
        if self.json {
            println!(
                "{}",
                serde_json::to_string(event).expect("serializable event")
            );
            return;
        }
        match event {
            Event::CheckFailed { manager, error } => eprintln!(
                "{} {manager}: check failed, {}",
                style("✗").red(),
                first_line(error)
            ),
            // The spinner already shows the command while it runs.
            Event::Running { .. } => {}
            Event::Planned {
                commands, packages, ..
            } => {
                self.packages(&style("•").cyan().to_string(), packages, 0.0);
                for command in *commands {
                    println!("      {}", style(format!("$ {command}")).dim());
                }
            }
            Event::Upgraded {
                packages, seconds, ..
            } => self.packages(&style("✓").green().to_string(), packages, *seconds),
            Event::Failed {
                packages,
                error,
                seconds,
                ..
            } => {
                self.packages(&style("✗").red().to_string(), packages, *seconds);
                let mut lines = error.lines();
                if let Some(first) = lines.next() {
                    println!("      {}", style(first).red());
                }
                let rest: Vec<&str> = lines.collect();
                for line in &rest[rest.len().saturating_sub(ERROR_LINES)..] {
                    println!("      {} {}", style("│").dim(), style(line).dim());
                }
            }
            Event::Skipped {
                reason, packages, ..
            } => {
                self.packages(&style("-").yellow().to_string(), packages, 0.0);
                println!("      {}", style(reason).yellow());
            }
            Event::Retrying { .. } => println!(
                "  {} {}",
                style("↻").yellow(),
                style("the batch failed, retrying one package at a time").yellow()
            ),
            Event::Summary(summary) => self.summary(summary),
        }
    }

    /// One line per package; a batch shares one duration, shown on its first line.
    fn packages(&self, mark: &str, packages: &[Package], seconds: f64) {
        for (i, p) in packages.iter().enumerate() {
            let time = if i == 0 && seconds > 0.0 {
                style(duration(seconds)).dim().to_string()
            } else {
                String::new()
            };
            println!(
                "  {mark} {:<nw$}  {:>cw$} → {:<10} {time}",
                p.name,
                p.current,
                p.latest,
                nw = self.name_width,
                cw = self.current_width,
            );
        }
    }

    fn summary(&self, summary: &Summary) {
        let total = &summary.total;
        if summary.managers.is_empty() {
            println!("\n{}", style("Everything is up to date.").green());
            return;
        }
        if total.planned > 0 {
            println!(
                "\n{} would be upgraded. Nothing was changed (dry run).",
                plural(total.planned, "package")
            );
            return;
        }

        let width = summary
            .managers
            .iter()
            .map(|m| m.name.len())
            .max()
            .unwrap_or(0)
            .max("Total".len());
        println!("\n{}", style("Summary").bold());
        println!(
            "  {}",
            style(format!(
                "{:<width$}  {:>8}  {:>6}  {:>7}  {:>7}",
                "Manager", "Upgraded", "Failed", "Skipped", "Time"
            ))
            .dim()
        );
        let row = |name: &str, t: &Tally| {
            let failed = format!("{:>6}", t.failed);
            let failed = if t.failed > 0 {
                style(failed).red().to_string()
            } else {
                failed
            };
            format!(
                "{name:<width$}  {:>8}  {failed}  {:>7}  {:>7}",
                t.upgraded,
                t.skipped,
                duration(t.seconds)
            )
        };
        for m in &summary.managers {
            println!("  {}", row(m.name, &m.tally));
        }
        println!("  {}", style(row("Total", total)).bold());
        println!("\nFinished in {}.", duration(summary.elapsed_seconds));
    }
}

fn duration(seconds: f64) -> String {
    match seconds {
        s if s < 1.0 => format!("{}ms", (s * 1000.0).round()),
        s if s < 60.0 => format!("{}s", s.round()),
        s => format!("{}m {:02}s", (s / 60.0).floor(), (s % 60.0).round()),
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

/// The full command to run, and how to show it: with the manager's name, not its path or wrapper.
fn command_line(found: &Found, args: &[String], root: bool) -> (Vec<String>, String) {
    // -n never prompts: interactive runs ask for the password up front with `sudo -v`.
    let sudo: &[&str] = if root { &["sudo", "-n"] } else { &[] };
    let mut command: Vec<String> = sudo.iter().map(|s| s.to_string()).collect();
    let mut shown = command.clone();
    command.extend(found.manager.invocation(&found.path));
    command.extend(args.iter().cloned());
    shown.push(found.manager.binary().to_string());
    shown.extend(args.iter().cloned());
    (command, shown.join(" "))
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
                    if !line.is_empty() && !is_warning(line) {
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
        "exited with {}\n{}",
        status
            .code()
            .map_or("a signal".to_string(), |c| c.to_string()),
        lines[lines.len().saturating_sub(40)..].join("\n")
    ))
}

/// Warnings about unrelated config (e.g. an `.npmrc` token) would bury the actual error.
fn is_warning(line: &str) -> bool {
    [
        "[WARN]",
        "npm warn",
        "WARN ",
        "You are running Composer with SSL/TLS",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
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
    let id_width = pending
        .iter()
        .map(|(found, _)| found.manager.id().len())
        .max()
        .unwrap_or(0);
    let name_width = pending
        .iter()
        .flat_map(|(_, packages)| packages)
        .map(|p| p.name.chars().count())
        .max()
        .unwrap_or(0);
    let current_width = pending
        .iter()
        .flat_map(|(_, packages)| packages)
        .map(|p| p.current.chars().count())
        .max()
        .unwrap_or(0);
    let options: Vec<Choice> = pending
        .iter()
        .enumerate()
        .flat_map(|(i, (found, packages))| {
            packages.iter().map(move |p| Choice {
                manager: i,
                label: format!(
                    "{:<id_width$}  {:<name_width$}  {:>current_width$} → {}",
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
        .with_formatter(&|picked| plural(picked.len(), "package"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(duration(0.25), "250ms");
        assert_eq!(duration(12.4), "12s");
        assert_eq!(duration(102.0), "1m 42s");
    }

    #[cfg(unix)]
    fn sh(script: &str) -> Vec<String> {
        ["sh", "-c", script].map(String::from).to_vec()
    }

    #[cfg(unix)]
    #[test]
    fn execute_reports_exit_code_and_output_tail() {
        let error = execute(
            &sh("echo working; echo broken >&2; exit 3"),
            &ProgressBar::hidden(),
        )
        .unwrap_err();
        assert!(error.starts_with("exited with 3\n"), "{error}");
        assert!(
            error.contains("working") && error.contains("broken"),
            "{error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn execute_leaves_warnings_out_of_the_error() {
        let error = execute(
            &sh("echo '[WARN] Failed to replace env in config' >&2; echo 'Error: real cause' >&2; exit 1"),
            &ProgressBar::hidden(),
        )
        .unwrap_err();
        assert_eq!(error, "exited with 1\nError: real cause");
    }

    #[cfg(unix)]
    #[test]
    fn execute_succeeds_on_zero_exit() {
        assert!(execute(&sh("echo done"), &ProgressBar::hidden()).is_ok());
    }
}
