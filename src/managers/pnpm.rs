use std::path::Path;
use std::slice;

use crate::manager::{Manager, Package, Step, capture};

pub struct Pnpm;

impl Manager for Pnpm {
    fn id(&self) -> &'static str {
        "pnpm"
    }

    fn name(&self) -> &'static str {
        "pnpm (global)"
    }

    fn binary(&self) -> &'static str {
        "pnpm"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let json = capture(bin, ["outdated", "--global", "--format", "json"])?.expect(&[0, 1])?;
        super::npm::parse_outdated(&json)
    }

    /// pnpm refuses `add --global pnpm` (ERR_PNPM_GLOBAL_PNPM_INSTALL) and wants `self-update`.
    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let (own, others): (Vec<Package>, Vec<Package>) =
            packages.iter().cloned().partition(|p| p.name == "pnpm");
        let mut steps = Vec::new();
        if !others.is_empty() {
            steps.push(super::npm::install("add", &others));
        }
        for p in own {
            steps.push(Step::new(["self-update", &p.latest], slice::from_ref(&p)));
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_itself_with_self_update() {
        let package = |name: &str, latest: &str| Package {
            name: name.into(),
            current: "1.0.0".into(),
            latest: latest.into(),
        };
        let steps = Pnpm.upgrade_steps(
            Path::new("pnpm"),
            &[package("pnpm", "12.15.0"), package("eslint", "10.13.0")],
        );
        let args: Vec<_> = steps.iter().map(|s| s.commands[0].join(" ")).collect();
        assert_eq!(args, ["add --global eslint@10.13.0", "self-update 12.15.0"]);
    }
}
