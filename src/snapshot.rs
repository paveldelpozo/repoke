//! The folder `export` writes and `init --from` reads: `repoke.toml` plus `files/`, where config
//! files are copied at their path relative to the home directory.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::manager::{self, Installed};

pub const FILE: &str = "repoke.toml";
pub const FILES: &str = "files";
const FORMAT: u32 = 1;
pub const REDACTED: &str = "<removed by repoke>";

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub format: u32,
    pub os: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Unix seconds.
    pub created: u64,
    #[serde(default)]
    pub managers: BTreeMap<String, Entry>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub packages: Vec<Installed>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<ConfigFile>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    /// Relative to the home directory.
    pub path: String,
    /// Values of lines that looked like secrets were replaced with `REDACTED`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub redacted: bool,
    /// Kept in `files.age` instead of `files/`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub encrypted: bool,
    /// Set, instead of copying the file, when it is a link into a dotfiles repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dotfiles: Option<Dotfiles>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Dotfiles {
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl Snapshot {
    pub fn new() -> Self {
        Self {
            format: FORMAT,
            os: env::consts::OS.to_string(),
            host: hostname(),
            created: now(),
            managers: BTreeMap::new(),
        }
    }

    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join(FILE);
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let snapshot: Self =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if snapshot.format > FORMAT {
            return Err(format!(
                "{} was written by a newer repoke (format {}), update repoke first",
                path.display(),
                snapshot.format
            ));
        }
        Ok(snapshot)
    }

    pub fn write(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let path = dir.join(FILE);
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

pub fn home() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn hostname() -> Option<String> {
    let name = env::var("COMPUTERNAME").ok().or_else(|| {
        manager::lines("hostname", [] as [&str; 0])
            .ok()?
            .into_iter()
            .next()
    })?;
    Some(name.trim_end_matches(".local").to_string())
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `YYYY-MM-DD` in UTC, from Howard Hinnant's days-to-civil algorithm.
pub fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `~/…` for paths under the home directory.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

const SECRET_KEYS: [&str; 9] = [
    "token",
    "password",
    "passwd",
    "secret",
    "_auth",
    "apikey",
    "api_key",
    "api-key",
    "credential",
];

/// Line numbers (from 0) of `key = value` lines whose key sounds like a secret and whose value is
/// written out. `${VAR}` references hold no secret and are left alone.
pub fn secret_lines(text: &str) -> Vec<usize> {
    let mut in_key = false;
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            if line.contains("PRIVATE KEY-----") {
                in_key = line.contains("BEGIN");
                return Some(i);
            }
            if in_key {
                return Some(i);
            }
            let (key, value) = assignment(line)?;
            let key = key.to_lowercase();
            let value = value.trim().trim_matches(['"', '\'']);
            (SECRET_KEYS.iter().any(|k| key.contains(k))
                && !value.is_empty()
                && !value.starts_with('$')
                && value != REDACTED)
                .then_some(i)
        })
        .collect()
}

/// Replaces the value of every secret line, keeping the key so the file still shows what to fill.
pub fn redact(text: &str) -> String {
    let secrets = secret_lines(text);
    let mut out: Vec<String> = text
        .lines()
        .enumerate()
        .map(|(i, line)| {
            if !secrets.contains(&i) {
                return line.to_string();
            }
            match assignment(line) {
                Some((key, _)) => {
                    let sep = &line[key.len()..key.len() + 1];
                    format!("{key}{sep}{REDACTED}")
                }
                None => REDACTED.to_string(),
            }
        })
        .collect();
    if text.ends_with('\n') {
        out.push(String::new());
    }
    out.join("\n")
}

