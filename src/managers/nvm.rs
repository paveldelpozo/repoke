use std::env;
use std::path::{Path, PathBuf};

use super::node_versions::{self, Version};
use crate::manager::{Installed, Manager, Package, Step, capture};

/// nvm is a shell function, so every call sources nvm.sh in a fresh bash first.
const SCRIPT: &str = r#". "$0" --no-use && nvm "$@""#;

pub struct Nvm;

impl Manager for Nvm {
    fn id(&self) -> &'static str {
        "nvm"
    }

    fn name(&self) -> &'static str {
        "nvm (Node.js)"
    }

    fn binary(&self) -> &'static str {
        "nvm"
    }

    fn supported(&self) -> bool {
        cfg!(unix)
    }

    fn locate(&self) -> Option<PathBuf> {
        let dir = env::var_os("NVM_DIR")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".nvm")))?;
        Some(dir.join("nvm.sh")).filter(|script| script.is_file())
    }

    fn invocation(&self, bin: &Path) -> Vec<String> {
        vec![
            "bash".to_string(),
            "-c".to_string(),
            SCRIPT.to_string(),
            bin.display().to_string(),
        ]
    }

    fn version(&self, bin: &Path) -> Option<String> {
        crate::manager::version_in(&run(bin, &["--version"]).ok()?)
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let (installed, _) = parse_ls(&run(bin, &["ls", "--no-colors"])?);
        let remote: Vec<Version> = run(bin, &["ls-remote", "--no-colors"])?
            .lines()
            .filter_map(first_version)
            .collect();
        Ok(node_versions::outdated(&installed, &remote))
    }

    /// `--reinstall-packages-from` carries the globals over, nvm keeps them per version.
    fn upgrade_steps(&self, bin: &Path, packages: &[Package]) -> Vec<Step> {
        let default = run(bin, &["ls", "--no-colors"])
            .ok()
            .and_then(|ls| parse_ls(&ls).1);
        packages
            .iter()
            .map(|p| {
                let mut commands = vec![vec![
                    "install".to_string(),
                    p.latest.clone(),
                    format!("--reinstall-packages-from={}", p.current),
                ]];
                if default.is_some_and(|d| Some(d[0]) == node_versions::major(p)) {
                    commands.push(vec![
                        "alias".to_string(),
                        "default".to_string(),
                        p.latest.clone(),
                    ]);
                }
                Step {
                    commands,
                    packages: vec![p.clone()],
                }
            })
            .collect()
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let (installed, default) = parse_ls(&run(bin, &["ls", "--no-colors"])?);
        Ok(node_versions::lines(&installed, default))
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        packages
            .iter()
            .filter_map(|p| {
                let major = p.name.strip_prefix("node@")?.to_string();
                let mut commands = vec![vec!["install".to_string(), major.clone()]];
                if p.is("default") {
                    commands.push(vec!["alias".to_string(), "default".to_string(), major]);
                }
                Some(Step {
                    commands,
                    packages: vec![Package::from(p)],
                })
            })
            .collect()
    }
}

fn run(bin: &Path, args: &[&str]) -> Result<String, String> {
    let script = bin.display().to_string();
    capture(
        "bash",
        ["-c", SCRIPT, &script]
            .into_iter()
            .chain(args.iter().copied()),
    )?
    .expect(&[0])
}

/// Version lines are `       v20.6.1 *` or `->     v22.13.1 *` for the active one.
fn first_version(line: &str) -> Option<Version> {
    let line = line.trim();
    let line = line.strip_prefix("->").unwrap_or(line);
    line.split_whitespace()
        .next()
        .and_then(node_versions::parse)
}

/// The default is the alias line `default -> 22 (-> v22.13.1 *)`.
fn parse_ls(text: &str) -> (Vec<Version>, Option<Version>) {
    let installed = text.lines().filter_map(first_version).collect();
    let default = text
        .lines()
        .find(|l| l.trim_start().starts_with("default ->"))
        .and_then(|l| l.split("(-> ").nth(1))
        .and_then(|rest| rest.split([' ', ')']).next())
        .and_then(node_versions::parse);
    (installed, default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ls_output() {
        let ls = "       v20.6.1 *\n->     v22.13.1 *\ndefault -> 22 (-> v22.13.1 *)\niojs -> N/A (default)\nlts/iron -> v20.19.5 (-> N/A)\n";
        let (installed, default) = parse_ls(ls);
        assert_eq!(installed, [[20, 6, 1], [22, 13, 1]]);
        assert_eq!(default, Some([22, 13, 1]));
    }

    #[test]
    fn parses_ls_remote_lines() {
        assert_eq!(
            first_version("        v22.22.2   (Latest LTS: Jod)"),
            Some([22, 22, 2])
        );
        assert_eq!(first_version("->     v24.15.0"), Some([24, 15, 0]));
    }
}
