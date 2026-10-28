//! Runs a plan of manager steps for `upgrade` and `init`: spinners, sudo, retries, the per-package
//! report and the closing summary.

use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, ExitCode, Stdio};
use std::slice;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use console::style;
use indicatif::ProgressBar;
use inquire::{InquireError, MultiSelect};
use serde::Serialize;

use crate::check::Found;
use crate::manager::{self, Manager, Package, Step};
use crate::progress::Progress;

pub const EXIT_CANCELLED: u8 = 130;
/// Lines of a failed command's output shown under the package; `--json` carries all of them.
const ERROR_LINES: usize = 10;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Upgrade,
    Install,
}

impl Action {
    fn title(self) -> &'static str {
        match self {
            Action::Upgrade => "Upgraded",
            Action::Install => "Installed",
        }
    }

    fn pending(self) -> &'static str {
        match self {
            Action::Upgrade => "would be upgraded",
            Action::Install => "would be installed",
        }
    }
}

pub struct Options {
    pub action: Action,
    pub json: bool,
    pub dry_run: bool,
    pub no_sudo: bool,
    pub interactive: bool,
}

/// With `--json` each event is printed as one line, so an agent can follow progress as it happens.
#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event<'a> {
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
    Done {
        action: Action,
        manager: &'a str,
        packages: &'a [Package],
        seconds: f64,
    },
    Failed {
        action: Action,
        manager: &'a str,
        error: &'a str,
        /// What usually fixes this error, when the manager recognises it.
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
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
pub struct Tally {
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
    pub planned: usize,
    pub seconds: f64,
}

impl Tally {
    fn add(&mut self, other: &Tally) {
        self.done += other.done;
        self.failed += other.failed;
        self.skipped += other.skipped;
        self.planned += other.planned;
        self.seconds += other.seconds;
    }
}

#[derive(Serialize)]
pub struct ManagerTally {
    id: &'static str,
    #[serde(skip)]
    name: &'static str,
    #[serde(skip)]
    manager: &'static dyn Manager,
    #[serde(flatten)]
    tally: Tally,
}

#[derive(Serialize)]
pub struct Summary {
    action: Action,
    #[serde(flatten)]
    total: Tally,
    /// Whole run, checks and selection included.
    elapsed_seconds: f64,
    managers: Vec<ManagerTally>,
}

pub struct Engine<'a> {
    pub report: Reporter,
    options: Options,
    progress: &'a Progress,
    managers: Vec<ManagerTally>,
}

impl<'a> Engine<'a> {
    pub fn new(options: Options, progress: &'a Progress) -> Self {
        Self {
            report: Reporter::new(options.json, options.action),
            options,
            progress,
            managers: Vec::new(),
        }
    }

    /// Runs one manager's share of the plan. `steps` turns items into commands, and is called
    /// again per item when a batch fails.
    pub fn run_manager<T>(
        &mut self,
        found: &Found,
        items: &[T],
        steps: impl Fn(&Found, &[T]) -> Vec<Step>,
    ) {
        let m = found.manager;
        self.report.header(m.name());
        self.managers.push(ManagerTally {
            id: m.id(),
            name: m.name(),
            manager: m,
            tally: Tally::default(),
        });

        let root = m.needs_root(&found.path);
        if root && self.options.no_sudo {
            let packages: Vec<Package> = steps(found, items)
                .into_iter()
                .flat_map(|s| s.packages)
                .collect();
            self.tally().skipped += packages.len();
            self.report.emit(&Event::Skipped {
                manager: m.id(),
                reason: "needs root and --no-sudo was given",
                packages: &packages,
            });
            return;
        }
        if root
            && self.options.interactive
            && !self.options.dry_run
            && let Err(error) = authenticate(self.progress)
        {
            let packages: Vec<Package> = steps(found, items)
                .into_iter()
                .flat_map(|s| s.packages)
                .collect();
            self.record(&packages, Duration::ZERO, Err(error));
            return;
        }

        for step in steps(found, items) {
            if self.options.dry_run {
                self.tally().planned += step.packages.len();
                let commands: Vec<String> = step
                    .commands
                    .iter()
                    .map(|args| command_line(found, args, root).1)
                    .collect();
                self.report.emit(&Event::Planned {
                    manager: m.id(),
                    commands: &commands,
                    packages: &step.packages,
                });
                continue;
            }
            match self.attempt(found, &step, root) {
                // Managers like npm install all or nothing, so one bad package would sink the rest.
                (_, Err(error)) if step.packages.len() > 1 && items.len() > 1 => {
                    self.report.emit(&Event::Retrying {
                        manager: m.id(),
                        error: &error,
                    });
                    for item in items {
                        for single in steps(found, slice::from_ref(item)) {
                            if single.packages.iter().any(|p| step.packages.contains(p)) {
                                let (elapsed, outcome) = self.attempt(found, &single, root);
                                self.record(&single.packages, elapsed, outcome);
                            }
                        }
                    }
                }
                (elapsed, outcome) => self.record(&step.packages, elapsed, outcome),
            }
        }
    }

