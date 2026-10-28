use std::path::Path;

use super::table;
use crate::manager::{Manager, Package, Step, capture};

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
