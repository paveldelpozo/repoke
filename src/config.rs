use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;

/// `~/.config/repoke/config.toml`, or the file named by `REPOKE_CONFIG`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Managers left out unless named in `--only`.
    pub exclude: Vec<String>,
    /// Packages never upgraded nor reported as pending, as `name` or `manager:name`.
    pub hold: Vec<String>,
    /// How long a plugin command may run: seconds, or `90s`, `10m`, `1h`; `0` for no limit.
    pub plugin_timeout: Option<String>,
    /// The longest one of a manager's commands may run, by manager id, in the same format.
    pub timeouts: BTreeMap<String, String>,
    pub brew: Brew,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Brew {
    /// Also casks that update themselves (`brew outdated --greedy`), which brew skips by default.
    pub greedy: bool,
}

/// The config for code that has no way to receive it, like a manager. A broken file is reported
/// by `load` when the command starts, so here it just falls back to the defaults.
pub fn current() -> &'static Config {
    static CURRENT: OnceLock<Config> = OnceLock::new();
    CURRENT.get_or_init(|| load().unwrap_or_default())
}

pub fn load() -> Result<Config, String> {
    let Some(path) = path() else {
        return Ok(Config::default());
    };
    match fs::read_to_string(&path) {
        Ok(text) => {
            let config: Config =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            if let Some(timeout) = &config.plugin_timeout {
                parse_duration(timeout)
                    .map_err(|e| format!("{}: plugin_timeout: {e}", path.display()))?;
            }
            for (id, timeout) in &config.timeouts {
                parse_duration(timeout)
                    .map_err(|e| format!("{}: timeouts.{id}: {e}", path.display()))?;
            }
            Ok(config)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn path() -> Option<PathBuf> {
    if let Some(file) = env::var_os("REPOKE_CONFIG") {
        return Some(file.into());
    }
    Some(dir()?.join("config.toml"))
}

/// `~/.config/repoke`, or `%APPDATA%\repoke` on Windows.
pub fn dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env::var_os("APPDATA").map(PathBuf::from)
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    Some(base?.join("repoke"))
}

/// The limit set for this manager in `[timeouts]`, if there is one; `Some(None)` is no limit.
pub fn timeout_for(id: &str) -> Option<Option<Duration>> {
    current()
        .timeouts
        .get(id)
        .and_then(|t| parse_duration(t).ok())
}

/// `600`, `90s`, `10m` or `1h`; `0` means no limit and gives `None`.
pub fn parse_duration(text: &str) -> Result<Option<Duration>, String> {
    let text = text.trim();
    let (number, unit) = match text.find(|c: char| !c.is_ascii_digit()) {
        Some(i) => text.split_at(i),
        None => (text, "s"),
    };
    let invalid = || format!("'{text}' isn't a duration like 600, 90s, 10m or 1h");
    let value: u64 = number.parse().map_err(|_| invalid())?;
    let seconds = match unit {
        "s" => value,
        "m" => value * 60,
        "h" => value * 3600,
        _ => return Err(invalid()),
    };
    Ok((seconds > 0).then(|| Duration::from_secs(seconds)))
}

pub fn show_duration(duration: Duration) -> String {
    match duration.as_secs() {
        s if s % 3600 == 0 => format!("{}h", s / 3600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

pub fn held(hold: &[String], manager: &str, package: &str) -> bool {
    hold.iter()
        .any(|h| h == package || h.split_once(':') == Some((manager, package)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config() {
        let config: Config = toml::from_str(
            r#"
            exclude = ["brew"]
            hold = ["eslint", "pnpm:typescript"]

            [timeouts]
            brew = "30m"

            [brew]
            greedy = true
            "#,
        )
        .unwrap();
        assert_eq!(config.timeouts["brew"], "30m");
        assert_eq!(config.exclude, ["brew"]);
        assert_eq!(config.hold, ["eslint", "pnpm:typescript"]);
        assert!(config.brew.greedy);
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("600"), Ok(Some(Duration::from_secs(600))));
        assert_eq!(parse_duration("90s"), Ok(Some(Duration::from_secs(90))));
        assert_eq!(parse_duration("10m"), Ok(Some(Duration::from_secs(600))));
        assert_eq!(parse_duration("1h"), Ok(Some(Duration::from_secs(3600))));
        assert_eq!(parse_duration("0"), Ok(None));
        assert!(parse_duration("10 minutes").is_err());
        assert!(parse_duration("m").is_err());
        assert_eq!(show_duration(Duration::from_secs(600)), "10m");
        assert_eq!(show_duration(Duration::from_secs(90)), "90s");
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<Config>("excludes = []").is_err());
    }

    #[test]
    fn hold_matches_plain_and_qualified_names() {
        let hold = ["eslint".to_string(), "pnpm:typescript".to_string()];
        assert!(held(&hold, "npm", "eslint"));
        assert!(held(&hold, "pnpm", "typescript"));
        assert!(!held(&hold, "npm", "typescript"));
        assert!(!held(&hold, "npm", "@google/gemini-cli"));
    }
}
