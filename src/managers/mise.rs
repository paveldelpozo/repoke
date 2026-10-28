use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::manager::{Installed, Manager, Package, Step, capture};

pub struct Mise;

impl Manager for Mise {
    fn id(&self) -> &'static str {
        "mise"
    }

    fn name(&self) -> &'static str {
        "mise (tools)"
    }

    fn binary(&self) -> &'static str {
        "mise"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        parse_outdated(&capture(bin, ["outdated", "--json"])?.expect(&[0])?)
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["upgrade".to_string()].into_iter().chain(names),
            packages,
        )]
    }

    /// The active version of each tool; with several, the last one listed.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let json = capture(bin, ["ls", "--json"])?.expect(&[0])?;
        let tools: BTreeMap<String, Vec<Listed>> =
            serde_json::from_str(&json).map_err(|e| format!("unexpected mise output: {e}"))?;
        Ok(tools
            .into_iter()
            .filter_map(|(name, versions)| {
                let chosen = versions.iter().find(|v| v.active).or(versions.last())?;
                Some(Installed::new(name).version(chosen.version.clone()))
            })
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| {
                let spec = match &p.version {
                    Some(version) => format!("{}@{version}", p.name),
                    None => p.name.clone(),
                };
                Step::install(["use", "--global", &spec], std::slice::from_ref(p))
            })
            .collect()
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[".config/mise/config.toml"]
    }
}

#[derive(Deserialize)]
struct Entry {
    current: Option<String>,
    latest: String,
}

fn parse_outdated(json: &str) -> Result<Vec<Package>, String> {
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let entries: BTreeMap<String, Entry> =
        serde_json::from_str(json).map_err(|e| format!("unexpected mise output: {e}"))?;
    Ok(entries
        .into_iter()
        .map(|(name, e)| Package {
            name,
            current: e.current.unwrap_or_default(),
            latest: e.latest,
        })
        .collect())
}

#[derive(Deserialize)]
struct Listed {
    version: String,
    #[serde(default)]
    active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated() {
        let json = r#"{"node": {"name": "node", "requested": "22", "current": "22.13.1", "latest": "22.22.2", "source": {"type": "mise.toml"}}}"#;
        let packages = parse_outdated(json).unwrap();
        assert_eq!(packages[0].name, "node");
        assert_eq!(packages[0].latest, "22.22.2");
        assert!(parse_outdated("{}").unwrap().is_empty());
    }
}
