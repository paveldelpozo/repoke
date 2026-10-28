use std::path::Path;
use std::thread;

use crate::manager::{Installed, Manager, Package, Step, capture};

pub struct Cargo;

const SEARCHES_AT_ONCE: usize = 8;

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

    /// One `cargo search` per crate, a few at a time: crates.io limits how fast it is asked.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let list = capture(bin, ["install", "--list"])?.expect(&[0])?;
        let mut packages = Vec::new();
        for batch in parse_installed(&list).chunks(SEARCHES_AT_ONCE) {
            let found = thread::scope(|scope| {
                let searches: Vec<_> = batch
                    .iter()
                    .map(|(name, _)| {
                        scope.spawn(move || {
                            let search =
                                capture(bin, ["search", name, "--limit", "10"])?.expect(&[0])?;
                            Ok::<_, String>(parse_search(&search, name))
                        })
                    })
                    .collect();
                searches
                    .into_iter()
                    .map(|s| {
                        s.join()
                            .unwrap_or_else(|_| Err("cargo search panicked".into()))
                    })
                    .collect::<Result<Vec<_>, String>>()
            })?;
            for ((name, current), latest) in batch.iter().zip(found) {
                if let Some(latest) = latest
                    && latest != *current
                {
                    packages.push(Package {
                        name: name.clone(),
                        current: current.clone(),
                        latest,
                    });
                }
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

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let list = capture(bin, ["install", "--list"])?.expect(&[0])?;
        Ok(parse_installed(&list)
            .into_iter()
            .map(|(name, version)| Installed::new(name).version(version))
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["install"], packages)
    }

    /// Not `credentials.toml`, which holds the crates.io token.
    fn config_files(&self) -> &'static [&'static str] {
        &[".cargo/config.toml"]
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
