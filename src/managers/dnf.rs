use std::collections::HashMap;
use std::path::Path;

use crate::manager::{Installed, Manager, Package, Step, capture, lines, needs_sudo};

pub struct Dnf;

impl Manager for Dnf {
    fn id(&self) -> &'static str {
        "dnf"
    }

    fn name(&self) -> &'static str {
        "DNF"
    }

    fn binary(&self) -> &'static str {
        "dnf"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "linux")
    }

    /// `check-update` refreshes stale metadata itself and exits 100 when there are updates.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let mut packages =
            parse_check_update(&capture(bin, ["check-update", "--quiet"])?.expect(&[0, 100])?);
        if packages.is_empty() {
            return Ok(packages);
        }
        // Not knowing the installed version only costs the "from" column, so rpm may fail.
        let format = "%{NAME} %{EPOCH}:%{VERSION}-%{RELEASE}\\n";
        let args = ["-q", "--qf", format]
            .into_iter()
            .map(String::from)
            .chain(packages.iter().map(|p| p.name.clone()));
        if let Ok(out) = capture("rpm", args) {
            let installed = parse_rpm_query(&out.stdout);
            for p in &mut packages {
                if let Some(version) = installed.get(&p.name) {
                    p.current = version.clone();
                }
            }
        }
        Ok(packages)
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        needs_sudo()
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["upgrade".to_string(), "-y".to_string()]
                .into_iter()
                .chain(names),
            packages,
        )]
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let args = [
            "repoquery",
            "--userinstalled",
            "--queryformat",
            "%{name}\\n",
        ];
        let mut names = lines(bin, args)?;
        names.dedup();
        Ok(names.into_iter().map(Installed::new).collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["install", "-y"], packages)
    }
}

/// `kernel-core.x86_64   6.11.4-301.fc41   updates`; dnf doesn't print the installed version.
fn parse_check_update(text: &str) -> Vec<Package> {
    text.lines()
        .take_while(|l| !l.starts_with("Obsoleting"))
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let (name_arch, latest, _repo) = (tokens.next()?, tokens.next()?, tokens.next()?);
            let (name, _arch) = name_arch.rsplit_once('.')?;
            Some(Package {
                name: name.to_string(),
                current: String::new(),
                latest: latest.to_string(),
            })
        })
        .collect()
}

/// `xz 1:5.6.2-2.fc41` or `bash (none):5.2.32-1.fc41`, written the way dnf writes versions.
fn parse_rpm_query(text: &str) -> HashMap<String, String> {
    let mut installed = HashMap::new();
    for line in text.lines() {
        let Some((name, version)) = line
            .split_once(' ')
            .filter(|_| !line.ends_with("not installed"))
        else {
            continue;
        };
        let version = version.strip_prefix("(none):").unwrap_or(version);
        installed
            .entry(name.to_string())
            .or_insert_with(|| version.to_string());
    }
    installed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_check_update() {
        let text = "\nkernel-core.x86_64          6.11.4-301.fc41     updates\npython3.11.x86_64  3.11.10-1.fc41  updates\nObsoleting Packages\nold.noarch 1.0 updates\n";
        let packages = parse_check_update(text);
        let names: Vec<_> = packages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["kernel-core", "python3.11"]);
        assert_eq!(packages[0].latest, "6.11.4-301.fc41");
    }

    #[test]
    fn reads_installed_versions_like_dnf_writes_them() {
        let installed = parse_rpm_query(
            "xz 1:5.6.2-2.fc41\nbash (none):5.2.32-1.fc41\nbash (none):5.2.32-1.fc41\npackage nope is not installed\n",
        );
        assert_eq!(installed["xz"], "1:5.6.2-2.fc41");
        assert_eq!(installed["bash"], "5.2.32-1.fc41");
        assert_eq!(installed.len(), 2);
    }
}
