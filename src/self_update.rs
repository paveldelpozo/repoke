//! `repoke self-update`, and the once-a-day notice that a newer repoke is out. Releases come
//! from GitHub; an install made by Homebrew or cargo is left to that manager.

use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use console::style;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manager::capture;
use crate::schedule::state_dir;
use crate::snapshot::{self, now};
use crate::{EXIT_UPDATES, SelfUpdateArgs, print_json};

const REPO: &str = "paveldelpozo/repoke";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const TARGET: &str = env!("REPOKE_TARGET");
const DAY: u64 = 86_400;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Copy, Serialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
enum Method {
    Brew,
    Cargo,
    Binary,
}

#[derive(Serialize)]
struct Report<'a> {
    current: &'a str,
    latest: &'a str,
    newer: bool,
    method: Method,
}

pub fn run(args: &SelfUpdateArgs) -> Result<ExitCode, String> {
    let exe = env::current_exe().map_err(|e| format!("cannot find repoke's own path: {e}"))?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    let method = method(&exe);
    let release = latest(10)?;
    let latest = release.tag_name.trim_start_matches('v').to_string();
    let newer = is_newer(&latest, VERSION);

    if args.json {
        print_json(&Report {
            current: VERSION,
            latest: &latest,
            newer,
            method,
        });
        if args.check || !newer {
            return Ok(exit(newer));
        }
    }
    if !newer {
        if !args.json {
            println!("repoke {VERSION} is the latest version.");
        }
        return Ok(ExitCode::SUCCESS);
    }
    if args.check {
        println!("repoke {latest} is available (you have {VERSION}).");
        return Ok(ExitCode::from(EXIT_UPDATES));
    }

    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    let delegated: Option<&[&str]> = match method {
        Method::Brew => Some(&["brew", "upgrade", "repoke"]),
        Method::Cargo => Some(&["cargo", "install", "repoke", "--locked"]),
        Method::Binary => None,
    };
    let action = match delegated {
        Some(command) => format!("run `{}`", command.join(" ")),
        None => format!(
            "download repoke {latest} for {TARGET} and replace {}",
            exe.display()
        ),
    };
    if !args.yes {
        if !interactive {
            return Err(format!("not running in a terminal, pass --yes to {action}"));
        }
        let go = inquire::Confirm::new(&format!(
            "Update repoke {VERSION} → {latest}? This will {action}."
        ))
        .with_default(true)
        .prompt()
        .unwrap_or(false);
        if !go {
            println!("Nothing changed.");
            return Ok(ExitCode::SUCCESS);
        }
    }

    match delegated {
        // It was installed by that manager, which has to stay the one that knows its version.
        Some(command) => {
            let status = Command::new(command[0])
                .args(&command[1..])
                .status()
                .map_err(|e| format!("could not run {}: {e}", command[0]))?;
            if !status.success() {
                return Err(format!("`{}` failed", command.join(" ")));
            }
        }
        None => replace_binary(&release, &latest, &exe)?,
    }
    println!("Updated repoke {VERSION} → {latest}.");
    Ok(ExitCode::SUCCESS)
}

fn exit(newer: bool) -> ExitCode {
    if newer {
        ExitCode::from(EXIT_UPDATES)
    } else {
        ExitCode::SUCCESS
    }
}

fn method(exe: &Path) -> Method {
    let text = exe.to_string_lossy();
    if text.contains("/Cellar/") || text.contains("/linuxbrew/") {
        return Method::Brew;
    }
    let cargo_bin = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| snapshot::home().map(|h| h.join(".cargo")))
        .map(|dir| dir.join("bin"));
    if cargo_bin.is_some_and(|bin| exe.parent() == Some(bin.as_path())) {
        Method::Cargo
    } else {
        Method::Binary
    }
}

/// `curl` ships with macOS, Windows 10+ and nearly every Linux, so no HTTP client is bundled.
fn latest(timeout_seconds: u32) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let timeout = timeout_seconds.to_string();
    let mut args = vec![
        "-fsSL".to_string(),
        "--max-time".into(),
        timeout,
        "-H".into(),
        "Accept: application/vnd.github+json".into(),
    ];
    // While the repository is private, a token lets its owner try this out.
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        args.extend(["-H".into(), format!("Authorization: Bearer {token}")]);
    }
    args.push(url);
    let out = capture("curl", &args)?;
    // curl's exit code for an HTTP error depends on its version (22, 56…); the status is stable.
    if out.code != Some(0) && out.stderr.contains("404") {
        return Err("no release is published yet, or the repository isn't public".into());
    }
    let json = out.expect(&[0])?;
    serde_json::from_str(&json).map_err(|e| format!("unexpected answer from GitHub: {e}"))
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let parts =
        |v: &str| -> Vec<u64> { v.split(['.', '-']).map_while(|p| p.parse().ok()).collect() };
    parts(candidate) > parts(current)
}

