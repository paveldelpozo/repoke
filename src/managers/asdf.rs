use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::manager::{Installed, Manager, Package, Step, capture, version_in};
use crate::snapshot;

pub struct Asdf;

impl Manager for Asdf {
    fn id(&self) -> &'static str {
        "asdf"
    }

    fn name(&self) -> &'static str {
        "asdf (tools)"
    }

    fn binary(&self) -> &'static str {
        "asdf"
    }

    fn supported(&self) -> bool {
        cfg!(unix)
    }

    fn version(&self, bin: &Path) -> Option<String> {
        version_in(&capture(bin, ["version"]).ok()?.stdout)
    }

    /// Only the global versions: `asdf current` would also pick up the `.tool-versions` of
    /// whatever project repoke runs from.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let global = global_versions();
        if global.is_empty() {
            return Ok(Vec::new());
        }
        let latest = parse_latest(&capture(bin, ["latest", "--all"])?.expect(&[0])?);
        Ok(outdated(&global, &latest))
    }

    /// `set -u` writes `~/.tool-versions` wherever it runs; the old version stays installed.
    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| Step {
                commands: vec![
                    vec!["install".to_string(), p.name.clone(), p.latest.clone()],
                    vec![
                        "set".to_string(),
                        "-u".to_string(),
                        p.name.clone(),
                        p.latest.clone(),
                    ],
                ],
                packages: vec![p.clone()],
                password: false,
            })
            .collect()
    }

    fn installed(&self, _bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(global_versions()
            .into_iter()
            .map(|(name, version)| Installed::new(name).version(version))
            .collect())
    }

    /// `plugin add` and `install` both succeed when there is nothing to do.
    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| {
                let version = p.version.clone().unwrap_or_else(|| "latest".to_string());
                Step {
                    commands: vec![
                        vec!["plugin".to_string(), "add".to_string(), p.name.clone()],
                        vec!["install".to_string(), p.name.clone(), version.clone()],
                        vec!["set".to_string(), "-u".to_string(), p.name.clone(), version],
                    ],
                    packages: vec![Package::from(p)],
                    password: false,
                }
            })
            .collect()
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[".tool-versions", ".asdfrc"]
    }
}

fn global_versions() -> BTreeMap<String, String> {
    snapshot::home()
        .and_then(|home| fs::read_to_string(home.join(".tool-versions")).ok())
        .map(|text| parse_tool_versions(&text))
        .unwrap_or_default()
}

/// The first version of each tool, which is the one asdf uses. `system`, `ref:` and `path:`
/// are not releases asdf can move forward.
fn parse_tool_versions(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split('#').next()?.split_whitespace();
            let (name, version) = (fields.next()?, fields.next()?);
            let pinned = version == "system" || version.contains(':');
            (!pinned).then(|| (name.to_string(), version.to_string()))
        })
        .collect()
}

/// `asdf latest --all` prints `name<TAB>latest<TAB>installed|missing`, one plugin per line.
fn parse_latest(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            Some((fields.next()?.to_string(), fields.next()?.to_string()))
        })
        .collect()
}

fn outdated(global: &BTreeMap<String, String>, latest: &BTreeMap<String, String>) -> Vec<Package> {
    global
        .iter()
        .filter_map(|(name, current)| {
            let latest = latest.get(name)?;
            (latest != current).then(|| Package {
                name: name.clone(),
                current: current.clone(),
                latest: latest.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_global_tool_versions() {
        let text = "jq 1.7 1.6\n# comment\nnodejs 22.13.1 # pinned for work\npython system\nruby ref:v3.4.0\n\n";
        let global = parse_tool_versions(text);
        assert_eq!(global.len(), 2);
        assert_eq!(global["jq"], "1.7");
        assert_eq!(global["nodejs"], "22.13.1");
    }

    #[test]
    fn compares_against_the_latest_release() {
        let global = parse_tool_versions("jq 1.7\nshellcheck 0.11.0\ngolang 1.25.0\n");
        let latest = parse_latest(
            "jq\t1.8.2\tmissing\nshellcheck\t0.11.0\tinstalled\nterraform\t1.13.0\tmissing\n",
        );
        let packages = outdated(&global, &latest);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "jq");
        assert_eq!(packages[0].current, "1.7");
        assert_eq!(packages[0].latest, "1.8.2");
    }
}
