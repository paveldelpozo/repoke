use std::path::Path;

use serde::Deserialize;

use super::table;

use crate::manager::{Installed, Manager, Package, Step, capture};

pub struct Winget;

impl Manager for Winget {
    fn id(&self) -> &'static str {
        "winget"
    }

    fn name(&self) -> &'static str {
        "WinGet"
    }

    fn binary(&self) -> &'static str {
        "winget"
    }

    fn supported(&self) -> bool {
        cfg!(windows)
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let args = [
            "upgrade",
            "--accept-source-agreements",
            "--disable-interactivity",
        ];
        Ok(parse_upgrade(&capture(bin, args)?.expect(&[0])?))
    }

    /// winget upgrades one package id per call.
    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| {
                Step::new(
                    [
                        "upgrade",
                        "--id",
                        &p.name,
                        "--exact",
                        "--version",
                        &p.latest,
                        "--silent",
                        "--accept-package-agreements",
                        "--accept-source-agreements",
                        "--disable-interactivity",
                    ],
                    std::slice::from_ref(p),
                )
            })
            .collect()
    }

    /// `winget list` mixes in everything Windows installs; `export` keeps what winget can install.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let file = std::env::temp_dir().join(format!("repoke-winget-{}.json", std::process::id()));
        let path = file.display().to_string();
        let exported = capture(
            bin,
            [
                "export",
                "-o",
                &path,
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        )
        .and_then(|o| o.expect(&[0]))
        .and_then(|_| std::fs::read_to_string(&file).map_err(|e| e.to_string()));
        let _ = std::fs::remove_file(&file);
        parse_export(&exported?)
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| {
                Step::install(
                    [
                        "install",
                        "--id",
                        &p.name,
                        "--exact",
                        "--silent",
                        "--accept-package-agreements",
                        "--accept-source-agreements",
                        "--disable-interactivity",
                    ],
                    std::slice::from_ref(p),
                )
            })
            .collect()
    }
}

/// Columns: Name, Id, Version, Available, Source. Packages are named by Id, which is what
/// `upgrade --id` takes; display names get truncated.
fn parse_upgrade(text: &str) -> Vec<Package> {
    table::parse(text)
        .into_iter()
        .filter_map(|row| {
            let id = row.get(1).filter(|id| !id.is_empty())?;
            let latest = row.get(3).filter(|v| !v.is_empty())?;
            Some(Package {
                name: id.clone(),
                current: row.get(2).cloned().unwrap_or_default(),
                latest: latest.clone(),
            })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Export {
    #[serde(default)]
    sources: Vec<Source>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Source {
    #[serde(default)]
    packages: Vec<Exported>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Exported {
    package_identifier: String,
}

fn parse_export(json: &str) -> Result<Vec<Installed>, String> {
    let export: Export =
        serde_json::from_str(json).map_err(|e| format!("unexpected winget export: {e}"))?;
    Ok(export
        .sources
        .into_iter()
        .flat_map(|s| s.packages)
        .map(|p| Installed::new(p.package_identifier))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upgrade_table() {
        let text = "Name            Id                  Version  Available Source\n------------------------------------------------------------------\nGit             Git.Git             2.46.0   2.47.0    winget\n1 upgrades available.\n";
        assert_eq!(
            parse_upgrade(text),
            [Package {
                name: "Git.Git".into(),
                current: "2.46.0".into(),
                latest: "2.47.0".into()
            }]
        );
    }
}
