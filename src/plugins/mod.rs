//! Managers that live outside repoke: `repoke-<id>` executables on the PATH, which answer the JSON
//! protocol in docs/plugins.md, and managers declared in `~/.config/repoke/managers/*.toml`.

mod declared;
mod exec;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Duration;

use console::style;
use serde::Serialize;

use crate::manager::Manager;
use crate::{PluginNewArgs, config, managers, print_json};

#[derive(Default)]
pub struct Loaded {
    pub managers: Vec<&'static dyn Manager>,
    sources: Vec<Source>,
}

#[derive(Serialize)]
struct Source {
    kind: &'static str,
    path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    problem: Option<String>,
}

/// One discovered plugin: where it is and the manager it gives, or why it can't be used.
type Candidate = (&'static str, PathBuf, Result<&'static dyn Manager, String>);

/// Discovery runs a describe call per executable, so it happens once per process.
pub fn loaded() -> &'static Loaded {
    static LOADED: OnceLock<Loaded> = OnceLock::new();
    LOADED.get_or_init(load)
}

fn load() -> Loaded {
    let mut loaded = Loaded::default();
    if env::var_os("REPOKE_NO_PLUGINS").is_some() {
        return loaded;
    }
    let builtin: Vec<&str> = managers::every().iter().map(|m| m.id()).collect();
    let candidates = exec::discover().into_iter().chain(
        config::dir()
            .map(|d| declared::discover(&d.join("managers")))
            .unwrap_or_default(),
    );
    for (kind, path, result) in candidates {
        let (id, problem) = match result {
            Ok(m) if builtin.contains(&m.id()) => (
                Some(m.id()),
                Some(format!(
                    "repoke already has a `{}` manager; rename the plugin",
                    m.id()
                )),
            ),
            Ok(m) if loaded.managers.iter().any(|other| other.id() == m.id()) => (
                Some(m.id()),
                Some(format!("another plugin already provides `{}`", m.id())),
            ),
            Ok(m) => {
                loaded.managers.push(m);
                (Some(m.id()), None)
            }
            Err(e) => (None, Some(e)),
        };
        loaded.sources.push(Source {
            kind,
            path,
            id,
            problem,
        });
    }
    loaded
}

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The longest a plugin command may run: `REPOKE_PLUGIN_TIMEOUT`, else `plugin_timeout` in the
/// config file, else 10 minutes. `0` means no limit.
pub fn timeout() -> Option<Duration> {
    static TIMEOUT: OnceLock<Option<Duration>> = OnceLock::new();
    *TIMEOUT.get_or_init(|| {
        let Ok(text) = env::var("REPOKE_PLUGIN_TIMEOUT") else {
            // A bad value in the config file is reported when the config is loaded.
            return config::load()
                .ok()
                .and_then(|c| c.plugin_timeout)
                .and_then(|t| config::parse_duration(&t).ok())
                .unwrap_or(Some(DEFAULT_TIMEOUT));
        };
        config::parse_duration(&text).unwrap_or_else(|e| {
            eprintln!("repoke: REPOKE_PLUGIN_TIMEOUT: {e}; using 10m");
            Some(DEFAULT_TIMEOUT)
        })
    })
}

/// Plugins are loaded once and live for the whole run, which is what `&'static` needs.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

pub fn list(json: bool) -> Result<ExitCode, String> {
    let sources = &loaded().sources;
    if json {
        print_json(sources);
        return Ok(ExitCode::SUCCESS);
    }
    if sources.is_empty() {
        let dir = config::dir().map(|d| d.join("managers"));
        println!("No plugins found.");
        println!(
            "{}",
            style(format!(
                "repoke looks for `repoke-<id>` executables on your PATH and for .toml files in {}. Start one with `repoke plugin new <id>`.",
                dir.map_or("~/.config/repoke/managers".into(), |d| d.display().to_string())
            ))
            .dim()
        );
        return Ok(ExitCode::SUCCESS);
    }
    for source in sources {
        let id = source.id.unwrap_or("?");
        match &source.problem {
            None => println!(
                "  {} {id:<12} {:<10} {}",
                style("✓").green(),
                source.kind,
                style(source.path.display()).dim()
            ),
            Some(problem) => {
                println!(
                    "  {} {id:<12} {:<10} {}",
                    style("✗").red(),
                    source.kind,
                    style(source.path.display()).dim()
                );
                println!("      {}", style(problem).red());
            }
        }
    }
    Ok(if sources.iter().any(|s| s.problem.is_some()) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

pub fn new(args: &PluginNewArgs) -> Result<ExitCode, String> {
    let id = &args.id;
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("a plugin id uses lowercase letters, digits and dashes, like `gh-ext`".into());
    }
    let (path, text) = if args.exec {
        let path = args
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from(format!("repoke-{id}")));
        (path, exec::template(id))
    } else {
        let path = match &args.output {
            Some(path) => path.clone(),
            None => config::dir()
                .ok_or("cannot find the config directory")?
                .join("managers")
                .join(format!("{id}.toml")),
        };
        (path, declared::template(id))
    };
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    write(&path, &text, args.exec)?;
    println!("Created {}.", path.display());
    let next = if args.exec {
        "Fill in its commands, then put it on your PATH (e.g. in ~/.local/bin). Check it with `repoke plugin list`.".to_string()
    } else {
        "Fill in its commands, then check it with `repoke plugin list` and `repoke check --only <id>`.".to_string()
    };
    println!("{}", style(next).dim());
    Ok(ExitCode::SUCCESS)
}

fn write(path: &Path, text: &str, executable: bool) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

fn supported(os: &[String]) -> bool {
    os.is_empty() || os.iter().any(|o| o == env::consts::OS)
}
