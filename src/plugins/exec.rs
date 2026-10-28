//! `repoke-<id>` executables. repoke runs them with one subcommand and reads JSON from stdout;
//! docs/plugins.md is the protocol.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use super::{Candidate, leak, supported, timeout};
use crate::manager::{Installed, Manager, Package, Step, capture_within};

/// The newest protocol this repoke speaks; a plugin's `describe` names the one it uses.
pub const PROTOCOL: u32 = 1;

#[derive(Deserialize)]
struct Describe {
    protocol: u32,
    id: String,
    name: String,
    #[serde(default)]
    os: Vec<String>,
    #[serde(default)]
    needs_root: bool,
    /// Optional subcommands it implements: `refresh`, `installed`, `install`.
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Deserialize)]
struct Detect {
    installed: bool,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Deserialize)]
struct Listed {
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    kind: Option<String>,
}

pub struct Exec {
    id: &'static str,
    name: &'static str,
    binary: &'static str,
    path: PathBuf,
    os: Vec<String>,
    needs_root: bool,
    capabilities: Vec<String>,
}

pub fn discover() -> Vec<Candidate> {
    on_path()
        .into_iter()
        .map(|(id, path)| {
            let manager = describe(&id, &path)
                .map(|exec| &*Box::leak(Box::new(exec)) as &'static dyn Manager);
            ("executable", path, manager)
        })
        .collect()
}

/// `repoke-<id>` files on the PATH; the first one found for an id wins, like any command.
fn on_path() -> Vec<(String, PathBuf)> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    let Some(path) = env::var_os("PATH") else {
        return found;
    };
    for dir in env::split_paths(&path) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<(String, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let file = entry.file_name().into_string().ok()?;
                let stem = if cfg!(windows) {
                    [".exe", ".cmd", ".bat"]
                        .iter()
                        .find_map(|ext| file.strip_suffix(ext))?
                        .to_string()
                } else {
                    file
                };
                let id = stem.strip_prefix("repoke-")?.to_string();
                let path = entry.path();
                (!id.is_empty() && executable(&path)).then_some((id, path))
            })
            .collect();
        names.sort();
        for (id, path) in names {
            if seen.insert(id.clone()) {
                found.push((id, path));
            }
        }
    }
    found
}

fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn describe(id: &str, path: &Path) -> Result<Exec, String> {
    let json = capture_within(path, ["describe"], timeout())?.expect(&[0])?;
    let describe: Describe = serde_json::from_str(&json)
        .map_err(|e| format!("`describe` didn't print the expected JSON: {e}"))?;
    if describe.protocol > PROTOCOL {
        return Err(format!(
            "speaks protocol {}, this repoke knows up to {PROTOCOL}: update repoke",
            describe.protocol
        ));
    }
    if describe.id != id {
        return Err(format!(
            "describes itself as `{}`, so the file should be named repoke-{}",
            describe.id, describe.id
        ));
    }
    Ok(Exec {
        binary: leak(format!("repoke-{id}")),
        id: leak(describe.id),
        name: leak(describe.name),
        path: path.to_path_buf(),
        os: describe.os,
        needs_root: describe.needs_root,
        capabilities: describe.capabilities,
    })
}

impl Exec {
    fn detect(&self) -> Option<Detect> {
        let json = capture_within(&self.path, ["detect"], self.timeout())
            .ok()?
            .expect(&[0])
            .ok()?;
        serde_json::from_str(&json).ok()
    }

    fn can(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

impl Manager for Exec {
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
        supported(&self.os)
    }

    /// The plugin is the program repoke runs; it is "installed" when the tool it manages is.
    fn locate(&self) -> Option<PathBuf> {
        self.detect()
            .filter(|d| d.installed)
            .map(|_| self.path.clone())
    }

    fn version(&self, _bin: &Path) -> Option<String> {
        self.detect()?.version
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let json = capture_within(bin, ["outdated"], self.timeout())?.expect(&[0])?;
        serde_json::from_str(&json)
            .map_err(|e| format!("`outdated` didn't print a JSON list of packages: {e}"))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["upgrade".to_string()].into_iter().chain(names),
            packages,
        )]
    }

    fn has_refresh(&self) -> bool {
        self.can("refresh")
    }

    fn refresh(&self, bin: &Path) -> Result<(), String> {
        if !self.can("refresh") {
            return Ok(());
        }
        capture_within(bin, ["refresh"], self.timeout())?
            .expect(&[0])
            .map(drop)
    }

    fn timeout(&self) -> Option<Duration> {
        super::timeout_of(self.id())
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        self.needs_root
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        if !self.can("installed") {
            return Err("this plugin doesn't list what is installed".into());
        }
        let json = capture_within(bin, ["installed"], self.timeout())?.expect(&[0])?;
        let listed: Vec<Listed> = serde_json::from_str(&json)
            .map_err(|e| format!("`installed` didn't print a JSON list: {e}"))?;
        Ok(listed
            .into_iter()
            .map(|l| {
                let installed = Installed::new(l.name).version(l.version.unwrap_or_default());
                match l.kind {
                    Some(kind) => installed.kind(kind),
                    None => installed,
                }
            })
            .collect())
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        if self.can("install") {
            Step::install_all(&["install"], packages)
        } else {
            Vec::new()
        }
    }
}

