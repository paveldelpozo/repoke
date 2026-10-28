use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::manager::{self, Installed, Manager, Package, Step, capture};

/// Only `--user` packages: a Homebrew or distro Python is "externally managed" (PEP 668), and
/// forcing it with `--break-system-packages` is how a system Python gets broken.
pub struct Pip;

impl Manager for Pip {
    fn id(&self) -> &'static str {
        "pip"
    }

    fn name(&self) -> &'static str {
        "pip (user)"
    }

    fn binary(&self) -> &'static str {
        "pip3"
    }

    fn locate(&self) -> Option<PathBuf> {
        manager::locate("pip3").or_else(|| manager::locate("pip"))
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let args = [
            "list",
            "--user",
            "--outdated",
            "--format",
            "json",
            "--disable-pip-version-check",
        ];
        parse_outdated(&capture(bin, args)?.expect(&[0])?)
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let specs = packages.iter().map(|p| format!("{}=={}", p.name, p.latest));
        let args = [
            "install",
            "--user",
            "--upgrade",
            "--disable-pip-version-check",
        ];
        vec![Step::new(
            args.into_iter().map(String::from).chain(specs),
            packages,
        )]
    }

    /// `--not-required` leaves out what was only pulled in as a dependency.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let args = [
            "list",
            "--user",
            "--not-required",
            "--format",
            "json",
            "--disable-pip-version-check",
        ];
        let entries: Vec<Listed> = serde_json::from_str(&capture(bin, args)?.expect(&[0])?)
            .map_err(|e| format!("unexpected pip output: {e}"))?;
        Ok(entries
            .into_iter()
            .map(|e| Installed::new(e.name).version(e.version))
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(
            &["install", "--user", "--disable-pip-version-check"],
            packages,
        )
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[
            ".config/pip/pip.conf",
            "Library/Application Support/pip/pip.conf",
            ".pip/pip.conf",
        ]
    }
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    version: String,
    latest_version: String,
}

fn parse_outdated(json: &str) -> Result<Vec<Package>, String> {
    let entries: Vec<Entry> =
        serde_json::from_str(json).map_err(|e| format!("unexpected pip output: {e}"))?;
    Ok(entries
        .into_iter()
        .map(|e| Package {
            name: e.name,
            current: e.version,
            latest: e.latest_version,
        })
        .collect())
}

#[derive(Deserialize)]
struct Listed {
    name: String,
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated() {
        let json = r#"[{"name": "requests", "version": "2.31.0", "latest_version": "2.32.3", "latest_filetype": "wheel"}]"#;
        let packages = parse_outdated(json).unwrap();
        assert_eq!(packages[0].name, "requests");
        assert_eq!(packages[0].latest, "2.32.3");
        assert!(parse_outdated("[]").unwrap().is_empty());
    }
}
