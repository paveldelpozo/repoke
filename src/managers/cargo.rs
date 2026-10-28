use std::path::Path;

use crate::manager::{Manager, Package, Step, capture};

pub struct Cargo;

impl Manager for Cargo {
    fn id(&self) -> &'static str {
        "cargo"
    }

    fn name(&self) -> &'static str {
        "cargo (installed crates)"
    }

    fn binary(&self) -> &'static str {
        "cargo"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let list = capture(bin, ["install", "--list"])?.expect(&[0])?;
        let mut packages = Vec::new();
        for (name, current) in parse_installed(&list) {
            let search = capture(bin, ["search", &name, "--limit", "10"])?.expect(&[0])?;
            if let Some(latest) = parse_search(&search, &name)
                && latest != current
            {
                packages.push(Package {
                    name,
                    current,
                    latest,
                });
            }
        }
        Ok(packages)
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let specs = packages.iter().map(|p| format!("{}@{}", p.name, p.latest));
        vec![Step::new(
            ["install".to_string()].into_iter().chain(specs),
            packages,
        )]
    }
}

/// Registry crates only: `name v1.2.3:`. Path and git installs carry `(source)` and are skipped.
fn parse_installed(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|l| !l.starts_with(char::is_whitespace) && !l.contains('('))
        .filter_map(|l| {
            let (name, version) = l.trim_end_matches(':').split_once(' ')?;
            Some((name.to_string(), version.strip_prefix('v')?.to_string()))
        })
        .collect()
}

/// `cargo search` ranks by relevance, so the line for this exact crate is looked up by name.
fn parse_search(text: &str, name: &str) -> Option<String> {
    let prefix = format!("{name} = \"");
    let line = text.lines().find(|l| l.starts_with(&prefix))?;
    Some(line[prefix.len()..].split('"').next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_installs_only() {
        let list = "clauth v0.15.2:\n    clauth\nrepoke v0.1.0 (/Users/me/repoke):\n    repoke\ntool v1.0.0 (https://github.com/x/tool#abc):\n    tool\n";
        assert_eq!(
            parse_installed(list),
            [("clauth".to_string(), "0.15.2".to_string())]
        );
    }

    #[test]
    fn finds_the_exact_crate_in_search_results() {
        let search = "import-lint = \"0.1.9\"    # A linter\nclauth-extra = \"1.0.0\"\nclauth = \"0.18.0\"    # Manage accounts\n";
        assert_eq!(parse_search(search, "clauth"), Some("0.18.0".to_string()));
        assert_eq!(parse_search(search, "missing"), None);
    }
}
