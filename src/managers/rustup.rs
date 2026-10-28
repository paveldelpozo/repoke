use std::path::Path;

use crate::manager::{Manager, Package, Step, capture};

pub struct Rustup;

impl Manager for Rustup {
    fn id(&self) -> &'static str {
        "rustup"
    }

    fn name(&self) -> &'static str {
        "rustup (toolchains)"
    }

    fn binary(&self) -> &'static str {
        "rustup"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        // `rustup check` exits 100 when something can be updated.
        Ok(parse_check(&capture(bin, ["check"])?.expect(&[0, 100])?))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let (own, toolchains): (Vec<Package>, Vec<Package>) =
            packages.iter().cloned().partition(|p| p.name == "rustup");
        let mut steps = Vec::new();
        if !toolchains.is_empty() {
            let names = toolchains.iter().map(|p| p.name.clone());
            let args = ["update".to_string()]
                .into_iter()
                .chain(names)
                .chain(["--no-self-update".to_string()]);
            steps.push(Step::new(args, &toolchains));
        }
        if !own.is_empty() {
            steps.push(Step::new(["self", "update"], &own));
        }
        steps
    }
}

/// `stable-aarch64-apple-darwin - update available: 1.98.1 (48a229cea 2026-09-01) -> 1.99.1 (…)`
/// and `rustup - update available : 1.29.1 -> 1.29.2`.
fn parse_check(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(" - update available")?;
            let (from, to) = rest.split_once(':')?.1.split_once("->")?;
            Some(Package {
                name: name.trim().to_string(),
                current: from.split_whitespace().next()?.to_string(),
                latest: to.split_whitespace().next()?.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toolchains_and_rustup() {
        let text = "stable-aarch64-apple-darwin - update available: 1.98.1 (48a229cea 2026-09-01) -> 1.99.1 (58cf8f9c3 2026-10-13)\nnightly-aarch64-apple-darwin - Up to date : 1.100.0-nightly\nrustup - update available : 1.29.1 -> 1.29.2\n";
        let packages = parse_check(text);
        let found: Vec<_> = packages
            .iter()
            .map(|p| format!("{} {} {}", p.name, p.current, p.latest))
            .collect();
        assert_eq!(
            found,
            [
                "stable-aarch64-apple-darwin 1.98.1 1.99.1",
                "rustup 1.29.1 1.29.2"
            ]
        );
    }
}
