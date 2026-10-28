use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::manager::{Manager, Package, Step, capture, writable};

pub struct Npm;

impl Manager for Npm {
    fn id(&self) -> &'static str {
        "npm"
    }

    fn name(&self) -> &'static str {
        "npm (global)"
    }

    fn binary(&self) -> &'static str {
        "npm"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let json = capture(bin, ["outdated", "--global", "--json"])?.expect(&[0, 1])?;
        parse_outdated(&json)
    }

    fn upgrade_steps(&self, packages: &[Package]) -> Vec<Step> {
        vec![install("install", packages)]
    }

    /// A system Node (e.g. /usr/local) keeps globals in a root-owned prefix; fnm and nvm don't.
    fn needs_root(&self, bin: &Path) -> bool {
        if cfg!(windows) {
            return false;
        }
        match capture(bin, ["prefix", "--global"]).and_then(|o| o.expect(&[0])) {
            Ok(prefix) => !writable(&Path::new(prefix.trim()).join("lib/node_modules")),
            Err(_) => false,
        }
    }
}

/// `update -g` stays within the current major, so globals are reinstalled at the version
/// `outdated` showed. `@latest` could resolve to a newer release that the running Node rejects.
pub(super) fn install(verb: &str, packages: &[Package]) -> Step {
    let mut args = vec![verb.to_string(), "--global".to_string()];
    args.extend(packages.iter().map(|p| match p.latest.as_str() {
        "" => format!("{}@latest", p.name),
        version => format!("{}@{version}", p.name),
    }));
    Step {
        args,
        packages: packages.to_vec(),
    }
}

#[derive(Deserialize)]
struct Entry {
    current: Option<String>,
    latest: Option<String>,
}

/// pnpm prints the same shape, keyed by package name.
pub(super) fn parse_outdated(json: &str) -> Result<Vec<Package>, String> {
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let entries: BTreeMap<String, Entry> =
        serde_json::from_str(json).map_err(|e| format!("unexpected output: {e}"))?;
    Ok(entries
        .into_iter()
        .map(|(name, entry)| Package {
            name,
            current: entry.current.unwrap_or_default(),
            latest: entry.latest.unwrap_or_default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated_globals() {
        let json = r#"{
            "corepack": {"current": "0.30.0", "wanted": "0.34.7", "latest": "0.34.7", "dependent": "global"},
            "eslint": {"current": "9.20.0", "wanted": "10.13.0", "latest": "10.13.0", "dependent": "global"}
        }"#;
        let packages = parse_outdated(json).unwrap();
        assert_eq!(packages.len(), 2);
        assert_eq!(
            packages[0],
            Package {
                name: "corepack".into(),
                current: "0.30.0".into(),
                latest: "0.34.7".into()
            }
        );
    }

    #[test]
    fn empty_output_means_up_to_date() {
        assert!(parse_outdated("").unwrap().is_empty());
        assert!(parse_outdated("{}").unwrap().is_empty());
    }

    fn package(name: &str, latest: &str) -> Package {
        Package {
            name: name.into(),
            current: "0.0.1".into(),
            latest: latest.into(),
        }
    }

    #[test]
    fn installs_the_reported_version() {
        let step = install(
            "install",
            &[
                package("@google/gemini-cli", "0.66.0"),
                package("eslint", ""),
            ],
        );
        assert_eq!(
            step.args,
            [
                "install",
                "--global",
                "@google/gemini-cli@0.66.0",
                "eslint@latest"
            ]
        );
        assert_eq!(step.packages.len(), 2);
    }

    #[test]
    fn rejects_error_payload() {
        assert!(parse_outdated(r#"{"error": "boom"}"#).is_err());
    }
}
