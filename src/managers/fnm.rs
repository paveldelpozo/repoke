use std::path::Path;

use super::node_versions::{self, Version};
use crate::manager::{Installed, Manager, Package, Step, capture};

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
        let (installed, default) = capture(bin, ["list"])
            .and_then(|o| o.expect(&[0]))
            .map(|list| parse_list(&list))
            .unwrap_or_default();
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
                // Steps stop at the first failure, so the old releases go only once the new runs.
                let old = node_versions::replaced(&installed, p);
                if !old.is_empty() {
                    commands.push(vec![
                        "exec".to_string(),
                        format!("--using={latest}"),
                        "node".to_string(),
                        "--version".to_string(),
                    ]);
                    commands.extend(
                        old.into_iter()
                            .map(|v| vec!["uninstall".to_string(), node_versions::show(v)]),
                    );
                }
                Step {
                    commands,
                    packages: vec![p.clone()],
                    password: false,
                }
            })
            .collect()
    }

    /// The newest installed version of each major line; the default line is marked.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let (installed, default) = parse_list(&capture(bin, ["list"])?.expect(&[0])?);
        Ok(node_versions::lines(&installed, default))
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .filter_map(|p| {
                let major = p.name.strip_prefix("node@")?.to_string();
                let mut commands = vec![vec!["install".to_string(), major.clone()]];
                if p.is("default") {
                    commands.push(vec!["default".to_string(), major]);
                }
                Some(Step {
                    commands,
                    packages: vec![Package::from(p)],
                    password: false,
                })
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

fn globals(bin: &Path, version: &str) -> Vec<String> {
    let using = format!("--using={version}");
    let args = [&using, "npm", "ls", "--global", "--depth=0", "--json"];
    capture(bin, ["exec"].into_iter().chain(args))
        .and_then(|o| o.expect(&[0]))
        .and_then(|json| super::npm::parse_globals(&json))
        .map(|globals| globals.into_iter().map(|g| g.name).collect())
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
