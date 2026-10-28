use std::path::Path;

use crate::manager::{Manager, Package, Step, capture, needs_sudo};

pub struct Zypper;

impl Manager for Zypper {
    fn id(&self) -> &'static str {
        "zypper"
    }

    fn name(&self) -> &'static str {
        "zypper"
    }

    fn binary(&self) -> &'static str {
        "zypper"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "linux")
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        Ok(parse_list_updates(
            &capture(bin, ["--non-interactive", "list-updates"])?.expect(&[0])?,
        ))
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        needs_sudo()
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["--non-interactive".to_string(), "update".to_string()]
                .into_iter()
                .chain(names),
            packages,
        )]
    }
}

/// Table rows: `v | Main Repository | curl | 8.9.1-1.1 | 8.10.1-1.1 | x86_64`
fn parse_list_updates(text: &str) -> Vec<Package> {
    text.lines()
        .filter(|l| l.starts_with("v "))
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            Some(Package {
                name: cells.get(2)?.to_string(),
                current: cells.get(3)?.to_string(),
                latest: cells.get(4)?.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_list_updates() {
        let text = "S | Repository      | Name | Current Version | Available Version | Arch\n--+-----------------+------+-----------------+-------------------+-------\nv | Main Repository | curl | 8.9.1-1.1       | 8.10.1-1.1        | x86_64\n";
        assert_eq!(
            parse_list_updates(text),
            [Package {
                name: "curl".into(),
                current: "8.9.1-1.1".into(),
                latest: "8.10.1-1.1".into()
            }]
        );
    }
}
