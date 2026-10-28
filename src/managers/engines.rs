//! npm and pnpm: a global whose new release needs a newer Node than the one in use installs
//! fine and then fails to run, so it is held back with the reason instead.

use std::path::{Path, PathBuf};
use std::thread;

use serde::Deserialize;

use super::node_versions::{self, Version};
use crate::manager::{Package, capture};

/// Packages whose `latest` (the newest release when it is empty, as in `init`) declares an
/// `engines.node` range the running Node is outside of. `viewer` is npm or pnpm, whose `view`
/// asks the registry; the Node checked is the one next to `bin`, the manager.
pub fn incompatible(bin: &Path, viewer: &Path, packages: &[Package]) -> Vec<(Package, String)> {
    let Some(node) = node_version(bin) else {
        return Vec::new();
    };
    thread::scope(|scope| {
        let handles: Vec<_> = packages
            .iter()
            .map(|p| scope.spawn(move || (p, required(viewer, p))))
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter_map(|(p, range)| {
                let range = range?;
                (!satisfies(&range, node)).then(|| {
                    let release = match p.latest.as_str() {
                        "" => "its latest release",
                        version => version,
                    };
                    let reason = format!(
                        "{release} needs Node {range}, and this is Node {}",
                        node_versions::show(node)
                    );
                    (p.clone(), reason)
                })
            })
            .collect()
    })
}

/// The Node next to the manager (fnm and nvm put them together), else the one on the PATH.
fn node_version(bin: &Path) -> Option<Version> {
    let beside = bin.with_file_name(if cfg!(windows) { "node.exe" } else { "node" });
    let node = if beside.exists() {
        beside
    } else {
        PathBuf::from("node")
    };
    node_versions::parse(&capture(node, ["--version"]).ok()?.expect(&[0]).ok()?)
}

fn required(bin: &Path, package: &Package) -> Option<String> {
    let version = match package.latest.as_str() {
        "" => "latest",
        version => version,
    };
    let spec = format!("{}@{version}", package.name);
    let json = capture(bin, ["view", &spec, "engines.node", "--json"])
        .ok()?
        .expect(&[0])
        .ok()?;
    // One string, or an array when the spec matches several versions.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Engines {
        One(String),
        Many(Vec<String>),
    }
    match serde_json::from_str(json.trim()).ok()? {
        Engines::One(range) => Some(range),
        Engines::Many(ranges) => ranges.into_iter().last(),
    }
}

/// Whether `version` is in an npm semver range like `^18.18.0 || >=20`. Syntax it doesn't know
/// counts as a match: holding a package back by mistake is worse than letting npm decide.
pub fn satisfies(range: &str, version: Version) -> bool {
    range
        .split("||")
        .map(|set| in_set(set, version))
        .try_fold(false, |any, set| set.map(|ok| any || ok))
        .unwrap_or(true)
}

fn in_set(set: &str, v: Version) -> Option<bool> {
    let set = set.trim();
    if let Some((low, high)) = set.split_once(" - ") {
        let (low, high) = (partial(low)?, partial(high)?);
        return Some(v >= floor(&low) && below_ceiling(&high, v));
    }
    // `>= 18` is written with a space now and then.
    let mut comparators = Vec::new();
    let mut pending = String::new();
    for token in set.split_whitespace() {
        pending.push_str(token);
        if !matches!(token, ">" | ">=" | "<" | "<=" | "=" | "^" | "~") {
            comparators.push(std::mem::take(&mut pending));
        }
    }
    comparators
        .iter()
        .map(|c| comparator(c, v))
        .try_fold(true, |all, c| c.map(|ok| all && ok))
}

fn comparator(text: &str, v: Version) -> Option<bool> {
    let ops = [">=", "<=", ">", "<", "=", "^", "~"];
    let op = ops.iter().find(|op| text.starts_with(**op)).copied();
    let p = partial(&text[op.map_or(0, str::len)..])?;
    let low = floor(&p);
    Some(match op {
        Some(">=") => v >= low,
        Some(">") if p.len() == 3 => v > low,
        Some(">") => !below_ceiling(&p, v),
        Some("<") => v < low,
        Some("<=") if p.len() == 3 => v <= low,
        Some("<=") => below_ceiling(&p, v),
        Some("^") => {
            // The first non-zero part given is the one that can't change.
            let fixed = p.iter().position(|&n| n != 0).unwrap_or(p.len() - 1);
            v >= low && below_ceiling(&p[..=fixed], v)
        }
        Some("~") => v >= low && below_ceiling(&p[..p.len().min(2)], v),
        _ if p.is_empty() => true,
        _ => v >= low && below_ceiling(&p, v),
    })
}

/// The parts given, up to the first wildcard: `18.x` is `[18]`, `*` is `[]`.
fn partial(text: &str) -> Option<Vec<u64>> {
    let text = text.trim().trim_start_matches('v');
    let text = text.split(['-', '+']).next()?;
    let mut parts = Vec::new();
    for part in text.split('.') {
        if matches!(part, "x" | "X" | "*" | "") {
            break;
        }
        parts.push(part.parse().ok()?);
    }
    (parts.len() <= 3).then_some(parts)
}

fn floor(p: &[u64]) -> Version {
    [0, 1, 2].map(|i| p.get(i).copied().unwrap_or(0))
}

/// `v` is within the given parts: `[20]` means under 21.0.0, a full version means up to it.
fn below_ceiling(p: &[u64], v: Version) -> bool {
    match p.len() {
        0 => true,
        3 => v <= floor(p),
        n => {
            let mut ceiling = floor(p);
            ceiling[n - 1] += 1;
            v < ceiling
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_npm_ranges() {
        let eslint = "^18.18.0 || ^20.9.0 || >=21.1.0";
        assert!(satisfies(eslint, [20, 11, 1]));
        assert!(satisfies(eslint, [24, 0, 0]));
        assert!(!satisfies(eslint, [20, 8, 0]));
        assert!(!satisfies(eslint, [19, 0, 0]));
        assert!(!satisfies(">=22.9.0", [20, 19, 5]));
        assert!(satisfies(">= 18", [18, 0, 0]));
        assert!(satisfies(">=18.0.0 <19", [18, 20, 4]));
        assert!(!satisfies(">=18.0.0 <19", [19, 0, 0]));
        assert!(satisfies("18.x || 20.x", [20, 3, 0]));
        assert!(!satisfies("~18.17", [18, 18, 0]));
        assert!(satisfies("^0.10.0", [0, 10, 5]));
        assert!(!satisfies("^0.10.0", [0, 11, 0]));
        assert!(satisfies("16 - 20", [20, 9, 9]));
        assert!(!satisfies("16 - 20", [21, 0, 0]));
        assert!(satisfies("*", [4, 0, 0]));
        assert!(!satisfies(">16.2", [16, 2, 9]));
    }

    #[test]
    fn unknown_syntax_does_not_hold_anything_back() {
        assert!(satisfies("node >= lts", [8, 0, 0]));
    }
}