/// The snapshot's text with each removed value taken back from `current`, the file already on
/// this machine, when it has the same key. Restoring must never swap a real token for the
/// placeholder.
pub fn merge_secrets(snapshot: &str, current: &str) -> String {
    let mut out: Vec<String> = snapshot
        .lines()
        .map(|line| {
            let Some((key, _)) = assignment(line).filter(|(_, v)| v.trim() == REDACTED) else {
                return line.to_string();
            };
            current
                .lines()
                .find(|l| assignment(l).is_some_and(|(k, _)| k == key))
                .map_or_else(|| line.to_string(), String::from)
        })
        .collect();
    if snapshot.ends_with('\n') {
        out.push(String::new());
    }
    out.join("\n")
}

/// Splits at `=` (ini, npmrc, toml) or else `:` (yaml). npmrc keys hold `:` themselves
/// (`//registry/:_authToken=…`), which is why `=` wins.
fn assignment(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }
    let at = line.find('=').or_else(|| line.find(':'))?;
    Some((&line[..at], &line[at + 1..]))
}

/// A config that is a symlink into a git repository: the user's dotfiles.
pub fn dotfiles_link(path: &Path, home: &Path) -> Option<Dotfiles> {
    if !fs::symlink_metadata(path).ok()?.file_type().is_symlink() {
        return None;
    }
    let target = fs::canonicalize(path).ok()?;
    let repo = target
        .ancestors()
        .skip(1)
        .find(|dir| dir.join(".git").exists())?;
    let remote = manager::lines(
        "git",
        [
            "-C".as_ref(),
            repo.as_os_str(),
            "remote".as_ref(),
            "get-url".as_ref(),
            "origin".as_ref(),
        ],
    )
    .ok()
    .and_then(|lines| lines.into_iter().next());
    Some(Dotfiles {
        target: tilde(&target, home),
        remote,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_removes_secret_values() {
        let npmrc = "registry=https://registry.npmjs.org/\n//npm.pkg.github.com/:_authToken=ghp_abc123\n//other/:_authToken=${NODE_AUTH_TOKEN}\n# password=commented\nsave-exact=true\n";
        assert_eq!(secret_lines(npmrc), [1]);
        let redacted = redact(npmrc);
        assert!(redacted.contains(&format!("//npm.pkg.github.com/:_authToken={REDACTED}")));
        assert!(redacted.contains("${NODE_AUTH_TOKEN}"));
        assert!(!redacted.contains("ghp_abc123"));
        assert!(redacted.ends_with("save-exact=true\n"));
        assert!(secret_lines(&redacted).is_empty());
    }

    #[test]
    fn restoring_keeps_the_secrets_already_here() {
        let snapshot = format!(
            "registry=https://r/\n//gh/:_authToken={REDACTED}\n//new/:_authToken={REDACTED}\n"
        );
        let current = "//gh/:_authToken=ghp_real\nregistry=https://r/\n";
        let merged = merge_secrets(&snapshot, current);
        assert_eq!(
            merged,
            format!(
                "registry=https://r/\n//gh/:_authToken=ghp_real\n//new/:_authToken={REDACTED}\n"
            )
        );
    }

    #[test]
    fn flags_yaml_secrets_and_private_keys() {
        let text = "user: me\napi_key: \"sk-123\"\n-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----\nafter: 1\n";
        assert_eq!(secret_lines(text), [1, 2, 3, 4]);
    }

    #[test]
    fn formats_dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(1_761_652_800), "2025-10-28");
        assert_eq!(date(951_782_400), "2000-02-29");
    }

    #[test]
    fn snapshot_round_trips() {
        let mut snapshot = Snapshot::new();
        snapshot.managers.insert(
            "brew".into(),
            Entry {
                packages: vec![
                    Installed::new("git"),
                    Installed::new("firefox").kind("cask"),
                ],
                config: Vec::new(),
            },
        );
        let text = toml::to_string_pretty(&snapshot).unwrap();
        assert!(text.contains("\"cask:firefox\""), "{text}");
        let back: Snapshot = toml::from_str(&text).unwrap();
        assert_eq!(
            back.managers["brew"].packages[1].kind.as_deref(),
            Some("cask")
        );
    }
}
