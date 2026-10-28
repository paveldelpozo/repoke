//! `repoke schedule`: a periodic run registered with the OS scheduler (a launchd agent, a systemd
//! user timer or the Windows Task Scheduler). The scheduler calls `repoke schedule run`, which
//! runs check or upgrade, logs the result and sends a notification when something happened.

use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use clap::ValueEnum;
use console::style;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config;
use crate::manager::{self, capture};
use crate::snapshot::{self, now};
use crate::{ScheduleInstallArgs, print_json};

/// launchd label; systemd units and the Windows task are named `repoke`.
const LABEL: &str = "repoke.schedule";
const TASK: &str = "repoke";

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Every {
    Daily,
    Weekly,
}

/// What the job does, saved next to the config so the scheduler entry only says `schedule run`.
#[derive(Serialize, Deserialize)]
struct Settings {
    every: Every,
    /// `HH:MM`, local time. Weekly runs are on Mondays.
    at: String,
    check_only: bool,
    #[serde(default)]
    only: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

impl Settings {
    fn file() -> Result<PathBuf, String> {
        Ok(config::dir()
            .ok_or("cannot find the config directory")?
            .join("schedule.toml"))
    }

    fn load() -> Result<Option<Self>, String> {
        let file = Self::file()?;
        match fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text)
                .map(Some)
                .map_err(|e| format!("{}: {e}", file.display())),
            Err(_) => Ok(None),
        }
    }

    fn describe(&self) -> String {
        let when = match self.every {
            Every::Daily => format!("daily at {}", self.at),
            Every::Weekly => format!("weekly on Mondays at {}", self.at),
        };
        let what = if self.check_only {
            "checks for updates and notifies"
        } else {
            "upgrades everything that needs no sudo"
        };
        format!("{when}, {what}")
    }
}

#[derive(Serialize, Deserialize)]
struct HistoryEntry {
    started: u64,
    check_only: bool,
    exit: i32,
    message: String,
}

pub fn install(args: &ScheduleInstallArgs) -> Result<ExitCode, String> {
    let (hour, minute) = parse_at(&args.at)?;
    let exe = scheduled_exe()?;
    if exe.components().any(|c| c.as_os_str() == "target") {
        eprintln!(
            "{}",
            style(format!(
                "repoke is running from a build folder ({}); the job breaks if that build goes away. Install it first, e.g. `cargo install --path .`.",
                exe.display()
            ))
            .yellow()
        );
    }
    let settings = Settings {
        every: args.every,
        at: format!("{hour:02}:{minute:02}"),
        check_only: args.check_only,
        only: args.managers.only.clone(),
        exclude: args.managers.exclude.clone(),
    };
    let path = job_path(&env::var_os("PATH").unwrap_or_default());

    let plan = plan(&exe, &path, &settings, hour, minute)?;
    if args.dry_run {
        for (file, contents) in &plan.files {
            println!(
                "{}\n{contents}",
                style(format!("# {}", file.display())).dim()
            );
        }
        for command in &plan.commands {
            println!("$ {}", command.join(" "));
        }
        return Ok(ExitCode::SUCCESS);
    }

    for (file, contents) in &plan.files {
        write(file, contents)?;
    }
    for command in &plan.before {
        // Unloading a job that isn't loaded fails, which is fine on a first install.
        let _ = capture(&command[0], &command[1..]);
    }
    for command in &plan.commands {
        capture(&command[0], &command[1..])?
            .expect(&[0])
            .map_err(|e| format!("{}: {e}", command.join(" ")))?;
    }
    let settings_file = Settings::file()?;
    write(
        &settings_file,
        &toml::to_string_pretty(&settings).map_err(|e| e.to_string())?,
    )?;

    println!("{} {}.", style("Scheduled:").bold(), settings.describe());
    println!(
        "{}",
        style(format!(
            "Logs go to {}. See `repoke schedule status`, or run the job now with `repoke schedule run`.",
            state_dir()?.display()
        ))
        .dim()
    );
    Ok(ExitCode::SUCCESS)
}

/// The path the job runs. Homebrew keeps the real file in a folder per version and deletes the
/// old one on upgrade, so the job uses the `repoke` on the PATH when it is this same binary.
fn scheduled_exe() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("cannot find repoke's own path: {e}"))?;
    let real = fs::canonicalize(&exe).unwrap_or(exe);
    Ok(manager::locate("repoke")
        .filter(|on_path| fs::canonicalize(on_path).ok().as_ref() == Some(&real))
        .unwrap_or(real))
}

