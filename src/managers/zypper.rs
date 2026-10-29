use std::path::Path;

use crate::manager::{Installed, Manager, Package, Step, capture, needs_sudo};

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

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(parse_user_installed(
            &capture(bin, ["--non-interactive", "packages", "--userinstalled"])?.expect(&[0])?,
        ))
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["--non-interactive", "install"], packages)
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

/// Rows of `packages --userinstalled`, where `i+` marks what the user asked for:
/// `i+ | repo-oss (16.0) | jq | 1.7.1-160000.5.1 | aarch64`. One entry per name, whatever the arch.
fn parse_user_installed(text: &str) -> Vec<Installed> {
    let mut installed: Vec<Installed> = Vec::new();
    for line in text.lines().filter(|l| l.starts_with("i+ ")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let (Some(name), Some(version)) = (cells.get(2), cells.get(3)) else {
            continue;
        };
        if !installed.iter().any(|i| i.name == *name) {
            installed.push(Installed::new(*name).version(*version));
        }
    }
    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_user_installed() {
        let text = "S  | Repository      | Name | Version          | Arch\n---+-----------------+------+------------------+--------\ni+ | repo-oss (16.0) | jq   | 1.7.1-160000.5.1 | aarch64\ni+ | repo-oss (16.0) | jq   | 1.7.1-160000.5.1 | x86_64\ni  | repo-oss (16.0) | libjq1 | 1.7.1-160000.5.1 | aarch64\n";
        let installed = parse_user_installed(text);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "jq");
        assert_eq!(installed[0].version.as_deref(), Some("1.7.1-160000.5.1"));
    }

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
