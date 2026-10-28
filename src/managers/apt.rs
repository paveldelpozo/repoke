use std::path::Path;

use crate::manager::{Manager, Package, Step, capture, needs_sudo};

pub struct Apt;

impl Manager for Apt {
    fn id(&self) -> &'static str {
        "apt"
    }

    fn name(&self) -> &'static str {
        "APT"
    }

    fn binary(&self) -> &'static str {
        "apt-get"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "linux")
    }

    /// A simulated upgrade needs no root and, unlike `apt`, has an output meant for scripts.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        Ok(parse_simulation(
            &capture(bin, ["--simulate", "upgrade"])?.expect(&[0])?,
        ))
    }

    fn refresh(&self, bin: &Path) -> Result<(), String> {
        let out = if needs_sudo() {
            capture("sudo", ["-n".as_ref(), bin.as_os_str(), "update".as_ref()])?
        } else {
            capture(bin, ["update"])?
        };
        out.expect(&[0]).map(drop)
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        needs_sudo()
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        let args = ["install", "--only-upgrade", "-y"];
        vec![Step::new(
            args.into_iter().map(String::from).chain(names),
            packages,
        )]
    }
}

/// `Inst libc6 [2.35-0ubuntu3.1] (2.35-0ubuntu3.4 Ubuntu:22.04/jammy-updates [amd64])`.
/// Lines without `[current]` are new dependencies, not upgrades.
fn parse_simulation(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("Inst ")?;
            let (name, rest) = rest.split_once(' ')?;
            let current = rest.strip_prefix('[')?.split(']').next()?;
            let latest = rest.split('(').nth(1)?.split_whitespace().next()?;
            Some(Package {
                name: name.to_string(),
                current: current.to_string(),
                latest: latest.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simulated_upgrade() {
        let text = "Reading package lists...\nInst libc6 [2.35-0ubuntu3.1] (2.35-0ubuntu3.4 Ubuntu:22.04/jammy-updates [amd64])\nInst libnew (1.0 Ubuntu:22.04/jammy [amd64])\nConf libc6 (2.35-0ubuntu3.4 Ubuntu:22.04/jammy-updates [amd64])\n";
        assert_eq!(
            parse_simulation(text),
            [Package {
                name: "libc6".into(),
                current: "2.35-0ubuntu3.1".into(),
                latest: "2.35-0ubuntu3.4".into()
            }]
        );
    }
}