    /// Reports a manager whose packages can't be handled, such as one that isn't installed.
    pub fn skip(&mut self, manager: &'static dyn Manager, packages: &[Package], reason: &str) {
        self.report.header(manager.name());
        self.managers.push(ManagerTally {
            id: manager.id(),
            name: manager.name(),
            manager,
            tally: Tally {
                skipped: packages.len(),
                ..Tally::default()
            },
        });
        self.report.emit(&Event::Skipped {
            manager: manager.id(),
            reason,
            packages,
        });
    }

    /// Prints the summary and gives the exit code.
    pub fn finish(self, started: Instant, checks_failed: bool) -> ExitCode {
        let mut total = Tally::default();
        for m in &self.managers {
            total.add(&m.tally);
        }
        let failed = checks_failed || total.failed > 0;
        let summary = Summary {
            action: self.options.action,
            total,
            elapsed_seconds: started.elapsed().as_secs_f64(),
            managers: self.managers,
        };
        self.report.emit(&Event::Summary(&summary));
        if failed {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
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
            let outcome = execute(&command, &bar, found.manager.timeout());
            bar.finish_and_clear();
            if let Err(error) = outcome {
                return (started.elapsed(), Err(format!("{shown} {error}")));
            }
        }
        (started.elapsed(), Ok(()))
    }

    fn record(&mut self, packages: &[Package], elapsed: Duration, outcome: Result<(), String>) {
        let manager = self.manager();
        let action = self.options.action;
        let seconds = elapsed.as_secs_f64();
        self.tally().seconds += seconds;
        match outcome {
            Ok(()) => {
                self.tally().done += packages.len();
                self.report.emit(&Event::Done {
                    action,
                    manager,
                    packages,
                    seconds,
                });
            }
            Err(error) => {
                self.tally().failed += packages.len();
                let hint = self
                    .managers
                    .last()
                    .and_then(|m| m.manager.hint(packages, &error));
                self.report.emit(&Event::Failed {
                    action,
                    manager,
                    error: &error,
                    hint,
                    packages,
                    seconds,
                });
            }
        }
    }
}

/// Prints events as JSON lines, or as a list grouped by manager for people.
pub struct Reporter {
    json: bool,
    action: Action,
    name_width: usize,
    current_width: usize,
}

impl Reporter {
    fn new(json: bool, action: Action) -> Self {
        Self {
            json,
            action,
            name_width: 0,
            current_width: 0,
        }
    }

