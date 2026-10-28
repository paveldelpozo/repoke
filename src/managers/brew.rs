use std::path::Path;

use serde::Deserialize;

use crate::manager::{Manager, Package, Step, capture};

pub struct Brew;

impl Manager for Brew {
    fn id(&self) -> &'static str {
        "brew"
    }

    fn name(&self) -> &'static str {
        "Homebrew"
    }

    fn binary(&self) -> &'static str {
        "brew"
    }

    fn supported(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "linux"))
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let json = capture(bin, ["outdated", "--json=v2"])?.expect(&[0, 1])?;
        parse_outdated(&json)
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["upgrade".to_string()].into_iter().chain(names),
            packages,
        )]
    }

    fn refresh(&self, bin: &Path) -> Result<(), String> {
        capture(bin, ["update"])?.expect(&[0]).map(drop)
    }
}

#[derive(Deserialize)]
struct Report {
    formulae: Vec<Item>,
    casks: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    name: String,
    installed_versions: Vec<String>,
    current_version: String,
}

fn parse_outdated(json: &str) -> Result<Vec<Package>, String> {
    let report: Report =
        serde_json::from_str(json).map_err(|e| format!("unexpected brew output: {e}"))?;
    Ok(report
        .formulae
        .into_iter()
        .chain(report.casks)
        .map(|item| Package {
            name: item.name,
            current: item.installed_versions.last().cloned().unwrap_or_default(),
            latest: item.current_version,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formulae_and_casks() {
        let json = r#"{
            "formulae": [{"name": "git", "installed_versions": ["2.50.0", "2.51.0"], "current_version": "2.52.0", "pinned": false}],
            "casks": [{"name": "firefox", "installed_versions": ["140.0"], "current_version": "141.0"}]
        }"#;
        let packages = parse_outdated(json).unwrap();
        assert_eq!(
            packages,
            vec![
                Package {
                    name: "git".into(),
                    current: "2.51.0".into(),
                    latest: "2.52.0".into()
                },
                Package {
                    name: "firefox".into(),
                    current: "140.0".into(),
                    latest: "141.0".into()
                },
            ]
        );
    }

    #[test]
    fn parses_empty_report() {
        assert!(
            parse_outdated(r#"{"formulae": [], "casks": []}"#)
                .unwrap()
                .is_empty()
        );
    }
}