pub fn remove(dry_run: bool) -> Result<ExitCode, String> {
    let (files, commands) = removal()?;
    if dry_run {
        for command in &commands {
            println!("$ {}", command.join(" "));
        }
        for file in &files {
            println!("rm {}", file.display());
        }
        return Ok(ExitCode::SUCCESS);
    }
    let installed = files.iter().any(|f| f.exists()) || Settings::load()?.is_some();
    if !installed {
        println!("No schedule is installed.");
        return Ok(ExitCode::SUCCESS);
    }
    for command in &commands {
        let _ = capture(&command[0], &command[1..]);
    }
    for file in files.iter().chain([&Settings::file()?]) {
        if file.exists() {
            fs::remove_file(file).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    if cfg!(target_os = "linux") {
        let _ = capture("systemctl", ["--user", "daemon-reload"]);
    }
    println!("Schedule removed.");
    Ok(ExitCode::SUCCESS)
}

#[derive(Serialize)]
struct Status {
    installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    every: Option<Every>,
    #[serde(skip_serializing_if = "Option::is_none")]
    at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    check_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_run: Option<HistoryEntry>,
    log: PathBuf,
}

pub fn status(json: bool) -> Result<ExitCode, String> {
    let settings = Settings::load()?;
    let (files, _) = removal()?;
    let registered = if cfg!(windows) {
        capture("schtasks", ["/Query", "/TN", TASK]).is_ok_and(|o| o.code == Some(0))
    } else {
        files.iter().any(|f| f.exists())
    };
    let next_run = if cfg!(target_os = "linux") && registered {
        manager::lines(
            "systemctl",
            ["--user", "list-timers", "repoke.timer", "--no-legend"],
        )
        .ok()
        .and_then(|lines| lines.into_iter().next())
    } else {
        None
    };
    let log = state_dir()?;
    let last_run = fs::read_to_string(log.join("history.jsonl"))
        .ok()
        .and_then(|text| {
            text.lines()
                .rev()
                .find_map(|l| serde_json::from_str::<HistoryEntry>(l).ok())
        });

    let status = Status {
        installed: registered && settings.is_some(),
        every: settings.as_ref().map(|s| s.every),
        at: settings.as_ref().map(|s| s.at.clone()),
        check_only: settings.as_ref().map(|s| s.check_only),
        next_run,
        last_run,
        log: log.join("last-run.log"),
    };
    if json {
        print_json(&status);
        return Ok(ExitCode::SUCCESS);
    }

    match (&settings, registered) {
        (Some(s), true) => println!("{} {}.", style("Scheduled:").bold(), s.describe()),
        (Some(_), false) => println!(
            "{}",
            style("A schedule is saved but not registered with the system scheduler; run `repoke schedule install` again.").yellow()
        ),
        (None, _) => println!("No schedule is installed. Set one up with `repoke schedule install`."),
    }
    if let Some(next) = &status.next_run {
        println!("Next run: {next}");
    }
    match &status.last_run {
        Some(run) => println!(
            "Last run: {}, {} {}",
            utc(run.started),
            run.message,
            style(format!("(log: {})", status.log.display())).dim()
        ),
        None if settings.is_some() => println!("It hasn't run yet."),
        None => {}
    }
    Ok(ExitCode::SUCCESS)
}

/// What the scheduler runs. It calls this same binary with `--json` and reads the events.
pub fn run_job() -> Result<ExitCode, String> {
    let settings = Settings::load()?
        .ok_or("no schedule is installed, set one up with `repoke schedule install`")?;
    let exe = env::current_exe().map_err(|e| format!("cannot find repoke's own path: {e}"))?;
    let mut args: Vec<String> = if settings.check_only {
        vec!["check".into(), "--json".into()]
    } else {
        vec![
            "upgrade".into(),
            "--yes".into(),
            "--no-sudo".into(),
            "--json".into(),
        ]
    };
    if !settings.only.is_empty() {
        args.extend(["--only".into(), settings.only.join(",")]);
    }
    if !settings.exclude.is_empty() {
        args.extend(["--exclude".into(), settings.exclude.join(",")]);
    }

    let started = now();
    let out = Command::new(&exe)
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run {}: {e}", exe.display()))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let code = out.status.code().unwrap_or(1);
    let (message, notify_user) = if settings.check_only {
        summarize_check(&stdout)
    } else {
        summarize_upgrade(&stdout)
    };
    let message = match code {
        2 => format!("repoke could not start: {}", stderr.trim()),
        75 => "skipped, another repoke upgrade was already running".to_string(),
        _ => message,
    };

    let dir = state_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    write(
        &dir.join("last-run.log"),
        &format!("$ repoke {}\n{stdout}{stderr}", args.join(" ")),
    )?;
    let entry = HistoryEntry {
        started,
        check_only: settings.check_only,
        exit: code,
        message: message.clone(),
    };
    let history = dir.join("history.jsonl");
    trim_history(&history);
    let mut history = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history)
        .map_err(|e| e.to_string())?;
    writeln!(
        history,
        "{}",
        serde_json::to_string(&entry).expect("serializable entry")
    )
    .map_err(|e| e.to_string())?;

    if notify_user || code == 2 {
        notify(&message);
    }
    println!("{message}");
    Ok(ExitCode::from(u8::try_from(code).unwrap_or(1)))
}

/// Runs kept in `history.jsonl`: more than two years of daily runs. Only the last one is read.
const HISTORY_RUNS: usize = 1000;

/// Drops the oldest runs once the history is full; a failure here only leaves it longer.
fn trim_history(path: &Path) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() >= HISTORY_RUNS {
        let kept = &lines[lines.len() + 1 - HISTORY_RUNS..];
        let _ = fs::write(path, kept.join("\n") + "\n");
    }
}

