use std::path::Path;

use crate::manager::{Installed, Manager, Package, Step, capture, lines};

pub struct Choco;

impl Manager for Choco {
    fn id(&self) -> &'static str {
        "choco"
    }

    fn name(&self) -> &'static str {
        "Chocolatey"
    }

    fn binary(&self) -> &'static str {
        "choco"
    }

    fn supported(&self) -> bool {
        cfg!(windows)
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let args = ["outdated", "--limit-output", "--ignore-unfound"];
        Ok(parse_outdated(&capture(bin, args)?.expect(&[0, 2])?))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        let flags = ["-y".to_string(), "--no-progress".to_string()];
        vec![Step::new(
            ["upgrade".to_string()]
                .into_iter()
                .chain(names)
                .chain(flags),
            packages,
        )]
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(lines(bin, ["list", "--limit-output"])?
            .iter()
            .filter_map(|line| {
                let (name, version) = line.split_once('|')?;
                (!name.eq_ignore_ascii_case("chocolatey"))
                    .then(|| Installed::new(name).version(version))
            })
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        let args = ["install".to_string()]
            .into_iter()
            .chain(names)
            .chain(["-y".to_string(), "--no-progress".to_string()]);
        if packages.is_empty() {
            Vec::new()
        } else {
            vec![Step::install(args, packages)]
        }
    }
}

/// `name|current|available|pinned`; pinned packages are left alone.
fn parse_outdated(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().split('|').collect();
            match cells[..] {
                [name, current, latest, pinned] if pinned != "true" => Some(Package {
                    name: name.to_string(),
                    current: current.to_string(),
                    latest: latest.to_string(),
                }),
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated_and_skips_pinned() {
        let packages = parse_outdated("git|2.46.0|2.47.0|false\nnodejs|20.0.0|22.0.0|true\n");
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].latest, "2.47.0");
    }
}
