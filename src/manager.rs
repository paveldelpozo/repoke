use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Package {
    pub name: String,
    pub current: String,
    pub latest: String,
}

/// Commands run in order for some packages, which are reported together as upgraded or failed.
#[derive(Debug, PartialEq)]
pub struct Step {
    /// Arguments for the manager; `Manager::invocation` supplies the program.
    pub commands: Vec<Vec<String>>,
    pub packages: Vec<Package>,
}

impl Step {
    pub fn new<S: Into<String>>(args: impl IntoIterator<Item = S>, packages: &[Package]) -> Self {
        Self {
            commands: vec![args.into_iter().map(Into::into).collect()],
            packages: packages.to_vec(),
        }
    }
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
    let program = program.as_ref();
    // stdin is closed so a manager that wants to prompt fails instead of hanging an agent.
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run {}: {e}", program.display()))?;
    Ok(Output {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
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
