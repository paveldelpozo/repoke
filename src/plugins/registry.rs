//! The official plugin index, github.com/paveldelpozo/repoke-plugins: `plugin search`, `plugin
//! install` and `plugin remove`. `REPOKE_PLUGIN_INDEX` points at another index, a URL or a local
//! file, for a fork or a test.

use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console::style;
use serde::{Deserialize, Serialize};

use super::{loaded, supported, write};
use crate::manager::{self, capture_within};
use crate::self_update::sha256;
use crate::{PluginInstallArgs, config, print_json, snapshot};

const INDEX: &str = "https://raw.githubusercontent.com/paveldelpozo/repoke-plugins/main/index.toml";
const FORMAT: u32 = 1;

#[derive(Deserialize)]
struct Index {
    format: u32,
    #[serde(default)]
    plugin: Vec<Entry>,
}

#[derive(Deserialize, Serialize, Clone)]
struct Entry {
    id: String,
    name: String,
    /// `executable` or `toml`.
    kind: String,
    file: String,
    #[serde(default)]
    os: Vec<String>,
    #[serde(default)]
    requires: Vec<String>,
    description: String,
    /// Of the file as it was reviewed; `install` refuses anything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

/// A remote index has to pin every file; a local one, used while writing a plugin, may not.
fn verify(entry: &Entry, location: &str, code: &str) -> Result<(), String> {
    let actual = sha256(code.as_bytes());
    match &entry.sha256 {
        Some(expected) if expected.eq_ignore_ascii_case(&actual) => Ok(()),
        Some(expected) => Err(format!(
            "`{}` doesn't match the reviewed version (sha256 {actual}, the index expects {expected}): it changed after review, so it won't be installed",
            entry.id
        )),
        None if location.contains("://") => Err(format!(
            "`{}` has no sha256 in {location}, so there is no reviewed version to check it against",
            entry.id
        )),
        None => {
            eprintln!(
                "{}",
                style(format!(
                    "`{}` has no sha256 in the local index; not verified.",
                    entry.id
                ))
                .yellow()
            );
            Ok(())
        }
    }
}

fn location() -> String {
    env::var("REPOKE_PLUGIN_INDEX").unwrap_or_else(|_| INDEX.to_string())
}

fn fetch(location: &str) -> Result<String, String> {
    if location.starts_with("https://") || location.starts_with("http://") {
        capture_within("curl", ["-fsSL", "--max-time", "60", location], None)?
            .expect(&[0])
            .map_err(|e| format!("could not download {location}: {e}"))
    } else {
        fs::read_to_string(location).map_err(|e| format!("{location}: {e}"))
    }
}

/// `file` is relative to the index, whether that is a URL or a path.
fn beside(index: &str, file: &str) -> String {
    if index.contains("://") {
        let base = index.rsplit_once('/').map_or(index, |(base, _)| base);
        format!("{base}/{file}")
    } else {
        Path::new(index)
            .parent()
            .unwrap_or(Path::new("."))
            .join(file)
            .display()
            .to_string()
    }
}

fn index() -> Result<(String, Vec<Entry>), String> {
    let location = location();
    let index: Index = toml::from_str(&fetch(&location)?)
        .map_err(|e| format!("{location} isn't a plugin index: {e}"))?;
    if index.format > FORMAT {
        return Err(format!(
            "the plugin index uses format {}, this repoke knows up to {FORMAT}: update repoke",
            index.format
        ));
    }
    Ok((location, index.plugin))
}

/// Where `plugin install` puts a plugin: executables in `~/.local/bin`, TOML in the config folder.
fn target(entry: &Entry) -> Result<PathBuf, String> {
    match entry.kind.as_str() {
        "executable" => Ok(snapshot::home()
            .ok_or("cannot find the home directory")?
            .join(".local/bin")
            .join(format!("repoke-{}", entry.id))),
        "toml" => Ok(config::dir()
            .ok_or("cannot find the config directory")?
            .join("managers")
            .join(format!("{}.toml", entry.id))),
        kind => Err(format!("unknown plugin kind `{kind}`")),
    }
}

pub fn search(json: bool) -> Result<ExitCode, String> {
    let (_, entries) = index()?;
    if json {
        print_json(&entries);
        return Ok(ExitCode::SUCCESS);
    }
    let active: Vec<&str> = loaded().managers.iter().map(|m| m.id()).collect();
    let width = entries.iter().map(|e| e.id.len()).max().unwrap_or(0);
    for entry in &entries {
        let state = if active.contains(&entry.id.as_str()) {
            style("installed").green().to_string()
        } else if !supported(&entry.os) {
            style(format!("not for {}", env::consts::OS))
                .dim()
                .to_string()
        } else {
            String::new()
        };
        println!("  {:<width$}  {}  {state}", entry.id, entry.description);
    }
    println!(
        "\n{}",
        style("Install one with `repoke plugin install <id>`.").dim()
    );
    Ok(ExitCode::SUCCESS)
}

pub fn install(args: &PluginInstallArgs) -> Result<ExitCode, String> {
    let (location, entries) = index()?;
    let entry = entries
        .iter()
        .find(|e| e.id == args.id)
        .ok_or(format!("there is no plugin `{}` in {location}", args.id))?;
    if !supported(&entry.os) {
        return Err(format!(
            "`{}` is for {}, not {}",
            entry.id,
            entry.os.join(", "),
            env::consts::OS
        ));
    }
    let target = target(entry)?;
    if target.exists() && !args.force {
        return Err(format!(
            "{} already exists; pass --force to replace it",
            target.display()
        ));
    }
    let missing: Vec<&str> = entry
        .requires
        .iter()
        .map(String::as_str)
        .filter(|tool| manager::locate(tool).is_none())
        .collect();

    let source = beside(&location, &entry.file);
    let code = fetch(&source)?;
    verify(entry, &location, &code)?;
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if !args.yes {
        if !interactive {
            return Err(
                "not running in a terminal, pass --yes to install without reviewing".into(),
            );
        }
        println!("{} {}\n", style("Source:").bold(), source);
        for (n, line) in code.lines().enumerate() {
            println!("{} {line}", style(format!("{:>4}", n + 1)).dim());
        }
        println!();
        if !missing.is_empty() {
            println!(
                "{}",
                style(format!(
                    "It needs {}, which isn't on your PATH.",
                    missing.join(", ")
                ))
                .yellow()
            );
        }
        let go = inquire::Confirm::new(&format!(
            "This runs as you whenever repoke checks or upgrades. Install it as {}?",
            target.display()
        ))
        .with_default(false)
        .prompt()
        .unwrap_or(false);
        if !go {
            println!("Nothing installed.");
            return Ok(ExitCode::SUCCESS);
        }
    }

    write(&target, &code, entry.kind == "executable")?;
    println!("Installed {} as {}.", entry.name, target.display());
    if entry.kind == "executable" && !on_path(target.parent().unwrap_or(Path::new(""))) {
        println!(
            "{}",
            style(format!(
                "{} isn't on your PATH, so repoke won't find it until you add it.",
                target
                    .parent()
                    .map(Path::display)
                    .map_or(String::new(), |d| d.to_string())
            ))
            .yellow()
        );
    }
    if !missing.is_empty() {
        println!(
            "{}",
            style(format!("It needs {} to work.", missing.join(", "))).yellow()
        );
    }
    println!(
        "{}",
        style(format!("Try it with `repoke check --only {}`.", entry.id)).dim()
    );
    Ok(ExitCode::SUCCESS)
}

/// Removes only what `install` would have written, never a plugin found elsewhere on the PATH.
pub fn remove(id: &str) -> Result<ExitCode, String> {
    let kinds = ["executable", "toml"];
    let mut removed = false;
    for kind in kinds {
        let entry = Entry {
            id: id.to_string(),
            name: id.to_string(),
            kind: kind.to_string(),
            file: String::new(),
            os: Vec::new(),
            requires: Vec::new(),
            description: String::new(),
            sha256: None,
        };
        let path = target(&entry)?;
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("Removed {}.", path.display());
            removed = true;
        }
    }
    if !removed {
        return Err(format!(
            "`{id}` isn't installed in ~/.local/bin or the managers folder; `repoke plugin list` shows where each plugin is"
        ));
    }
    Ok(ExitCode::SUCCESS)
}

