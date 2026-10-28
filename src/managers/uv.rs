use std::path::Path;

use crate::manager::{Installed, Manager, Package, Step, capture, lines};

pub struct Uv;

impl Manager for Uv {
    fn id(&self) -> &'static str {
        "uv"
    }

    fn name(&self) -> &'static str {
        "uv (tools)"
    }

    fn binary(&self) -> &'static str {
        "uv"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let mut packages =
            parse_tools(&capture(bin, ["tool", "list", "--outdated"])?.expect(&[0])?);
        // `self update` only works for the standalone installer; with brew or pip it fails and
        // that install is updated by its own manager.
        if let Ok(out) = capture(bin, ["self", "update", "--dry-run"])
            && out.code == Some(0)
            && let Some(own) = parse_self_update(&format!("{}\n{}", out.stdout, out.stderr))
        {
            packages.push(own);
        }
        Ok(packages)
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let (own, tools): (Vec<Package>, Vec<Package>) =
            packages.iter().cloned().partition(|p| p.name == "uv");
        let mut steps = Vec::new();
        if !tools.is_empty() {
            let names = tools.iter().map(|p| p.name.clone());
            steps.push(Step::new(
                ["tool".to_string(), "upgrade".to_string()]
                    .into_iter()
                    .chain(names),
                &tools,
            ));
        }
        if !own.is_empty() {
            steps.push(Step::new(["self", "update"], &own));
        }
        steps
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(lines(bin, ["tool", "list"])?
            .iter()
            .filter(|l| !l.starts_with('-'))
            .filter_map(|l| {
                let mut tokens = l.split_whitespace();
                let name = tokens.next()?;
                let version = tokens.next().unwrap_or_default().trim_start_matches('v');
                Some(Installed::new(name).version(version))
            })
            .collect())
    }

    /// `uv tool install` takes one tool at a time.
    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .map(|p| Step::install(["tool", "install", &p.name], std::slice::from_ref(p)))
            .collect()
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[".config/uv/uv.toml"]
    }
}

/// Tool lines are `parakeet-mlx v0.5.2 [latest: 0.5.3]`, followed by `- executable` lines.
fn parse_tools(text: &str) -> Vec<Package> {
    text.lines()
        .filter(|l| !l.starts_with('-'))
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let name = tokens.next()?;
            let current = tokens.next()?.strip_prefix('v')?;
            let latest = line.split("[latest: ").nth(1)?.trim_end_matches(']');
            Some(Package {
                name: name.to_string(),
                current: current.to_string(),
                latest: latest.trim().to_string(),
            })
        })
        .collect()
}

/// `Would update uv from v0.12.10 to v0.13.6`
fn parse_self_update(text: &str) -> Option<Package> {
    let line = text.lines().find(|l| l.contains("Would update uv from"))?;
    let (from, to) = line.split("from ").nth(1)?.split_once(" to ")?;
    Some(Package {
        name: "uv".to_string(),
        current: from.trim().trim_start_matches('v').to_string(),
        latest: to.trim().trim_start_matches('v').to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated_tools() {
        let text = "parakeet-mlx v0.5.2 [latest: 0.5.3]\n- parakeet-mlx\nruff v0.9.0 [latest: 0.10.1]\n- ruff\n";
        let packages = parse_tools(text);
        assert_eq!(packages.len(), 2);
        assert_eq!(
            packages[0],
            Package {
                name: "parakeet-mlx".into(),
                current: "0.5.2".into(),
                latest: "0.5.3".into()
            }
        );
    }

    #[test]
    fn parses_self_update_dry_run() {
        let own = parse_self_update(
            "info: Checking for updates...\nWould update uv from v0.12.10 to v0.13.6\n",
        )
        .unwrap();
        assert_eq!(
            (own.current.as_str(), own.latest.as_str()),
            ("0.12.10", "0.13.6")
        );
    }
}
