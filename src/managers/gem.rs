use std::path::{Path, PathBuf};

use crate::manager::{self, Manager, Package, Step, capture, writable};

pub struct Gem;

impl Manager for Gem {
    fn id(&self) -> &'static str {
        "gem"
    }

    fn name(&self) -> &'static str {
        "RubyGems"
    }

    fn binary(&self) -> &'static str {
        "gem"
    }

    /// The Ruby bundled with macOS belongs to the system: its gems are not upgraded.
    fn locate(&self) -> Option<PathBuf> {
        manager::locate("gem")
            .filter(|path| !(cfg!(target_os = "macos") && path.starts_with("/usr/bin")))
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        Ok(parse_outdated(&capture(bin, ["outdated"])?.expect(&[0])?))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["update".to_string()].into_iter().chain(names),
            packages,
        )]
    }

    fn needs_root(&self, bin: &Path) -> bool {
        cfg!(unix)
            && capture(bin, ["environment", "gemdir"])
                .and_then(|o| o.expect(&[0]))
                .is_ok_and(|dir| !writable(Path::new(dir.trim())))
    }
}

/// `rake (13.0.6 < 13.2.1)`
fn parse_outdated(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(" (")?;
            let (current, latest) = rest.trim_end_matches(')').split_once(" < ")?;
            Some(Package {
                name: name.trim().to_string(),
                current: current.trim().to_string(),
                latest: latest.trim().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated_gems() {
        let packages = parse_outdated("rake (13.0.6 < 13.2.1)\nbundler (2.4.10 < 2.5.23)\n");
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[1].current, "2.4.10");
        assert_eq!(packages[1].latest, "2.5.23");
    }
}
