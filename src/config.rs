use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde::Deserialize;

/// `~/.config/repoke/config.toml`, or the file named by `REPOKE_CONFIG`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Managers left out unless named in `--only`.
    pub exclude: Vec<String>,
    /// Packages never upgraded nor reported as pending, as `name` or `manager:name`.
    pub hold: Vec<String>,
}

pub fn load() -> Result<Config, String> {
    let Some(path) = path() else {
        return Ok(Config::default());
    };
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn path() -> Option<PathBuf> {
    if let Some(file) = env::var_os("REPOKE_CONFIG") {
        return Some(file.into());
    }
    let base = if cfg!(windows) {
        env::var_os("APPDATA").map(PathBuf::from)
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    Some(base?.join("repoke").join("config.toml"))
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
            "#,
        )
        .unwrap();
        assert_eq!(config.exclude, ["brew"]);
        assert_eq!(config.hold, ["eslint", "pnpm:typescript"]);
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
