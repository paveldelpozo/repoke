//! Managers declared in TOML: which commands list what is outdated, how to read their output
//! (JSON or a regular expression per line) and which commands upgrade.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex_lite::Regex;
use serde::Deserialize;
use serde_json::Value;

use super::{Candidate, leak, supported, timeout};
use crate::manager::{self, Installed, Manager, Package, Step, capture_within};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    id: String,
    name: String,
    /// The program every command runs; the manager is installed when it is on the PATH.
    binary: String,
    #[serde(default)]
    os: Vec<String>,
    #[serde(default)]
    needs_root: bool,
    outdated: Query,
    upgrade: Run,
    #[serde(default)]
    refresh: Option<Run>,
    #[serde(default)]
    installed: Option<Query>,
    #[serde(default)]
    install: Option<Run>,
}

/// Arguments for `binary`. `{packages}` expands to every package name; an argument with `{name}`
/// or `{version}` makes one command per package. Without placeholders the command runs once.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Run {
    command: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    command: Vec<String>,
    #[serde(default = "success")]
    ok_codes: Vec<i32>,
    format: Format,
    /// json: JSON pointer to the array of packages ("" for the top level).
    #[serde(default)]
    items: String,
    /// json: field (or JSON pointer) inside each item.
    name: Option<String>,
    current: Option<String>,
    latest: Option<String>,
    version: Option<String>,
    /// lines: regular expression with named groups `name` and `current`/`latest`/`version`.
    pattern: Option<String>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Format {
    Json,
    Lines,
}

fn success() -> Vec<i32> {
    vec![0]
}

pub struct Declared {
    id: &'static str,
    name: &'static str,
    binary: &'static str,
    spec: Spec,
    outdated_pattern: Option<Regex>,
    installed_pattern: Option<Regex>,
}

#[derive(Default, Debug, PartialEq)]
struct Row {
    name: String,
    current: String,
    latest: String,
    version: String,
}

pub fn discover(dir: &Path) -> Vec<Candidate> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let manager = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| parse(&text))
                .map(|d| &*Box::leak(Box::new(d)) as &'static dyn Manager);
            ("toml", path, manager)
        })
        .collect()
}

fn parse(text: &str) -> Result<Declared, String> {
    let spec: Spec = toml::from_str(text).map_err(|e| e.to_string())?;
    if spec.id.is_empty() {
        return Err("`id` is empty".into());
    }
    let outdated_pattern = pattern(&spec.outdated, "outdated")?;
    let installed_pattern = match &spec.installed {
        Some(query) => pattern(query, "installed")?,
        None => None,
    };
    Ok(Declared {
        id: leak(spec.id.clone()),
        name: leak(spec.name.clone()),
        binary: leak(spec.binary.clone()),
        spec,
        outdated_pattern,
        installed_pattern,
    })
}

/// Checks the query up front, so a broken file is reported by `plugin list` and not mid-run.
fn pattern(query: &Query, section: &str) -> Result<Option<Regex>, String> {
    match query.format {
        Format::Json if query.name.is_none() => {
            Err(format!("[{section}] with format = \"json\" needs `name`"))
        }
        Format::Json => Ok(None),
        Format::Lines => {
            let source = query.pattern.as_deref().ok_or(format!(
                "[{section}] with format = \"lines\" needs `pattern`"
            ))?;
            let regex = Regex::new(source).map_err(|e| format!("[{section}] pattern: {e}"))?;
            if !regex.capture_names().any(|n| n == Some("name")) {
                return Err(format!("[{section}] pattern needs a (?P<name>…) group"));
            }
            Ok(Some(regex))
        }
    }
}

fn rows(query: &Query, regex: Option<&Regex>, text: &str) -> Result<Vec<Row>, String> {
    match (query.format, regex) {
        (Format::Lines, Some(regex)) => Ok(text
            .lines()
            .filter_map(|line| {
                let caps = regex.captures(line)?;
                let group = |g: &str| {
                    caps.name(g)
                        .map_or(String::new(), |m| m.as_str().to_string())
                };
                Some(Row {
                    name: group("name"),
                    current: group("current"),
                    latest: group("latest"),
                    version: group("version"),
                })
            })
            .collect()),
        _ => {
            let value: Value =
                serde_json::from_str(text).map_err(|e| format!("the output isn't JSON: {e}"))?;
            let items = value
                .pointer(&pointer(&query.items))
                .and_then(Value::as_array)
                .ok_or(format!("no JSON array at `{}`", query.items))?;
            Ok(items
                .iter()
                .map(|item| {
                    let field = |f: &Option<String>| {
                        f.as_deref().map_or(String::new(), |f| text_at(item, f))
                    };
                    Row {
                        name: field(&query.name),
                        current: field(&query.current),
                        latest: field(&query.latest),
                        version: field(&query.version),
                    }
                })
                .filter(|row| !row.name.is_empty())
                .collect())
        }
    }
}

