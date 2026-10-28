use std::path::Path;

use serde::Deserialize;

use crate::manager::{Installed, Manager, Package, Step, capture};

pub struct Composer;

impl Manager for Composer {
    fn id(&self) -> &'static str {
        "composer"
    }

    fn name(&self) -> &'static str {
        "Composer (global)"
    }

    fn binary(&self) -> &'static str {
        "composer"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let args = [
            "global",
            "outdated",
            "--direct",
            "--format=json",
            "--no-interaction",
        ];
        parse_outdated(&capture(bin, args)?.expect(&[0])?)
    }

    /// `global update` stays inside the version constraints in composer.json, so the
    /// constraint is moved to the version that was listed. `-W` lets dependencies pinned in the
    /// lock file move too (laravel/installer 5.32 needs a newer laravel/prompts).
    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let specs = packages.iter().map(|p| format!("{}:^{}", p.name, p.latest));
        let args = [
            "global",
            "require",
            "--with-all-dependencies",
            "--no-interaction",
        ];
        vec![Step::new(
            args.into_iter().map(String::from).chain(specs),
            packages,
        )]
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let args = [
            "global",
            "show",
            "--direct",
            "--format=json",
            "--no-interaction",
        ];
        let report: Shown = serde_json::from_str(&capture(bin, args)?.expect(&[0])?)
            .map_err(|e| format!("unexpected composer output: {e}"))?;
        Ok(report
            .installed
            .into_iter()
            .map(|e| Installed::new(e.name).version(e.version.trim_start_matches('v')))
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["global", "require", "--no-interaction"], packages)
    }

    /// Not `auth.json`, which holds registry tokens.
    fn config_files(&self) -> &'static [&'static str] {
        &[".composer/config.json", ".config/composer/config.json"]
    }
}

#[derive(Deserialize)]
struct Report {
    #[serde(default)]
    installed: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    version: String,
    latest: String,
}

fn parse_outdated(json: &str) -> Result<Vec<Package>, String> {
    let report: Report =
        serde_json::from_str(json).map_err(|e| format!("unexpected composer output: {e}"))?;
    Ok(report
        .installed
        .into_iter()
        .map(|e| Package {
            name: e.name,
            current: e.version.trim_start_matches('v').to_string(),
            latest: e.latest.trim_start_matches('v').to_string(),
        })
        .collect())
}

#[derive(Deserialize)]
struct Shown {
    #[serde(default)]
    installed: Vec<ShownEntry>,
}

#[derive(Deserialize)]
struct ShownEntry {
    name: String,
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated() {
        let json = r#"{"installed": [{"name": "laravel/installer", "direct-dependency": true, "version": "v5.8.3", "latest": "v5.13.0", "latest-status": "semver-safe-update"}]}"#;
        let packages = parse_outdated(json).unwrap();
        assert_eq!(packages[0].name, "laravel/installer");
        assert_eq!(packages[0].current, "5.8.3");
        assert_eq!(packages[0].latest, "5.13.0");
        assert!(parse_outdated(r#"{"installed": []}"#).unwrap().is_empty());
    }
}