/// The message for the log and the notification, and whether it is worth a notification.
fn summarize_upgrade(stdout: &str) -> (String, bool) {
    let events: Vec<Value> = stdout
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let managers = |event: &str| -> Vec<String> {
        let mut names: Vec<String> = events
            .iter()
            .filter(|e| e["event"] == event)
            .filter_map(|e| e["manager"].as_str().map(String::from))
            .collect();
        names.dedup();
        names
    };
    let checks_failed = managers("check_failed");
    let Some(summary) = events.iter().find(|e| e["event"] == "summary") else {
        return (
            "the upgrade ended without a summary, see the log".into(),
            true,
        );
    };
    let count = |field: &str| summary[field].as_u64().unwrap_or(0);
    let (done, failed, skipped) = (count("done"), count("failed"), count("skipped"));

    let mut parts = Vec::new();
    if done > 0 {
        parts.push(format!("upgraded {}", packages(done)));
    }
    if failed > 0 {
        parts.push(format!(
            "{failed} failed ({})",
            managers("failed").join(", ")
        ));
    }
    if skipped > 0 {
        parts.push(format!(
            "{skipped} skipped, they need sudo ({})",
            managers("skipped").join(", ")
        ));
    }
    if !checks_failed.is_empty() {
        parts.push(format!("could not check {}", checks_failed.join(", ")));
    }
    if parts.is_empty() {
        return ("everything was up to date".into(), false);
    }
    (capitalize(&parts.join(", ")), true)
}

fn summarize_check(stdout: &str) -> (String, bool) {
    let Ok(results) = serde_json::from_str::<Vec<Value>>(stdout) else {
        return ("the check ended without results, see the log".into(), true);
    };
    let pending: Vec<(&str, usize)> = results
        .iter()
        .filter_map(|r| {
            let n = r["packages"].as_array().map_or(0, Vec::len);
            (n > 0).then(|| (r["id"].as_str().unwrap_or("?"), n))
        })
        .collect();
    let failed: Vec<&str> = results
        .iter()
        .filter(|r| !r["error"].is_null())
        .filter_map(|r| r["id"].as_str())
        .collect();
    let total: usize = pending.iter().map(|(_, n)| n).sum();
    let mut message = if total == 0 {
        "everything is up to date".to_string()
    } else {
        let names: Vec<&str> = pending.iter().map(|(id, _)| *id).collect();
        format!(
            "{} available ({}), run `repoke upgrade`",
            plural_updates(total),
            names.join(", ")
        )
    };
    if !failed.is_empty() {
        message.push_str(&format!("; could not check {}", failed.join(", ")));
    }
    let notify_user = total > 0 || !failed.is_empty();
    (capitalize(&message), notify_user)
}

