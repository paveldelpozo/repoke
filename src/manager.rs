use std::env;
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

    fn supported(&self) -> bool {
        true
    }

    fn version(&self, bin: &Path) -> Option<String> {
        let out = capture(bin, &["--version"]).ok()?;
        out.stdout.lines().next().map(|l| l.trim().to_string())
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
            code => Err(format!(
                "exited with {}: {}",
                code.map_or("a signal".to_string(), |c| c.to_string()),
                self.stderr.trim()
            )),
        }
    }
}

pub fn capture(bin: &Path, args: &[&str]) -> Result<Output, String> {
    // stdin is closed so a manager that wants to prompt fails instead of hanging an agent.
    let out = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run {}: {e}", bin.display()))?;
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
