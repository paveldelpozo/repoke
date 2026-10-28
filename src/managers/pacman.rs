use std::path::Path;

use crate::manager::{self, Installed, Manager, Package, Step, capture, lines, needs_sudo};

pub struct Pacman;

impl Manager for Pacman {
    fn id(&self) -> &'static str {
        "pacman"
    }

    fn name(&self) -> &'static str {
        "pacman"
    }

    fn binary(&self) -> &'static str {
        "pacman"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "linux")
    }

    /// `checkupdates` syncs a throwaway copy of the database. Running `pacman -Sy` to refresh
    /// would set up a partial upgrade, which Arch does not support.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let text = match manager::locate("checkupdates") {
            // 2 means no updates.
            Some(checkupdates) => capture(checkupdates, [] as [&str; 0])?.expect(&[0, 2])?,
            None => capture(bin, ["-Qu"])?.expect(&[0, 1])?,
        };
        Ok(parse_updates(&text))
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        needs_sudo()
    }

    /// Arch only supports full upgrades, so any selection runs `-Syu` for everything.
    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        vec![Step::new(["-Syu", "--noconfirm"], packages)]
    }

    /// Explicitly installed packages from the repos; AUR ones (`-m`) need an AUR helper.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(lines(bin, ["-Qqen"])?
            .into_iter()
            .map(Installed::new)
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["-S", "--needed", "--noconfirm"], packages)
    }
}

/// `linux 6.11.3.arch1-1 -> 6.11.4.arch1-1`
fn parse_updates(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let (name, current, arrow, latest) = (
                tokens.next()?,
                tokens.next()?,
                tokens.next()?,
                tokens.next()?,
            );
            (arrow == "->").then(|| Package {
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
    fn parses_updates() {
        let packages = parse_updates(
            "linux 6.11.3.arch1-1 -> 6.11.4.arch1-1\nvim 9.1.0-1 -> 9.1.1-1 [ignored]\n",
        );
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].latest, "6.11.4.arch1-1");
    }

    #[test]
    fn always_runs_a_full_upgrade() {
        let steps = Pacman.upgrade_steps(Path::new("pacman"), &[]);
        assert_eq!(steps[0].commands[0], ["-Syu", "--noconfirm"]);
    }
}
