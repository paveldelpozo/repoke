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

pub trait Manager: Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String>;
    /// Arguments for `binary` that upgrade exactly these packages to their latest version.
    fn upgrade_args(&self, packages: &[&str]) -> Vec<String>;

    fn supported(&self) -> bool {
        true
    }

    fn version(&self, bin: &Path) -> Option<String> {
        let out = capture(bin, ["--version"]).ok()?;
        out.stdout.lines().next().map(|l| l.trim().to_string())
    }

    /// Updates the local package index so `outdated` sees the newest releases.
    fn refresh(&self, _bin: &Path) -> Result<(), String> {
        Ok(())
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        false
    }
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

/// Permission bits alone don't say whether the current user can write, `test -w` does.
pub fn writable(path: &Path) -> bool {
    Command::new("test")
        .arg("-w")
        .arg(path)
        .status()
        .is_ok_and(|s| s.success())
}
