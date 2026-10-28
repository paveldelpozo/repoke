use std::path::Path;

use serde::Deserialize;

use super::table;

use crate::manager::{Installed, Manager, Package, Step, capture};

pub struct Scoop;

impl Manager for Scoop {
    fn id(&self) -> &'static str {
        "scoop"
    }

    fn name(&self) -> &'static str {
        "Scoop"
    }

    fn binary(&self) -> &'static str {
        "scoop"
    }

    fn supported(&self) -> bool {
        cfg!(windows)
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        Ok(parse_status(&capture(bin, ["status"])?.expect(&[0])?))
    }

    /// Without app names, `scoop update` updates scoop itself and its buckets.
    fn refresh(&self, bin: &Path) -> Result<(), String> {
        capture(bin, ["update"])?.expect(&[0]).map(drop)
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["update".to_string()].into_iter().chain(names),
            packages,
        )]
    }

    /// Buckets first (as `bucket:<name>`), then apps with their bucket as `kind`.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        parse_export(&capture(bin, ["export"])?.expect(&[0])?)
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| match p.kind.as_deref() {
                Some("bucket") => {
                    Step::install(["bucket", "add", &p.name], std::slice::from_ref(p))
                }
                Some(bucket) => Step::install(
                    ["install", &format!("{bucket}/{}", p.name)],
                    std::slice::from_ref(p),
                ),
                None => Step::install(["install", &p.name], std::slice::from_ref(p)),
            })
            .collect()
    }
}

/// Columns: Name, Installed Version, Latest Version, Missing Dependencies, Info.
fn parse_status(text: &str) -> Vec<Package> {
    table::parse(text)
        .into_iter()
        .filter_map(|row| {
            let latest = row.get(2).filter(|v| !v.is_empty())?;
            Some(Package {
                name: row.first()?.clone(),
                current: row.get(1).cloned().unwrap_or_default(),
                latest: latest.clone(),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct Export {
    #[serde(default)]
    buckets: Vec<Bucket>,
    #[serde(default)]
    apps: Vec<App>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Bucket {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct App {
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    source: String,
}

/// `main` is scoop's own bucket and needs no `bucket add`.
fn parse_export(json: &str) -> Result<Vec<Installed>, String> {
    let export: Export =
        serde_json::from_str(json).map_err(|e| format!("unexpected scoop export: {e}"))?;
    let buckets = export
        .buckets
        .into_iter()
        .filter(|b| b.name != "main")
        .map(|b| Installed::new(b.name).kind("bucket"));
    let apps = export.apps.into_iter().map(|a| {
        let app = Installed::new(a.name).version(a.version);
        if a.source.is_empty() || a.source == "main" {
            app
        } else {
            app.kind(a.source)
        }
    });
    Ok(buckets.chain(apps).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_table() {
        let text = "Scoop is up to date.\n\nName    Installed Version Latest Version Missing Dependencies Info\n----    ----------------- -------------- -------------------- ----\ngit     2.46.0            2.47.0\n7zip    24.07                                                 Held package\n";
        let packages = parse_status(text);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "git");
        assert_eq!(packages[0].latest, "2.47.0");
    }
}