    pub fn fit<'p>(&mut self, packages: impl Iterator<Item = &'p Package>) {
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

    pub fn emit(&self, event: &Event) {
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
                error.lines().next().unwrap_or_default()
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
            Event::Done {
                packages, seconds, ..
            } => self.packages(&style("✓").green().to_string(), packages, *seconds),
            Event::Failed {
                packages,
                error,
                hint,
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
                if let Some(hint) = hint {
                    println!("      {} {}", style("→").yellow(), style(hint).yellow());
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
            let versions = match (p.current.as_str(), p.latest.as_str()) {
                ("", "") => String::new(),
                ("", latest) => format!("{:>cw$}   {latest:<10}", "", cw = self.current_width),
                (current, latest) => {
                    format!("{current:>cw$} → {latest:<10}", cw = self.current_width)
                }
            };
            println!(
                "  {mark} {:<nw$}  {versions} {time}",
                p.name,
                nw = self.name_width
            );
        }
    }

    fn summary(&self, summary: &Summary) {
        let total = &summary.total;
        if summary.managers.is_empty() {
            println!(
                "\n{}",
                style(match self.action {
                    Action::Upgrade => "Everything is up to date.",
                    Action::Install => "Nothing to install.",
                })
                .green()
            );
            return;
        }
        if total.planned > 0 {
            println!(
                "\n{} {}. Nothing was changed (dry run).",
                plural(total.planned, "package"),
                self.action.pending()
            );
            return;
        }

        let title = self.action.title();
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
                "{:<width$}  {title:>9}  {:>6}  {:>7}  {:>7}",
                "Manager", "Failed", "Skipped", "Time"
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
                "{name:<width$}  {:>9}  {failed}  {:>7}  {:>7}",
                t.done,
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

pub fn duration(seconds: f64) -> String {
    match seconds {
        s if s < 1.0 => format!("{}ms", (s * 1000.0).round()),
        s if s < 60.0 => format!("{}s", s.round()),
        s => format!("{}m {:02}s", (s / 60.0).floor(), (s % 60.0).round()),
    }
}

pub fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

struct Choice<T> {
    manager: usize,
    item: T,
    label: String,
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// Multi-select over every item of every manager; `label` gives the text after the manager id.
pub fn choose<T: Clone>(
    prompt: &str,
    pending: Vec<(Found, Vec<T>)>,
    preselected: bool,
    label: impl Fn(&T) -> String,
) -> Result<Vec<(Found, Vec<T>)>, InquireError> {
    let id_width = pending
        .iter()
        .map(|(found, _)| found.manager.id().len())
        .max()
        .unwrap_or(0);
    let options: Vec<Choice<T>> = pending
        .iter()
        .enumerate()
        .flat_map(|(i, (found, items))| {
            let label = &label;
            items.iter().map(move |item| Choice {
                manager: i,
                label: format!("{:<id_width$}  {}", found.manager.id(), label(item)),
                item: item.clone(),
            })
        })
        .collect();

    let mut select = MultiSelect::new(prompt, options)
        .with_page_size(20)
        .with_formatter(&|picked| plural(picked.len(), "package"));
    if preselected {
        select = select.with_all_selected_by_default();
    }
    let picked = select.prompt()?;

    Ok(pending
        .into_iter()
        .enumerate()
        .filter_map(|(i, (found, _))| {
            let items: Vec<T> = picked
                .iter()
                .filter(|c| c.manager == i)
                .map(|c| c.item.clone())
                .collect();
            (!items.is_empty()).then_some((found, items))
        })
        .collect())
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
fn execute(command: &[String], bar: &ProgressBar, timeout: Option<Duration>) -> Result<(), String> {
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
    let lines = Arc::new(Mutex::new(Vec::new()));
    let readers: Vec<_> = pipes
        .into_iter()
        .map(|pipe| {
            let (bar, lines) = (bar.clone(), Arc::clone(&lines));
            thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    let line = line.trim();
                    if !line.is_empty() && !is_warning(line) {
                        bar.set_message(line.to_string());
                        lines.lock().expect("output lock").push(line.to_string());
                    }
                }
            })
        })
        .collect();

    let status = manager::wait_within(&mut child, timeout, program.as_ref())?;
    for reader in readers {
        let _ = reader.join();
    }
    if status.success() {
        return Ok(());
    }
    let lines = lines.lock().expect("output lock");
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
            None,
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
            None,
        )
        .unwrap_err();
        assert_eq!(error, "exited with 1\nError: real cause");
    }

    #[cfg(unix)]
    #[test]
    fn execute_stops_commands_that_run_too_long() {
        let started = Instant::now();
        let error = execute(
            &sh("sleep 5"),
            &ProgressBar::hidden(),
            Some(Duration::from_millis(200)),
        )
        .unwrap_err();
        assert!(error.contains("was stopped after"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn execute_succeeds_on_zero_exit() {
        assert!(execute(&sh("echo done"), &ProgressBar::hidden(), None).is_ok());
    }
}