fn pointer(field: &str) -> String {
    if field.is_empty() || field.starts_with('/') {
        field.to_string()
    } else {
        format!("/{field}")
    }
}

fn text_at(item: &Value, field: &str) -> String {
    match item.pointer(&pointer(field)) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// Commands for `run` over these packages, as (name, version) pairs; see `Run`.
fn expand(run: &Run, packages: &[(String, String)]) -> Vec<(Vec<String>, Vec<usize>)> {
    let per_package = run
        .command
        .iter()
        .any(|arg| arg.contains("{name}") || arg.contains("{version}"));
    if per_package {
        return packages
            .iter()
            .enumerate()
            .map(|(i, (name, version))| {
                let args = run
                    .command
                    .iter()
                    .map(|arg| arg.replace("{name}", name).replace("{version}", version))
                    .collect();
                (args, vec![i])
            })
            .collect();
    }
    let args = run
        .command
        .iter()
        .flat_map(|arg| {
            if arg == "{packages}" {
                packages.iter().map(|(name, _)| name.clone()).collect()
            } else {
                vec![arg.clone()]
            }
        })
        .collect();
    vec![(args, (0..packages.len()).collect())]
}

impl Manager for Declared {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn binary(&self) -> &'static str {
        self.binary
    }

    fn supported(&self) -> bool {
        supported(&self.spec.os)
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let query = &self.spec.outdated;
        let text = capture_within(bin, &query.command, timeout())?.expect(&query.ok_codes)?;
        Ok(rows(query, self.outdated_pattern.as_ref(), &text)?
            .into_iter()
            .map(|row| Package {
                name: row.name,
                current: row.current,
                latest: row.latest,
            })
            .collect())
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let pairs: Vec<(String, String)> = packages
            .iter()
            .map(|p| (p.name.clone(), p.latest.clone()))
            .collect();
        expand(&self.spec.upgrade, &pairs)
            .into_iter()
            .map(|(args, covered)| Step {
                commands: vec![args],
                packages: covered.iter().map(|&i| packages[i].clone()).collect(),
                password: false,
            })
            .collect()
    }

    fn refresh(&self, bin: &Path) -> Result<(), String> {
        match &self.spec.refresh {
            Some(run) => capture_within(bin, &run.command, timeout())?
                .expect(&[0])
                .map(drop),
            None => Ok(()),
        }
    }

    fn timeout(&self) -> Option<Duration> {
        timeout()
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        self.spec.needs_root && manager::needs_sudo()
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let query = self
            .spec
            .installed
            .as_ref()
            .ok_or("this manager has no [installed] section")?;
        let text = capture_within(bin, &query.command, timeout())?.expect(&query.ok_codes)?;
        Ok(rows(query, self.installed_pattern.as_ref(), &text)?
            .into_iter()
            .map(|row| Installed::new(row.name).version(row.version))
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        let Some(run) = &self.spec.install else {
            return Vec::new();
        };
        let pairs: Vec<(String, String)> = packages
            .iter()
            .map(|p| (p.name.clone(), p.version.clone().unwrap_or_default()))
            .collect();
        expand(run, &pairs)
            .into_iter()
            .map(|(args, covered)| {
                let covered: Vec<Installed> =
                    covered.iter().map(|&i| packages[i].clone()).collect();
                Step::install(args, &covered)
            })
            .collect()
    }
}

pub fn template(id: &str) -> String {
    format!(
        r#"# repoke manager `{id}`. Reference: https://github.com/paveldelpozo/repoke/blob/main/docs/plugins.md
id = "{id}"
name = "{id}"
# The program every command below runs; the manager counts as installed when it is on the PATH.
binary = "{id}"
# os = ["macos", "linux", "windows"]   # all by default
# needs_root = true                   # upgrade through sudo (skipped with --no-sudo)

[outdated]
command = ["outdated", "--json"]
# ok_codes = [0, 1]                   # exit codes that mean success
format = "json"                       # or "lines"
items = ""                            # JSON pointer to the list, "" for the top level
name = "name"                         # fields (or JSON pointers) inside each item
current = "current"
latest = "latest"
# format = "lines"
# pattern = '^(?P<name>\S+) (?P<current>\S+) -> (?P<latest>\S+)$'

[upgrade]
# {{packages}} expands to every package name; {{name}} or {{version}} runs one command per package.
command = ["upgrade", "{{packages}}"]

# [refresh]                           # updates the index before `check --refresh`
# command = ["update"]

# [installed]                         # for `repoke export`
# command = ["list", "--json"]
# format = "json"
# name = "name"
# version = "version"

# [install]                           # for `repoke init`
# command = ["install", "{{packages}}"]
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_with_pointers() {
        let declared = parse(
            r#"
id = "casks"
name = "Casks"
binary = "brew"
[outdated]
command = ["outdated"]
format = "json"
items = "/casks"
name = "name"
current = "/installed_versions/0"
latest = "current_version"
[upgrade]
command = ["upgrade", "--cask", "{packages}"]
"#,
        )
        .unwrap();
        let output = r#"{"casks":[{"name":"caskhub","installed_versions":["0.9.0"],"current_version":"0.9.3"}]}"#;
        let found = rows(&declared.spec.outdated, None, output).unwrap();
        assert_eq!(
            found,
            [Row {
                name: "caskhub".into(),
                current: "0.9.0".into(),
                latest: "0.9.3".into(),
                version: String::new()
            }]
        );
        let packages: Vec<Package> = found
            .into_iter()
            .map(|r| Package {
                name: r.name,
                current: r.current,
                latest: r.latest,
            })
            .collect();
        let steps = declared.upgrade_steps(Path::new("brew"), &packages);
        assert_eq!(steps[0].commands[0], ["upgrade", "--cask", "caskhub"]);
    }

    #[test]
    fn reads_lines_and_expands_per_package() {
        let declared = parse(
            r#"
id = "ext"
name = "Extensions"
binary = "gh"
[outdated]
command = ["extension", "upgrade", "--all", "--dry-run"]
format = "lines"
pattern = '^\[(?P<name>[^\]]+)\]: would have upgraded from (?P<current>\S+) to (?P<latest>\S+)'
[upgrade]
command = ["extension", "upgrade", "{name}"]
"#,
        )
        .unwrap();
        let text = "[gh-dash]: would have upgraded from v4.1.0 to v4.2.0\n[gh-copilot]: already up to date\n";
        let found = rows(
            &declared.spec.outdated,
            declared.outdated_pattern.as_ref(),
            text,
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].current.as_str(), found[0].latest.as_str()),
            ("v4.1.0", "v4.2.0")
        );
        let packages = [
            Package {
                name: "a".into(),
                current: "1".into(),
                latest: "2".into(),
            },
            Package {
                name: "b".into(),
                current: "1".into(),
                latest: "3".into(),
            },
        ];
        let steps = declared.upgrade_steps(Path::new("gh"), &packages);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].commands[0], ["extension", "upgrade", "b"]);
        assert_eq!(steps[1].packages[0].name, "b");
    }

    #[test]
    fn reports_broken_files() {
        let base = "id = \"x\"\nname = \"X\"\nbinary = \"x\"\n[upgrade]\ncommand = [\"up\"]\n";
        let missing_name = format!("{base}[outdated]\ncommand = [\"o\"]\nformat = \"json\"\n");
        assert!(parse(&missing_name).err().unwrap().contains("needs `name`"));
        let no_group = format!(
            "{base}[outdated]\ncommand = [\"o\"]\nformat = \"lines\"\npattern = '(\\S+)'\n"
        );
        assert!(parse(&no_group).err().unwrap().contains("(?P<name>"));
        assert!(parse("id = \"x\"\nbogus = 1\n").is_err());
    }

    #[test]
    fn the_template_parses() {
        assert!(parse(&template("demo")).is_ok());
    }
}
