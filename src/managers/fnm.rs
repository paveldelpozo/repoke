use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::node_versions::{self, Version};
use crate::manager::{Manager, Package, Step, capture};

pub struct Fnm;

impl Manager for Fnm {
    fn id(&self) -> &'static str {
        "fnm"
    }

    fn name(&self) -> &'static str {
        "fnm (Node.js)"
    }

    fn binary(&self) -> &'static str {
        "fnm"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let (installed, _) = parse_list(&capture(bin, ["list"])?.expect(&[0])?);
        let remote: Vec<Version> = capture(bin, ["ls-remote"])?
            .expect(&[0])?
            .lines()
            .filter_map(|l| l.split_whitespace().next().and_then(node_versions::parse))
            .collect();
        Ok(node_versions::outdated(&installed, &remote))
    }

    /// fnm keeps globals per Node version, so they are reinstalled on the new one.
    fn upgrade_steps(&self, bin: &Path, packages: &[Package]) -> Vec<Step> {
        let default = capture(bin, ["list"])
            .and_then(|o| o.expect(&[0]))
            .ok()
            .and_then(|list| parse_list(&list).1);
        packages
            .iter()
            .map(|p| {
                let latest = p.latest.clone();
                let mut commands = vec![vec!["install".to_string(), latest.clone()]];
                if default.is_some_and(|d| Some(d[0]) == node_versions::major(p)) {
                    commands.push(vec!["default".to_string(), latest.clone()]);
                }
                // Globals live in the version in use, which is the default when it is on this line.
                let source = default
                    .filter(|d| Some(d[0]) == node_versions::major(p))
                    .map_or(p.current.clone(), node_versions::show);
                let globals = globals(bin, &source);
                if !globals.is_empty() {
                    let mut install = vec![
                        "exec".to_string(),
                        format!("--using={latest}"),
                        "npm".to_string(),
                        "install".to_string(),
                        "--global".to_string(),
                    ];
                    install.extend(globals);
                    commands.push(install);
                }
                Step {
                    commands,
                    packages: vec![p.clone()],
                }
            })
            .collect()
    }
}

/// Lines look like `* v22.13.1 default, lts-latest`.
fn parse_list(text: &str) -> (Vec<Version>, Option<Version>) {
    let mut installed = Vec::new();
    let mut default = None;
    for line in text.lines() {
        if let Some(version) = line.split_whitespace().find_map(node_versions::parse) {
            installed.push(version);
            if line.contains("default") {
                default = Some(version);
            }
        }
    }
    (installed, default)
}

#[derive(Deserialize)]
struct Globals {
    #[serde(default)]
    dependencies: BTreeMap<String, serde_json::Value>,
}

fn globals(bin: &Path, version: &str) -> Vec<String> {
    let using = format!("--using={version}");
    let args = [&using, "npm", "ls", "--global", "--depth=0", "--json"];
    let Ok(json) = capture(bin, ["exec"].into_iter().chain(args)).and_then(|o| o.expect(&[0]))
    else {
        return Vec::new();
    };
    serde_json::from_str::<Globals>(&json)
        .map(|g| {
            g.dependencies
                .into_keys()
                // Both ship with Node itself.
                .filter(|name| name != "npm" && name != "corepack")
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_installed_versions_and_default() {
        let list = "* v18.18.0\n* v22.13.1 default, lts-latest\n* v24.15.0\n* system\n";
        let (installed, default) = parse_list(list);
        assert_eq!(installed, [[18, 18, 0], [22, 13, 1], [24, 15, 0]]);
        assert_eq!(default, Some([22, 13, 1]));
    }
}
