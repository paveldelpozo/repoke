use std::path::Path;

use crate::manager::{Manager, Package, Step, capture};

pub struct Flatpak;

impl Manager for Flatpak {
    fn id(&self) -> &'static str {
        "flatpak"
    }

    fn name(&self) -> &'static str {
        "Flatpak"
    }

    fn binary(&self) -> &'static str {
        "flatpak"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "linux")
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let args = ["remote-ls", "--updates", "--columns=application,version"];
        Ok(parse_updates(&capture(bin, args)?.expect(&[0])?))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let ids = packages.iter().map(|p| p.name.clone());
        let args = ["update", "-y", "--noninteractive"];
        vec![Step::new(
            args.into_iter().map(String::from).chain(ids),
            packages,
        )]
    }
}

/// Tab separated `application<TAB>version`; flatpak doesn't list the installed version here.
fn parse_updates(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let mut cells = line.split('\t');
            let name = cells.next()?.trim();
            (!name.is_empty()).then(|| Package {
                name: name.to_string(),
                current: String::new(),
                latest: cells.next().unwrap_or_default().trim().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_updates() {
        let packages = parse_updates("org.mozilla.firefox\t131.0.3\norg.gnome.Platform\t\n");
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].latest, "131.0.3");
        assert_eq!(packages[1].name, "org.gnome.Platform");
    }
}
