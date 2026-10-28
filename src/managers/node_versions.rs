//! Shared by fnm and nvm: each installed major line of Node is one "package", upgraded to the
//! newest release of that same line. Moving to a new major is left to the user.

use crate::manager::{Installed, Package};

pub type Version = [u64; 3];

pub fn parse(token: &str) -> Option<Version> {
    let mut parts = token.trim().strip_prefix('v')?.split('.');
    let mut next = || parts.next()?.parse().ok();
    Some([next()?, next()?, next()?])
}

pub fn show(v: Version) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

pub fn outdated(installed: &[Version], remote: &[Version]) -> Vec<Package> {
    let mut majors: Vec<u64> = installed.iter().map(|v| v[0]).collect();
    majors.sort_unstable();
    majors.dedup();
    majors
        .into_iter()
        .filter_map(|major| {
            let current = installed.iter().filter(|v| v[0] == major).max()?;
            let latest = remote.iter().filter(|v| v[0] == major).max()?;
            (latest > current).then(|| Package {
                name: format!("node@{major}"),
                current: show(*current),
                latest: show(*latest),
            })
        })
        .collect()
}

/// For `export`: the newest installed version of each major line, the default's line marked.
pub fn lines(installed: &[Version], default: Option<Version>) -> Vec<Installed> {
    let mut newest: Vec<Version> = Vec::new();
    for v in installed {
        match newest.iter_mut().find(|n| n[0] == v[0]) {
            Some(n) if *v > *n => *n = *v,
            Some(_) => {}
            None => newest.push(*v),
        }
    }
    newest.sort_unstable();
    newest
        .into_iter()
        .map(|v| {
            let line = Installed::new(format!("node@{}", v[0])).version(show(v));
            if default.is_some_and(|d| d[0] == v[0]) {
                line.kind("default")
            } else {
                line
            }
        })
        .collect()
}

/// With `[node] remove_old`, the releases on the package's line that the upgrade replaces.
pub fn replaced(installed: &[Version], package: &Package) -> Vec<Version> {
    if !crate::config::current().node.remove_old {
        return Vec::new();
    }
    let (Some(major), Some(latest)) = (major(package), parse(&format!("v{}", package.latest)))
    else {
        return Vec::new();
    };
    installed
        .iter()
        .filter(|v| v[0] == major && **v != latest)
        .copied()
        .collect()
}

pub fn major(package: &Package) -> Option<u64> {
    package.name.strip_prefix("node@")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versions() {
        assert_eq!(parse("v22.13.1"), Some([22, 13, 1]));
        assert_eq!(parse("system"), None);
        assert_eq!(parse("v22"), None);
    }

    #[test]
    fn one_entry_per_outdated_major_line() {
        let installed = [[20, 6, 1], [20, 19, 0], [22, 13, 1], [24, 22, 1]];
        let remote = [[20, 19, 5], [22, 22, 2], [24, 22, 1], [26, 12, 1]];
        let packages = outdated(&installed, &remote);
        let names: Vec<_> = packages
            .iter()
            .map(|p| format!("{} {} {}", p.name, p.current, p.latest))
            .collect();
        assert_eq!(
            names,
            ["node@20 20.19.0 20.19.5", "node@22 22.13.1 22.22.2"]
        );
    }
}