fn on_path(dir: &Path) -> bool {
    env::var_os("PATH").is_some_and(|path| env::split_paths(&path).any(|p| p == dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_sit_beside_the_index() {
        assert_eq!(
            beside(INDEX, "plugins/omz/repoke-omz"),
            "https://raw.githubusercontent.com/paveldelpozo/repoke-plugins/main/plugins/omz/repoke-omz"
        );
        assert_eq!(beside("/tmp/x/index.toml", "plugins/a"), "/tmp/x/plugins/a");
    }

    #[test]
    fn parses_an_index() {
        let index: Index = toml::from_str(
            r#"
format = 1
[[plugin]]
id = "omz"
name = "oh-my-zsh"
kind = "executable"
file = "plugins/omz/repoke-omz"
os = ["macos", "linux"]
requires = ["git"]
description = "oh-my-zsh"
"#,
        )
        .unwrap();
        assert_eq!(index.plugin[0].requires, ["git"]);
    }

    #[test]
    fn installs_only_the_reviewed_file() {
        let mut entry: Entry = toml::from_str(
            r#"
id = "omz"
name = "oh-my-zsh"
kind = "executable"
file = "plugins/omz/repoke-omz"
description = "oh-my-zsh"
"#,
        )
        .unwrap();
        let code = "#!/bin/sh\necho reviewed\n";
        assert!(verify(&entry, INDEX, code).is_err());
        assert!(verify(&entry, "/tmp/index.toml", code).is_ok());
        entry.sha256 = Some(sha256(code.as_bytes()).to_uppercase());
        assert!(verify(&entry, INDEX, code).is_ok());
        assert!(verify(&entry, INDEX, "#!/bin/sh\necho changed\n").is_err());
    }
}
