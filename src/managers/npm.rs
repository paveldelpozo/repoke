use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::Deserialize;

use crate::manager::{Blocked, Installed, Manager, Package, Printed, Step, capture, writable};

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

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        vec![reporting(install("install", packages))]
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

    fn incompatible(&self, bin: &Path, packages: &[Package]) -> Vec<(Package, String)> {
        super::engines::incompatible(bin, bin, packages)
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let json = capture(bin, ["ls", "--global", "--depth=0", "--json"])?.expect(&[0])?;
        parse_globals(&json)
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["install", "--global"], packages)
            .into_iter()
            .map(reporting)
            .collect()
    }

    fn blocked(&self, packages: &[Package], printed: &Printed) -> Vec<Blocked> {
        blocked_scripts(packages, &printed.stdout)
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[".npmrc"]
    }
}

/// `update -g` stays within the current major, so globals are reinstalled at the version
/// `outdated` showed. `@latest` could resolve to a newer release that the running Node rejects.
pub(super) fn install(verb: &str, packages: &[Package]) -> Step {
    let specs = packages.iter().map(|p| match p.latest.as_str() {
        "" => format!("{}@latest", p.name),
        version => format!("{}@{version}", p.name),
    });
    Step::new(
        [verb.to_string(), "--global".to_string()]
            .into_iter()
            .chain(specs),
        packages,
    )
}

/// With `--json`, npm 12 and later list the install scripts they refused to run under
/// `unreviewedScripts`; older releases print the same report without it.
fn reporting(mut step: Step) -> Step {
    for command in &mut step.commands {
        command.push("--json".into());
    }
    step
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Report {
    add: Vec<Placed>,
    change: Vec<Changed>,
    unreviewed_scripts: Vec<Unreviewed>,
}

#[derive(Default, Deserialize)]
struct Placed {
    path: String,
}

#[derive(Deserialize)]
struct Changed {
    to: Placed,
}

#[derive(Deserialize)]
struct Unreviewed {
    name: Option<String>,
    path: String,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
}

/// `unreviewedScripts` covers the whole global folder, so only the folders this install placed
/// count; each one belongs to the package whose folder holds it.
fn blocked_scripts(packages: &[Package], stdout: &str) -> Vec<Blocked> {
    let Some(start) = stdout.find('{') else {
        return Vec::new();
    };
    let Some(Ok(report)) = serde_json::Deserializer::from_str(&stdout[start..])
        .into_iter::<Report>()
        .next()
    else {
        return Vec::new();
    };
    let placed: HashSet<String> = report
        .add
        .into_iter()
        .chain(report.change.into_iter().map(|c| c.to))
        .map(|p| normalize(&p.path))
        .collect();
    let mut found: Vec<(Package, Vec<Unreviewed>)> = Vec::new();
    for entry in report.unreviewed_scripts {
        let path = normalize(&entry.path);
        if !placed.contains(&path) {
            continue;
        }
        let Some(owner) = owner(packages, &path) else {
            continue;
        };
        match found.iter_mut().find(|(p, _)| p == owner) {
            Some((_, entries)) => entries.push(entry),
            None => found.push((owner.clone(), vec![entry])),
        }
    }
    found
        .into_iter()
        .map(|(package, entries)| explain_blocked(package, &entries))
        .collect()
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

/// The package whose folder comes first in `path`: in `.../node_modules/a/node_modules/b`, `a`.
fn owner<'p>(packages: &'p [Package], path: &str) -> Option<&'p Package> {
    packages
        .iter()
        .filter_map(|p| {
            let folder = format!("/node_modules/{}", p.name);
            path.match_indices(&folder)
                .find(|(i, _)| matches!(path[i + folder.len()..].chars().next(), None | Some('/')))
                .map(|(i, _)| (i, p))
        })
        .min_by_key(|(i, _)| *i)
        .map(|(_, p)| p)
}

