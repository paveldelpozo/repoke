use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::manager::{self, Installed, Manager, Package, Step, capture, lines};

/// Global packages of Yarn 1. Yarn 2 and later dropped `yarn global`, so they aren't offered.
pub struct Yarn;

impl Manager for Yarn {
    fn id(&self) -> &'static str {
        "yarn"
    }

    fn name(&self) -> &'static str {
        "yarn (global)"
    }

    fn binary(&self) -> &'static str {
        "yarn"
    }

    fn locate(&self) -> Option<PathBuf> {
        let bin = manager::locate("yarn")?;
        let version = capture(&bin, ["--version"]).ok()?.stdout;
        version.trim().starts_with("1.").then_some(bin)
    }

    /// The global folder is a plain Yarn project, so `outdated` works inside it.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let dir = capture(bin, ["global", "dir"])
            .and_then(|o| o.expect(&[0]))
            .map_err(explain)?;
        let json = capture(bin, ["--cwd", dir.trim(), "outdated", "--json"])
            .and_then(|o| o.expect(&[0, 1]))
            .map_err(explain)?;
        Ok(parse_outdated(&json))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let specs = packages.iter().map(|p| format!("{}@{}", p.name, p.latest));
        vec![Step::new(
            ["global".to_string(), "add".to_string()]
                .into_iter()
                .chain(specs),
            packages,
        )]
    }

    /// Yarn has no `view` for one field, so npm reads `engines.node` when it is there.
    fn incompatible(&self, bin: &Path, packages: &[Package]) -> Vec<(Package, String)> {
        match manager::locate("npm") {
            Some(npm) => super::engines::incompatible(bin, &npm, packages),
            None => Vec::new(),
        }
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        Ok(parse_list(
            &lines(bin, ["global", "list"]).map_err(explain)?,
        ))
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["global", "add"], packages)
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[".yarnrc"]
    }
}

/// Yarn 1 stops on any `${VAR}` in `.npmrc` that isn't set, before doing anything.
fn explain(error: String) -> String {
    let Some(start) = error.find("Failed to replace env in config: ${") else {
        return error;
    };
    let var = error[start..]
        .split(['{', '}'])
        .nth(1)
        .unwrap_or("a variable");
    format!("yarn 1 can't read your .npmrc: {var} isn't set. Export it, even empty, and try again")
}

/// The `table` event's rows are `[name, current, wanted, latest, type, url]`.
fn parse_outdated(json: &str) -> Vec<Package> {
    json.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|event| event["type"] == "table")
        .flat_map(|event| {
            event["data"]["body"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|row| {
            let cell = |i: usize| row.get(i)?.as_str().map(String::from);
            Some(Package {
                name: cell(0)?,
                current: cell(1)?,
                latest: cell(3)?,
            })
        })
        .collect()
}

/// `info "create-vite@4.1.0" has binaries:`; a scoped name has its own `@`.
fn parse_list(lines: &[String]) -> Vec<Installed> {
    lines
        .iter()
        .filter_map(|l| {
            let spec = l.strip_prefix("info \"")?.split('"').next()?;
            let (name, version) = spec.rsplit_once('@')?;
            (!name.is_empty()).then(|| Installed::new(name).version(version))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_outdated_table() {
        let json = r#"{"type":"warning","data":"package.json: No license field"}
{"type":"table","data":{"head":["Package","Current","Wanted","Latest","Package Type","URL"],"body":[["create-vite","4.1.0","4.4.1","9.3.3","dependencies","https://github.com/vitejs/vite"],["typescript","5.2.2","5.9.3","7.0.2","dependencies","https://www.typescriptlang.org/"]]}}"#;
        let packages = parse_outdated(json);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "create-vite");
        assert_eq!(packages[0].current, "4.1.0");
        assert_eq!(packages[0].latest, "9.3.3");
    }

    #[test]
    fn reads_global_list() {
        let lines = [
            "yarn global v1.22.22".to_string(),
            "info \"create-vite@4.1.0\" has binaries:".to_string(),
            "   - create-vite".to_string(),
            "info \"@vue/cli@5.0.8\" has binaries:".to_string(),
        ];
        let names: Vec<String> = parse_list(&lines).iter().map(|p| p.to_string()).collect();
        assert_eq!(names, ["create-vite==4.1.0", "@vue/cli==5.0.8"]);
    }

    #[test]
    fn explains_unset_npmrc_variables() {
        let error = "exited with 1: error Error: Failed to replace env in config: ${NODE_AUTH_TOKEN}\n    at cli.js";
        assert_eq!(
            explain(error.into()),
            "yarn 1 can't read your .npmrc: NODE_AUTH_TOKEN isn't set. Export it, even empty, and try again"
        );
    }
}
