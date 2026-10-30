use std::env;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub current: String,
    pub latest: String,
}

/// A package as `export` records it and `init` installs it, written `kind:name==version` so a
/// snapshot stays one readable line per package. `==` because names use `@` themselves
/// (`python@3.12`, `@scope/pkg`). `kind` is the manager's own detail: a brew cask
/// or tap, a flatpak remote, a scoop bucket, an App Store id, the default Node line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Installed {
    pub name: String,
    pub version: Option<String>,
    pub kind: Option<String>,
}

impl Installed {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            kind: None,
        }
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into()).filter(|v| !v.is_empty());
        self
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn is(&self, kind: &str) -> bool {
        self.kind.as_deref() == Some(kind)
    }
}

impl fmt::Display for Installed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(kind) = &self.kind {
            write!(f, "{kind}:")?;
        }
        f.write_str(&self.name)?;
        if let Some(version) = &self.version {
            write!(f, "=={version}")?;
        }
        Ok(())
    }
}

impl From<Installed> for String {
    fn from(installed: Installed) -> Self {
        installed.to_string()
    }
}

impl TryFrom<String> for Installed {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let (kind, rest) = match text.split_once(':') {
            Some((kind, rest)) => (Some(kind.to_string()), rest),
            None => (None, text.as_str()),
        };
        let (name, version) = match rest.split_once("==") {
            Some((name, version)) => (name, Some(version.to_string())),
            None => (rest, None),
        };
        if name.is_empty() {
            return Err(format!("empty package name in '{text}'"));
        }
        Ok(Self {
            name: name.to_string(),
            version,
            kind,
        })
    }
}

impl From<&Installed> for Package {
    fn from(installed: &Installed) -> Self {
        Self {
            name: installed.name.clone(),
            current: String::new(),
            latest: installed.version.clone().unwrap_or_default(),
        }
    }
}

/// Commands run in order for some packages, which are reported together as upgraded or failed.
#[derive(Debug, PartialEq)]
pub struct Step {
    /// Arguments for the manager; `Manager::invocation` supplies the program.
    pub commands: Vec<Vec<String>>,
    pub packages: Vec<Package>,
    /// The commands may ask for an administrator password on their own, as some brew casks do.
    pub password: bool,
}

impl Step {
    pub fn new<S: Into<String>>(args: impl IntoIterator<Item = S>, packages: &[Package]) -> Self {
        Self {
            commands: vec![args.into_iter().map(Into::into).collect()],
            packages: packages.to_vec(),
            password: false,
        }
    }

    pub fn with_password(mut self) -> Self {
        self.password = true;
        self
    }

    pub fn install<S: Into<String>>(
        args: impl IntoIterator<Item = S>,
        packages: &[Installed],
    ) -> Self {
        Self {
            commands: vec![args.into_iter().map(Into::into).collect()],
            packages: packages.iter().map(Package::from).collect(),
            password: false,
        }
    }

    /// `args` followed by every package name, as one step; none when there is nothing to install.
    pub fn install_all(args: &[&str], packages: &[Installed]) -> Vec<Self> {
        if packages.is_empty() {
            return Vec::new();
        }
        let names = packages.iter().map(|p| p.name.clone());
        let args = args.iter().map(|a| a.to_string()).chain(names);
        vec![Self::install(args, packages)]
    }
}

/// Everything a step's commands printed, unfiltered.
#[derive(Debug, Default)]
pub struct Printed {
    pub stdout: String,
    pub stderr: String,
}

/// A package whose install scripts were blocked: why, and how the user can run them.
#[derive(Debug, PartialEq)]
pub struct Blocked {
    pub package: Package,
    pub reason: String,
    pub hint: String,
}

pub trait Manager: Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String>;
    /// Commands that upgrade exactly these packages to the `latest` that `outdated` reported.
    fn upgrade_steps(&self, bin: &Path, packages: &[Package]) -> Vec<Step>;

    fn supported(&self) -> bool {
        true
    }

    fn locate(&self) -> Option<PathBuf> {
        locate(self.binary())
    }

    /// Program and leading arguments that run the manager.
    fn invocation(&self, bin: &Path) -> Vec<String> {
        vec![bin.display().to_string()]
    }

    fn version(&self, bin: &Path) -> Option<String> {
        version_in(&capture(bin, ["--version"]).ok()?.stdout)
    }

    /// Updates the local package index so `outdated` sees the newest releases.
    fn refresh(&self, _bin: &Path) -> Result<(), String> {
        Ok(())
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        false
    }

    /// Packages that would not work here, each with the reason, so they are skipped up front
    /// instead of failing halfway through the upgrade.
    fn incompatible(&self, _bin: &Path, _packages: &[Package]) -> Vec<(Package, String)> {
        Vec::new()
    }

    /// A suggestion for a failure the manager recognises, shown under the error.
    fn hint(&self, _packages: &[Package], _error: &str) -> Option<String> {
        None
    }

    /// Packages a step installed while the manager refused to run their install scripts, read
    /// from what its commands printed. They count as blocked, not upgraded: they may not work.
    fn blocked(&self, _packages: &[Package], _printed: &Printed) -> Vec<Blocked> {
        Vec::new()
    }

    /// How long one of its commands may run: `[timeouts]` in the config, else no limit for
    /// built-in managers (plugins fall back to `plugin_timeout`).
    fn timeout(&self) -> Option<Duration> {
        crate::config::timeout_for(self.id()).flatten()
    }

    /// Whether `refresh` does anything, so the spinner only says so when it does.
    fn has_refresh(&self) -> bool {
        false
    }

    /// What the user installed on purpose, leaving out dependencies, for `export`.
    fn installed(&self, _bin: &Path) -> Result<Vec<Installed>, String> {
        Err("export is not supported for this manager yet".into())
    }

    fn install_steps(&self, _bin: &Path, _packages: &[Installed]) -> Vec<Step> {
        Vec::new()
    }

    /// Config files, relative to the home directory, that `export` can carry over. Files holding
    /// credentials (`~/.cargo/credentials.toml`, composer's `auth.json`) are left out on purpose.
    fn config_files(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Trimmed, non-empty lines of a command that must exit 0.
pub fn lines<I, S>(program: impl AsRef<OsStr>, args: I) -> Result<Vec<String>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(capture(program, args)?
        .expect(&[0])?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// First token that looks like a version: `pip 26.2.2 from /opt/...` gives `26.2.2`.
pub fn version_in(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|t| t.trim_start_matches('v').trim_end_matches([',', ';']))
        .find(|t| t.starts_with(|c: char| c.is_ascii_digit()) && t.contains('.'))
        .map(String::from)
}

pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// Several managers exit 1 when they find outdated packages, so callers say which codes are fine.
    pub fn expect(self, ok_codes: &[i32]) -> Result<String, String> {
        match self.code {
            Some(code) if ok_codes.contains(&code) => Ok(self.stdout),
            code => {
                let lines: Vec<&str> = self.stderr.trim().lines().collect();
                let tail = lines[lines.len().saturating_sub(20)..].join("\n");
                Err(format!(
                    "exited with {}: {tail}",
                    code.map_or("a signal".to_string(), |c| c.to_string())
                ))
            }
        }
    }
}