fn explain_blocked(package: Package, entries: &[Unreviewed]) -> Blocked {
    let names: Vec<&str> = entries
        .iter()
        .map(|e| e.name.as_deref().unwrap_or(&package.name))
        .collect();
    let scripts: Vec<String> = entries
        .iter()
        .zip(&names)
        .map(|(e, name)| {
            let events: Vec<&str> = e.scripts.keys().map(String::as_str).collect();
            format!("{name} ({})", events.join(", "))
        })
        .collect();
    let allow = names.join(",");
    let spec = match package.latest.as_str() {
        "" => package.name.clone(),
        version => format!("{}@{version}", package.name),
    };
    Blocked {
        reason: format!(
            "installed, but npm blocked the install scripts of {}, so it may not work",
            scripts.join(", ")
        ),
        hint: format!(
            "run them now with `npm install --global --allow-scripts={allow} {spec}`, or allow them from now on by adding `allow-scripts={allow}` to ~/.npmrc"
        ),
        package,
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

#[derive(Deserialize)]
pub(super) struct Globals {
    #[serde(default)]
    dependencies: BTreeMap<String, Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    version: Option<String>,
}

impl Globals {
    /// npm and corepack ship with Node itself; pnpm lists itself when it was added globally.
    pub(super) fn packages(self) -> Vec<Installed> {
        self.dependencies
            .into_iter()
            .filter(|(name, _)| !matches!(name.as_str(), "npm" | "corepack" | "pnpm"))
            .map(|(name, dep)| Installed::new(name).version(dep.version.unwrap_or_default()))
            .collect()
    }
}

pub(super) fn parse_globals(json: &str) -> Result<Vec<Installed>, String> {
    serde_json::from_str::<Globals>(json)
        .map(Globals::packages)
        .map_err(|e| format!("unexpected npm output: {e}"))
}

#[cfg(test)]
mod tests {
    use std::slice;

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
            step.commands[0],
            [
                "install",
                "--global",
                "@google/gemini-cli@0.66.0",
                "eslint@latest"
            ]
        );
        assert_eq!(step.packages.len(), 2);
    }

    /// What npm 12 printed for the 2026-10-30 scheduled run, plus a global it didn't touch.
    const BLOCKED: &str = r#"{
  "add": [],
  "added": 0,
  "change": [
    {
      "from": {"name": "@anthropic-ai/claude-code", "version": "2.1.312", "path": "/fnm/lib/node_modules/@anthropic-ai/claude-code"},
      "to": {"name": "@anthropic-ai/claude-code", "version": "2.1.314", "path": "/fnm/lib/node_modules/@anthropic-ai/claude-code"}
    }
  ],
  "changed": 1,
  "remove": [],
  "removed": 0,
  "audited": 3,
  "funding": 0,
  "unreviewedScripts": [
    {"name": "@anthropic-ai/claude-code", "version": "2.1.314", "path": "/fnm/lib/node_modules/@anthropic-ai/claude-code", "scripts": {"postinstall": "node install.cjs"}},
    {"name": "esbuild", "version": "0.25.0", "path": "/fnm/lib/node_modules/vite/node_modules/esbuild", "scripts": {"postinstall": "node install.js"}}
  ]
}"#;

    #[test]
    fn upgrades_ask_npm_for_its_report() {
        let steps = Npm.upgrade_steps(Path::new("npm"), &[package("eslint", "10.13.0")]);
        assert_eq!(
            steps[0].commands[0],
            ["install", "--global", "eslint@10.13.0", "--json"]
        );
        let steps = Npm.install_steps(Path::new("npm"), &[Installed::new("eslint")]);
        assert_eq!(
            steps[0].commands[0],
            ["install", "--global", "eslint", "--json"]
        );
    }

    #[test]
    fn reports_packages_whose_scripts_npm_blocked() {
        let claude = package("@anthropic-ai/claude-code", "2.1.314");
        let blocked = blocked_scripts(slice::from_ref(&claude), BLOCKED);
        assert_eq!(blocked.len(), 1, "vite was not installed by this step");
        assert_eq!(blocked[0].package, claude);
        assert_eq!(
            blocked[0].reason,
            "installed, but npm blocked the install scripts of @anthropic-ai/claude-code (postinstall), so it may not work"
        );
        assert_eq!(
            blocked[0].hint,
            "run them now with `npm install --global --allow-scripts=@anthropic-ai/claude-code @anthropic-ai/claude-code@2.1.314`, or allow them from now on by adding `allow-scripts=@anthropic-ai/claude-code` to ~/.npmrc"
        );
    }

    #[test]
    fn a_blocked_dependency_belongs_to_the_package_that_holds_it() {
        let json = r#"{"add": [{"name": "esbuild", "version": "0.25.0", "path": "C:\\npm\\node_modules\\vite\\node_modules\\esbuild"}],
            "unreviewedScripts": [{"name": "esbuild", "version": "0.25.0", "path": "C:\\npm\\node_modules\\vite\\node_modules\\esbuild", "scripts": {"postinstall": "node install.js"}}]}"#;
        let packages = [package("esbuild", "0.25.0"), package("vite", "8.0.0")];
        let blocked = blocked_scripts(&packages, json);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].package.name, "vite");
        assert!(
            blocked[0]
                .hint
                .contains("--allow-scripts=esbuild vite@8.0.0")
        );
    }

    #[test]
    fn allowed_scripts_and_older_npm_block_nothing() {
        // allow-scripts in ~/.npmrc: npm ran the script and lists nothing.
        let allowed = r#"{"added": 0, "changed": 1, "change": [{"from": {"path": "/g/node_modules/a"}, "to": {"path": "/g/node_modules/a"}}], "audited": 1, "funding": 0}"#;
        // npm 11 and earlier run every script and don't know the field.
        let older = r#"{"added": 1, "removed": 0, "changed": 0, "audited": 1, "funding": 0}"#;
        for stdout in [allowed, older, "", "added 1 package in 2s"] {
            assert!(
                blocked_scripts(&[package("a", "2.0.0")], stdout).is_empty(),
                "{stdout}"
            );
        }
    }

    #[test]
    fn rejects_error_payload() {
        assert!(parse_outdated(r#"{"error": "boom"}"#).is_err());
    }
}
