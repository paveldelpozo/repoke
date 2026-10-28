use std::path::Path;

use crate::manager::{Installed, Manager, Package, Step, capture, lines, needs_sudo};

pub struct Snap;

impl Manager for Snap {
    fn id(&self) -> &'static str {
        "snap"
    }

    fn name(&self) -> &'static str {
        "Snap"
    }

    fn binary(&self) -> &'static str {
        "snap"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "linux")
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        Ok(parse_refresh_list(
            &capture(bin, ["refresh", "--list"])?.expect(&[0])?,
        ))
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        needs_sudo()
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["refresh".to_string()].into_iter().chain(names),
            packages,
        )]
    }

    /// Base and system snaps (core, snapd, gnome runtimes) come with the apps that need them.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(lines(bin, ["list"])?
            .iter()
            .skip(1)
            .filter_map(|line| {
                let cells: Vec<&str> = line.split_whitespace().collect();
                let (name, version, notes) = (cells.first()?, cells.get(1)?, cells.last()?);
                if notes.contains("base") || notes.contains("snapd") || notes.contains("core") {
                    return None;
                }
                let installed = Installed::new(*name).version(*version);
                Some(if notes.contains("classic") {
                    installed.kind("classic")
                } else {
                    installed
                })
            })
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| {
                let mut args = vec!["install", p.name.as_str()];
                if p.is("classic") {
                    args.push("--classic");
                }
                Step::install(args, std::slice::from_ref(p))
            })
            .collect()
    }
}

/// Table under a `Name  Version  Rev ...` header; with no updates snap prints only to stderr.
fn parse_refresh_list(text: &str) -> Vec<Package> {
    text.lines()
        .skip_while(|l| !l.starts_with("Name"))
        .skip(1)
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            Some(Package {
                name: tokens.next()?.to_string(),
                current: String::new(),
                latest: tokens.next()?.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_refresh_list() {
        let text = "Name     Version  Rev    Size   Publisher   Notes\nfirefox  131.0.3  5091   285MB  mozilla✓    -\n";
        let packages = parse_refresh_list(text);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].latest, "131.0.3");
        assert!(parse_refresh_list("").is_empty());
    }
}