fn replace_binary(release: &Release, latest: &str, exe: &Path) -> Result<(), String> {
    let archive = if cfg!(windows) {
        format!("repoke-{TARGET}.zip")
    } else {
        format!("repoke-{TARGET}.tar.gz")
    };
    let find = |name: &str| {
        release
            .assets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| format!("release {latest} has no {name}"))
    };
    let (asset, checksum) = (find(&archive)?, find(&format!("{archive}.sha256"))?);

    let work = env::temp_dir().join(format!("repoke-update-{}", std::process::id()));
    fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let result = (|| {
        let downloaded = work.join(&archive);
        download(&asset.browser_download_url, &downloaded)?;
        let expected = capture("curl", ["-fsSL", &checksum.browser_download_url])?.expect(&[0])?;
        let expected = expected
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_lowercase();
        let bytes = fs::read(&downloaded).map_err(|e| e.to_string())?;
        if sha256(&bytes) != expected {
            return Err("the download doesn't match its checksum, nothing was changed".to_string());
        }
        // bsdtar on Windows also unpacks zip files.
        capture(
            "tar",
            [
                "-xf".as_ref(),
                downloaded.as_os_str(),
                "-C".as_ref(),
                work.as_os_str(),
            ],
        )?
        .expect(&[0])?;
        let new = work.join(if cfg!(windows) {
            "repoke.exe"
        } else {
            "repoke"
        });
        swap(exe, &new)
    })();
    let _ = fs::remove_dir_all(&work);
    result
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    capture(
        "curl",
        [
            "-fsSL".as_ref(),
            "-o".as_ref(),
            to.as_os_str(),
            url.as_ref(),
        ],
    )?
    .expect(&[0])
    .map(drop)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A running binary can be renamed on every OS but not overwritten on Windows, so the old one is
/// moved aside, the new one moved in, and the old one put back if that fails.
fn swap(exe: &Path, new: &Path) -> Result<(), String> {
    let dir = exe.parent().ok_or("repoke's folder is unknown")?;
    let staged = dir.join(".repoke-new");
    let old = exe.with_extension("old");
    let _ = fs::remove_file(&old);
    fs::copy(new, &staged).map_err(|e| format!("cannot write to {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    fs::rename(exe, &old).map_err(|e| format!("cannot move {}: {e}", exe.display()))?;
    if let Err(e) = fs::rename(&staged, exe) {
        let _ = fs::rename(&old, exe);
        return Err(format!("cannot put the new repoke in place: {e}"));
    }
    // On Windows the old file is still in use; the next update removes it.
    let _ = fs::remove_file(&old);
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Checked {
    at: u64,
    latest: String,
}

/// Prints a one-line notice when a newer repoke is out, asking GitHub at most once a day.
pub fn notice() {
    if env::var_os("REPOKE_NO_UPDATE_CHECK").is_some() {
        return;
    }
    let Ok(dir) = state_dir() else {
        return;
    };
    let file = dir.join("update-check.json");
    let cached = fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str::<Checked>(&text).ok())
        .filter(|c| now().saturating_sub(c.at) < DAY);
    let latest = match cached {
        Some(checked) => checked.latest,
        None => {
            // A failed check is remembered too, so an offline machine doesn't retry every run.
            let latest = latest(2)
                .map(|r| r.tag_name.trim_start_matches('v').to_string())
                .unwrap_or_else(|_| VERSION.to_string());
            let checked = Checked {
                at: now(),
                latest: latest.clone(),
            };
            let _ = fs::create_dir_all(&dir);
            let _ = fs::write(
                &file,
                serde_json::to_string(&checked).expect("serializable check"),
            );
            latest
        }
    };
    if is_newer(&latest, VERSION) {
        eprintln!(
            "\n{}",
            style(format!(
                "repoke {latest} is available (you have {VERSION}): run `repoke self-update`."
            ))
            .yellow()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("1.0.0", "0.99.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn hashes_like_sha256sum() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn knows_how_it_was_installed() {
        assert_eq!(
            method(Path::new("/opt/homebrew/Cellar/repoke/0.1.0/bin/repoke")),
            Method::Brew
        );
        assert_eq!(method(Path::new("/usr/local/bin/repoke")), Method::Binary);
    }
}
