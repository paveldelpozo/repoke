use std::path::Path;
use std::slice;

use super::npm::Globals;
use crate::manager::{Blocked, Installed, Manager, Package, Printed, Step, capture};

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

    fn incompatible(&self, bin: &Path, packages: &[Package]) -> Vec<(Package, String)> {
        super::engines::incompatible(bin, bin, packages)
    }

    /// With `strictDepBuilds`, pnpm fails instead of only warning.
    fn hint(&self, packages: &[Package], error: &str) -> Option<String> {
        let names = ignored_builds(error).unwrap_or_default();
        (error.contains("ERR_PNPM_IGNORED_BUILDS") || !names.is_empty())
            .then(|| allow_hint(&names, packages))
    }

    fn blocked(&self, packages: &[Package], printed: &Printed) -> Vec<Blocked> {
        let Some(names) = ignored_builds(&format!("{}\n{}", printed.stdout, printed.stderr)) else {
            return Vec::new();
        };
        // pnpm names the dependencies, not which global needs them, unless it is the global itself.
        let own: Vec<&Package> = packages
            .iter()
            .filter(|p| names.contains(&p.name))
            .collect();
        let owners = if own.is_empty() {
            packages.iter().collect()
        } else {
            own
        };
        owners
            .into_iter()
            .map(|p| Blocked {
                package: p.clone(),
                reason: format!(
                    "installed, but pnpm ignored the build scripts of {}, so it may not work",
                    names.join(", ")
                ),
                hint: allow_hint(&names, slice::from_ref(p)),
            })
            .collect()
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let json = capture(bin, ["ls", "--global", "--depth=0", "--json"])?.expect(&[0])?;
        let roots: Vec<Globals> =
            serde_json::from_str(&json).map_err(|e| format!("unexpected pnpm output: {e}"))?;
        Ok(roots.into_iter().flat_map(Globals::packages).collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["add", "--global"], packages)
    }

    /// The global rc lives in the platform's config folder.
    fn config_files(&self) -> &'static [&'static str] {
        &[".config/pnpm/rc", "Library/Preferences/pnpm/rc"]
    }
}

/// The names after "Ignored build scripts:", which pnpm prints alone or inside a box.
fn ignored_builds(output: &str) -> Option<Vec<String>> {
    let line = output.lines().find_map(|l| {
        l.split_once("Ignored build scripts:")
            .map(|(_, rest)| rest.trim_end_matches(|c: char| c == '│' || c.is_whitespace()))
    })?;
    let names: Vec<String> = line
        .trim()
        .trim_end_matches('.')
        .split(',')
        .map(|n| without_version(n.trim()).to_string())
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then_some(names)
}

/// `esbuild@0.25.0` and `@scope/pkg@1.0.0` without the version.
fn without_version(spec: &str) -> &str {
    match spec.char_indices().skip(1).find(|(_, c)| *c == '@') {
        Some((i, _)) => &spec[..i],
        None => spec,
    }
}

fn allow_hint(names: &[String], packages: &[Package]) -> String {
    let specs: Vec<String> = packages
        .iter()
        .map(|p| format!("{}@{}", p.name, p.latest))
        .collect();
    let allow = if names.is_empty() {
        "<dependency>".to_string()
    } else {
        names.join(",")
    };
    format!(
        "run them now and allow them from now on with `pnpm add --global --allow-build={allow} {}`",
        specs.join(" ")
    )
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

    fn package(name: &str) -> Package {
        Package {
            name: name.into(),
            current: "1.0.0".into(),
            latest: "2.0.0".into(),
        }
    }

    #[test]
    fn reports_ignored_build_scripts() {
        let printed = Printed {
            stdout: "Packages: +2\n╭ Warning ──╮\n│   Ignored build scripts: esbuild@0.25.0, @parcel/watcher.   │\n│   Run \"pnpm approve-builds\" to pick which dependencies should be allowed to run scripts.   │\nDone in 2s\n".into(),
            stderr: String::new(),
        };
        let blocked = Pnpm.blocked(&[package("vite")], &printed);
        assert_eq!(blocked.len(), 1);
        assert_eq!(
            blocked[0].reason,
            "installed, but pnpm ignored the build scripts of esbuild, @parcel/watcher, so it may not work"
        );
        assert_eq!(
            blocked[0].hint,
            "run them now and allow them from now on with `pnpm add --global --allow-build=esbuild,@parcel/watcher vite@2.0.0`"
        );
        let own = Printed {
            stdout: "Ignored build scripts: esbuild.".into(),
            stderr: String::new(),
        };
        let blocked = Pnpm.blocked(&[package("esbuild"), package("eslint")], &own);
        let names: Vec<&str> = blocked.iter().map(|b| b.package.name.as_str()).collect();
        assert_eq!(names, ["esbuild"]);
    }

    #[test]
    fn approved_builds_block_nothing() {
        let printed = Printed {
            stdout: "Packages: +1\nDone in 1s\n".into(),
            stderr: String::new(),
        };
        assert!(Pnpm.blocked(&[package("vite")], &printed).is_empty());
    }

    #[test]
    fn hints_at_strict_dep_builds_failures() {
        let error = "pnpm add --global vite@2.0.0 exited with 1\nERR_PNPM_IGNORED_BUILDS  Ignored build scripts: esbuild";
        assert!(
            Pnpm.hint(&[package("vite")], error)
                .unwrap()
                .contains("--allow-build=esbuild vite@2.0.0")
        );
        assert!(
            Pnpm.hint(&[package("vite")], "exited with 1\nnetwork")
                .is_none()
        );
    }
}