fn packages(n: u64) -> String {
    if n == 1 {
        "1 package".into()
    } else {
        format!("{n} packages")
    }
}

fn plural_updates(n: usize) -> String {
    if n == 1 {
        "1 update".into()
    } else {
        format!("{n} updates")
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn notify(message: &str) {
    if cfg!(target_os = "macos") {
        let escaped = message.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{escaped}\" with title \"repoke\"");
        let _ = capture("osascript", ["-e", &script]);
    } else if cfg!(target_os = "linux") && manager::locate("notify-send").is_some() {
        let _ = capture("notify-send", ["repoke", message]);
    }
}

/// `HH:MM` on a 24-hour clock.
fn parse_at(at: &str) -> Result<(u8, u8), String> {
    let invalid = || format!("--at takes a 24-hour time like 09:00, got '{at}'");
    let (h, m) = at.split_once(':').ok_or_else(invalid)?;
    let (h, m): (u8, u8) = (
        h.parse().map_err(|_| invalid())?,
        m.parse().map_err(|_| invalid())?,
    );
    if h > 23 || m > 59 {
        return Err(invalid());
    }
    Ok((h, m))
}

/// `YYYY-MM-DD HH:MM UTC`; repoke has no time zone database.
fn utc(unix: u64) -> String {
    let minutes = unix % 86_400 / 60;
    format!(
        "{} {:02}:{:02} UTC",
        snapshot::date(unix),
        minutes / 60,
        minutes % 60
    )
}

/// The PATH the job runs with. Schedulers start jobs with a bare PATH where brew, npm and the rest
/// aren't found, so the user's PATH is saved, without duplicates and with fnm's per-shell folder
/// (gone once that shell closes) swapped for fnm's stable `default` alias.
fn job_path(path: &OsString) -> String {
    let fnm_default = fnm_dir().map(|d| d.join("aliases").join("default").join("bin"));
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in env::split_paths(path) {
        let entry = if entry
            .components()
            .any(|c| c.as_os_str() == "fnm_multishells")
        {
            match &fnm_default {
                Some(stable) if stable.is_dir() => stable.clone(),
                _ => continue,
            }
        } else {
            entry
        };
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    env::join_paths(entries)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn fnm_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("FNM_DIR") {
        return Some(dir.into());
    }
    let home = snapshot::home()?;
    Some(if cfg!(target_os = "macos") {
        home.join("Library/Application Support/fnm")
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("fnm")
    })
}

/// Where run logs, run history and the update check live: `~/Library/Logs/repoke`,
/// `~/.local/state/repoke` or `%LOCALAPPDATA%\repoke`.
pub fn state_dir() -> Result<PathBuf, String> {
    let home = snapshot::home().ok_or("cannot find the home directory")?;
    Ok(if cfg!(target_os = "macos") {
        home.join("Library/Logs/repoke")
    } else if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("repoke")
    } else {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"))
            .join("repoke")
    })
}

struct Plan {
    files: Vec<(PathBuf, String)>,
    /// Commands whose failure is expected, such as unloading a job that was never loaded.
    before: Vec<Vec<String>>,
    commands: Vec<Vec<String>>,
}

fn plan(exe: &Path, path: &str, settings: &Settings, hour: u8, minute: u8) -> Result<Plan, String> {
    let home = snapshot::home().ok_or("cannot find the home directory")?;
    let exe_s = exe.display().to_string();
    if cfg!(target_os = "macos") {
        let file = home.join(format!("Library/LaunchAgents/{LABEL}.plist"));
        let uid = manager::lines("id", ["-u"])?
            .into_iter()
            .next()
            .ok_or("cannot read the user id")?;
        let logs = state_dir()?.join("launchd.log");
        let domain = format!("gui/{uid}");
        Ok(Plan {
            files: vec![(
                file.clone(),
                plist(
                    &exe_s,
                    path,
                    &logs.display().to_string(),
                    settings.every,
                    hour,
                    minute,
                ),
            )],
            before: vec![vec![
                "launchctl".into(),
                "bootout".into(),
                format!("{domain}/{LABEL}"),
            ]],
            commands: vec![vec![
                "launchctl".into(),
                "bootstrap".into(),
                domain,
                file.display().to_string(),
            ]],
        })
    } else if cfg!(target_os = "linux") {
        if manager::locate("systemctl").is_none() {
            let (dom, dow) = match settings.every {
                Every::Daily => ("*", "*"),
                Every::Weekly => ("*", "1"),
            };
            return Err(format!(
                "systemd isn't available here; add this line to your crontab (`crontab -e`) instead:\n{minute} {hour} {dom} * {dow} PATH=\"{path}\" \"{exe_s}\" schedule run"
            ));
        }
        let dir = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("systemd/user");
        let systemctl = |args: &[&str]| -> Vec<String> {
            ["systemctl", "--user"]
                .iter()
                .chain(args)
                .map(|s| s.to_string())
                .collect()
        };
        Ok(Plan {
            files: vec![
                (dir.join("repoke.service"), systemd_service(&exe_s, path)),
                (
                    dir.join("repoke.timer"),
                    systemd_timer(settings.every, hour, minute),
                ),
            ],
            before: Vec::new(),
            commands: vec![
                systemctl(&["daemon-reload"]),
                systemctl(&["enable", "--now", "repoke.timer"]),
            ],
        })
    } else if cfg!(windows) {
        Ok(Plan {
            files: Vec::new(),
            before: Vec::new(),
            commands: vec![schtasks(&exe_s, settings.every, hour, minute)],
        })
    } else {
        Err("scheduling is supported on macOS, Linux and Windows".into())
    }
}

/// Files and commands that undo `install`.
fn removal() -> Result<(Vec<PathBuf>, Vec<Vec<String>>), String> {
    let home = snapshot::home().ok_or("cannot find the home directory")?;
    let strings = |args: &[&str]| args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if cfg!(target_os = "macos") {
        let uid = manager::lines("id", ["-u"])?
            .into_iter()
            .next()
            .unwrap_or_default();
        Ok((
            vec![home.join(format!("Library/LaunchAgents/{LABEL}.plist"))],
            vec![strings(&[
                "launchctl",
                "bootout",
                &format!("gui/{uid}/{LABEL}"),
            ])],
        ))
    } else if cfg!(target_os = "linux") {
        let dir = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("systemd/user");
        Ok((
            vec![dir.join("repoke.timer"), dir.join("repoke.service")],
            vec![strings(&[
                "systemctl",
                "--user",
                "disable",
                "--now",
                "repoke.timer",
            ])],
        ))
    } else {
        Ok((
            Vec::new(),
            vec![strings(&["schtasks", "/Delete", "/TN", TASK, "/F"])],
        ))
    }
}

fn write(file: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(file, contents).map_err(|e| format!("{}: {e}", file.display()))
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn plist(exe: &str, path: &str, log: &str, every: Every, hour: u8, minute: u8) -> String {
    // Weekday 1 is Monday. A run missed while the Mac sleeps happens on wake.
    let weekday = match every {
        Every::Daily => String::new(),
        Every::Weekly => "\n        <key>Weekday</key>\n        <integer>1</integer>".into(),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>schedule</string>
        <string>run</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>
    </dict>
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>{hour}</integer>
        <key>Minute</key>
        <integer>{minute}</integer>{weekday}
    </dict>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        exe = xml(exe),
        path = xml(path),
        log = xml(log),
    )
}

fn systemd_service(exe: &str, path: &str) -> String {
    format!(
        "[Unit]\nDescription=repoke: keep package managers up to date\n\n[Service]\nType=oneshot\nEnvironment=\"PATH={path}\"\nExecStart=\"{exe}\" schedule run\n"
    )
}

/// `Persistent=true` runs a time missed while the machine was off at the next boot.
fn systemd_timer(every: Every, hour: u8, minute: u8) -> String {
    let day = match every {
        Every::Daily => "*-*-*",
        Every::Weekly => "Mon *-*-*",
    };
    format!(
        "[Unit]\nDescription=Run repoke on a schedule\n\n[Timer]\nOnCalendar={day} {hour:02}:{minute:02}:00\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
    )
}

fn schtasks(exe: &str, every: Every, hour: u8, minute: u8) -> Vec<String> {
    let mut args: Vec<String> = ["schtasks", "/Create", "/F", "/TN", TASK]
        .map(String::from)
        .to_vec();
    match every {
        Every::Daily => args.extend(["/SC".into(), "DAILY".into()]),
        Every::Weekly => args.extend(["/SC".into(), "WEEKLY".into(), "/D".into(), "MON".into()]),
    }
    args.extend([
        "/ST".into(),
        format!("{hour:02}:{minute:02}"),
        "/TR".into(),
        format!("\"{exe}\" schedule run"),
    ]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_the_latest_runs() {
        let path = env::temp_dir().join(format!("repoke-history-{}.jsonl", std::process::id()));
        let runs: String = (0..HISTORY_RUNS + 5).map(|i| format!("{i}\n")).collect();
        fs::write(&path, runs).unwrap();
        trim_history(&path);
        let text = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // One short of the limit, so the run being recorded makes it full.
        assert_eq!(lines.len(), HISTORY_RUNS - 1);
        assert_eq!(lines.last(), Some(&"1004"));
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_at("09:00"), Ok((9, 0)));
        assert_eq!(parse_at("23:59"), Ok((23, 59)));
        assert!(parse_at("24:00").is_err());
        assert!(parse_at("9").is_err());
    }

    #[test]
    fn plist_escapes_and_schedules() {
        let text = plist(
            "/Users/me/.cargo/bin/repoke",
            "/a&b:/usr/bin",
            "/tmp/log",
            Every::Weekly,
            7,
            5,
        );
        assert!(text.contains("<string>/a&amp;b:/usr/bin</string>"));
        assert!(text.contains("<key>Hour</key>\n        <integer>7</integer>"));
        assert!(text.contains("<key>Weekday</key>\n        <integer>1</integer>"));
        assert!(!plist("x", "p", "l", Every::Daily, 9, 0).contains("Weekday"));
    }

    #[test]
    fn systemd_timer_runs_on_mondays_when_weekly() {
        assert!(systemd_timer(Every::Weekly, 9, 30).contains("OnCalendar=Mon *-*-* 09:30:00"));
        assert!(systemd_timer(Every::Daily, 9, 0).contains("OnCalendar=*-*-* 09:00:00"));
    }

    #[test]
    fn schtasks_command() {
        let args = schtasks(r"C:\bin\repoke.exe", Every::Weekly, 9, 0);
        assert_eq!(
            args.join(" "),
            r#"schtasks /Create /F /TN repoke /SC WEEKLY /D MON /ST 09:00 /TR "C:\bin\repoke.exe" schedule run"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn job_path_drops_fnm_shell_folders_and_duplicates() {
        let path = OsString::from(
            "/opt/homebrew/bin:/Users/me/.local/state/fnm_multishells/8253_1/bin:/usr/bin:/opt/homebrew/bin",
        );
        let cleaned = job_path(&path);
        assert!(!cleaned.contains("fnm_multishells"));
        assert_eq!(cleaned.matches("/opt/homebrew/bin").count(), 1);
        assert!(cleaned.contains("/usr/bin"));
    }

    #[test]
    fn summarizes_upgrade_events() {
        let stdout = [
            r#"{"event":"done","action":"upgrade","manager":"npm","packages":[],"seconds":1.0}"#,
            r#"{"event":"failed","action":"upgrade","manager":"composer","error":"x","packages":[],"seconds":1.0}"#,
            r#"{"event":"summary","action":"upgrade","done":5,"failed":1,"skipped":0,"planned":0,"seconds":2.0,"elapsed_seconds":3.0,"managers":[]}"#,
        ]
        .join("\n");
        assert_eq!(
            summarize_upgrade(&stdout),
            ("Upgraded 5 packages, 1 failed (composer)".to_string(), true)
        );
        let quiet = r#"{"event":"summary","action":"upgrade","done":0,"failed":0,"skipped":0,"planned":0,"seconds":0.0,"elapsed_seconds":1.0,"managers":[]}"#;
        assert!(!summarize_upgrade(quiet).1);
    }

    #[test]
    fn summarizes_check_results() {
        let stdout = r#"[{"id":"npm","packages":[{"name":"a"},{"name":"b"}],"error":null},{"id":"brew","packages":[],"error":null}]"#;
        assert_eq!(
            summarize_check(stdout),
            (
                "2 updates available (npm), run `repoke upgrade`".to_string(),
                true
            )
        );
    }

    #[test]
    fn formats_utc_times() {
        assert_eq!(
            utc(1_761_652_800 + 9 * 3600 + 5 * 60),
            "2025-10-28 21:05 UTC"
        );
    }
}