pub fn capture<I, S>(program: impl AsRef<OsStr>, args: I) -> Result<Output, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    capture_within(program, args, CHECK_TIMEOUT.get())
}

thread_local! {
    /// Managers call `capture` themselves while checking, so the limit reaches them this way.
    static CHECK_TIMEOUT: std::cell::Cell<Option<Duration>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with every `capture` on this thread stopped after `timeout`.
pub fn within<R>(timeout: Option<Duration>, f: impl FnOnce() -> R) -> R {
    CHECK_TIMEOUT.set(timeout);
    let out = f();
    CHECK_TIMEOUT.set(None);
    out
}

/// Like `capture`, but the program is stopped once `timeout` passes.
pub fn capture_within<I, S>(
    program: impl AsRef<OsStr>,
    args: I,
    timeout: Option<Duration>,
) -> Result<Output, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let program = program.as_ref();
    // stdin is closed so a manager that wants to prompt fails instead of hanging an agent.
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null());
    let Some(limit) = timeout else {
        let out = command
            .output()
            .map_err(|e| format!("could not run {}: {e}", program.display()))?;
        return Ok(Output {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    };
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {}: {e}", program.display()))?;
    let read = |mut pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let stdout = read(Box::new(child.stdout.take().expect("stdout is piped")));
    let stderr = read(Box::new(child.stderr.take().expect("stderr is piped")));
    let status = wait_within(&mut child, Some(limit), program)?;
    Ok(Output {
        code: status.code(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Waits for `child`, killing it once `timeout` passes. Readers of its output are left alone
/// then: a grandchild may still hold the pipes open.
pub fn wait_within(
    child: &mut Child,
    timeout: Option<Duration>,
    program: &OsStr,
) -> Result<ExitStatus, String> {
    let failed = |e: std::io::Error| format!("could not wait for {}: {e}", program.display());
    let Some(limit) = timeout else {
        return child.wait().map_err(failed);
    };
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().map_err(failed)? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{} was stopped after {}, its time limit",
                program.display(),
                crate::config::show_duration(limit)
            ));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn locate(bin: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    env::split_paths(&env::var_os("PATH")?).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{bin}{ext}")))
            .find(|p| p.is_file())
    })
}

pub fn is_root() -> bool {
    capture("id", ["-u"]).is_ok_and(|o| o.stdout.trim() == "0")
}

/// For system package managers: sudo is needed unless repoke already runs as root.
pub fn needs_sudo() -> bool {
    cfg!(unix) && !is_root()
}

/// Permission bits alone don't say whether the current user can write, `test -w` does.
pub fn writable(path: &Path) -> bool {
    Command::new("test")
        .arg("-w")
        .arg(path)
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_round_trips_through_its_text_form() {
        for text in [
            "git",
            "python@3.10",
            "cask:visual-studio-code",
            "@google/gemini-cli==0.66.0",
            "default:node@22==22.23.4",
            "flathub:org.mozilla.firefox",
        ] {
            let installed = Installed::try_from(text.to_string()).unwrap();
            assert_eq!(installed.to_string(), text);
        }
        let scoped = Installed::try_from("@google/gemini-cli==0.66.0".to_string()).unwrap();
        assert_eq!(scoped.name, "@google/gemini-cli");
        assert_eq!(scoped.version.as_deref(), Some("0.66.0"));
        let versioned = Installed::try_from("python@3.10".to_string()).unwrap();
        assert_eq!(
            (versioned.name.as_str(), versioned.version),
            ("python@3.10", None)
        );
        assert!(Installed::try_from("cask:".to_string()).is_err());
    }

    #[test]
    fn finds_the_version_in_banners() {
        let cases = [
            (
                "pip 26.2.2 from /opt/homebrew/lib/python3.15 (python 3.15)",
                "26.2.2",
            ),
            ("Homebrew 7.0.11", "7.0.11"),
            ("fnm 1.39.0", "1.39.0"),
            ("Composer version 2.10.2 2026-07-01 11:24:45", "2.10.2"),
            ("v0.40.1", "0.40.1"),
        ];
        for (banner, version) in cases {
            assert_eq!(version_in(banner).as_deref(), Some(version), "{banner}");
        }
    }
}