pub fn template(id: &str) -> String {
    format!(
        r#"#!/bin/sh
# repoke plugin `{id}`, protocol {PROTOCOL}: https://github.com/paveldelpozo/repoke/blob/main/docs/plugins.md
# repoke runs it with one subcommand, stdin closed, and reads JSON from stdout.
set -eu

case "${{1:-}}" in
  describe)
    # Add "refresh", "installed" or "install" to capabilities if you implement them.
    echo '{{"protocol":{PROTOCOL},"id":"{id}","name":"{id}","os":["macos","linux"],"needs_root":false,"capabilities":[]}}'
    ;;
  detect)
    # Whether the tool this plugin manages is installed, and its version.
    if command -v {id} >/dev/null 2>&1; then
      printf '{{"installed":true,"version":"%s"}}\n' "$({id} --version | head -n1)"
    else
      echo '{{"installed":false}}'
    fi
    ;;
  outdated)
    # A JSON list of {{"name","current","latest"}}; [] when everything is up to date.
    echo '[]'
    ;;
  upgrade)
    shift
    # Upgrade the packages named in "$@". Exit non-zero on failure; output is shown as progress.
    echo "upgrading $*"
    ;;
  *)
    echo "usage: $0 describe|detect|outdated|upgrade <package>..." >&2
    exit 2
    ;;
esac
"#
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn plugin(dir: &Path, id: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(format!("repoke-{id}"));
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        // On Linux, a child that another test forks while the file is still open for writing
        // keeps it open until it execs, and running the script meanwhile fails with ETXTBSY.
        for _ in 0..100 {
            match std::process::Command::new(&path).output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                _ => break,
            }
        }
        path
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("repoke-test-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn speaks_the_protocol() {
        let dir = scratch("exec");
        let path = plugin(
            &dir,
            "demo",
            r#"case "$1" in
  describe) echo '{"protocol":1,"id":"demo","name":"Demo","capabilities":["installed"]}' ;;
  detect) echo '{"installed":true,"version":"1.2.3"}' ;;
  outdated) echo '[{"name":"a","current":"1","latest":"2"}]' ;;
  installed) echo '[{"name":"a","version":"1"}]' ;;
esac"#,
        );
        let exec = describe("demo", &path).unwrap();
        assert_eq!(
            (exec.id(), exec.name(), exec.binary()),
            ("demo", "Demo", "repoke-demo")
        );
        assert_eq!(exec.locate(), Some(path.clone()));
        assert_eq!(exec.version(&path).as_deref(), Some("1.2.3"));
        assert_eq!(exec.outdated(&path).unwrap()[0].latest, "2");
        assert_eq!(exec.installed(&path).unwrap()[0].to_string(), "a==1");
        let steps = exec.upgrade_steps(&path, &exec.outdated(&path).unwrap());
        assert_eq!(steps[0].commands[0], ["upgrade", "a"]);
        assert!(exec.install_steps(&path, &[Installed::new("a")]).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_bad_descriptions() {
        let dir = scratch("bad");
        let newer = plugin(
            &dir,
            "new",
            r#"echo '{"protocol":9,"id":"new","name":"New"}'"#,
        );
        assert!(
            describe("new", &newer)
                .err()
                .unwrap()
                .contains("protocol 9")
        );
        let renamed = plugin(
            &dir,
            "one",
            r#"echo '{"protocol":1,"id":"two","name":"Two"}'"#,
        );
        assert!(
            describe("one", &renamed)
                .err()
                .unwrap()
                .contains("repoke-two")
        );
        let silent = plugin(&dir, "quiet", "exit 0");
        assert!(describe("quiet", &silent).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
