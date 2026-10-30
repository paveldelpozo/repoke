use std::path::Path;

use crate::manager::{Blocked, Installed, Manager, Package, Printed, Step, capture, lines};

pub struct Bun;

impl Manager for Bun {
    fn id(&self) -> &'static str {
        "bun"
    }

    fn name(&self) -> &'static str {
        "bun (global)"
    }

    fn binary(&self) -> &'static str {
        "bun"
    }

    /// Without any global package bun has no global package.json and says so; that's up to date.
    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let out = capture(bin, ["outdated", "--global"])?;
        if out.stderr.contains("missing package.json") {
            return Ok(Vec::new());
        }
        Ok(parse_outdated(&out.expect(&[0])?))
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let specs = packages.iter().map(|p| format!("{}@{}", p.name, p.latest));
        vec![Step::new(
            ["add".to_string(), "--global".to_string()]
                .into_iter()
                .chain(specs),
            packages,
        )]
    }

    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        match lines(bin, ["pm", "ls", "--global"]) {
            Ok(lines) => Ok(parse_list(&lines)),
            Err(e) if e.contains("No package.json") => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
        Step::install_all(&["add", "--global"], packages)
    }

    fn config_files(&self) -> &'static [&'static str] {
        &[".bunfig.toml"]
    }

    /// bun skips the postinstall of dependencies outside `trustedDependencies` and its own
    /// default list, and only says how many; its line carries the command that names them.
    fn blocked(&self, packages: &[Package], printed: &Printed) -> Vec<Blocked> {
        let output = format!("{}\n{}", printed.stdout, printed.stderr);
        let Some(line) = output
            .lines()
            .map(plain)
            .find(|l| l.trim_start().starts_with("Blocked ") && l.contains("postinstall"))
        else {
            return Vec::new();
        };
        packages
            .iter()
            .map(|p| Blocked {
                package: p.clone(),
                reason: "installed, but bun blocked postinstall scripts it doesn't trust, so it may not work".into(),
                hint: format!(
                    "bun says: {}; `bun pm trust <name>` runs one and trusts it from now on",
                    line.trim()
                ),
            })
            .collect()
    }
}

/// Without the colour codes bun adds.
fn plain(line: &str) -> String {
    let mut text = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            chars.by_ref().find(|c| c.is_ascii_alphabetic());
        } else {
            text.push(c);
        }
    }
    text
}

/// Rows of a table drawn with bars: `| cowsay | 1.5.0 | 1.5.0 | 1.6.0 |`, after a header row.
fn parse_outdated(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let cells: Vec<&str> = l
                .trim()
                .strip_prefix('|')?
                .split('|')
                .map(str::trim)
                .collect();
            let [name, current, _update, latest, ..] = cells[..] else {
                return None;
            };
            (name != "Package" && !name.starts_with('-')).then(|| Package {
                name: name.to_string(),
                current: current.to_string(),
                latest: latest.to_string(),
            })
        })
        .collect()
}

/// `├── cowsay@1.5.0`; a scoped name has its own `@`.
fn parse_list(lines: &[String]) -> Vec<Installed> {
    lines
        .iter()
        .filter_map(|l| {
            let spec = l.split_once("── ")?.1.trim();
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
        let text = "|---------------------------------------|
| Package   | Current | Update | Latest |
|-----------|---------|--------|--------|
| cowsay    | 1.5.0   | 1.5.0  | 1.6.0  |
|-----------|---------|--------|--------|
| is-number | 6.0.0   | 6.0.0  | 7.0.0  |
|---------------------------------------|";
        let packages = parse_outdated(text);
        let rows: Vec<String> = packages
            .iter()
            .map(|p| format!("{} {} {}", p.name, p.current, p.latest))
            .collect();
        assert_eq!(rows, ["cowsay 1.5.0 1.6.0", "is-number 6.0.0 7.0.0"]);
    }

    #[test]
    fn reports_blocked_postinstalls() {
        let package = Package {
            name: "@vue/cli".into(),
            current: "5.0.8".into(),
            latest: "5.0.9".into(),
        };
        let blocked = Printed {
            stdout: "installed @vue/cli@5.0.9\n\n\u{1b}[1m1 package\u{1b}[0m installed\nBlocked 1 postinstall. Run `bun pm -g untrusted` for details.\n".into(),
            stderr: String::new(),
        };
        let found = Bun.blocked(std::slice::from_ref(&package), &blocked);
        assert_eq!(found.len(), 1);
        assert!(found[0].hint.starts_with(
            "bun says: Blocked 1 postinstall. Run `bun pm -g untrusted` for details."
        ));
        let clean = Printed {
            stdout: "installed @vue/cli@5.0.9\n1 package installed\n".into(),
            stderr: String::new(),
        };
        assert!(Bun.blocked(&[package], &clean).is_empty());
    }

    #[test]
    fn reads_global_list() {
        let lines = [
            "/Users/me/.bun/install/global node_modules (42 installed)".to_string(),
            "├── @vue/cli@5.0.8".to_string(),
            "└── is-number@6.0.0".to_string(),
        ];
        let names: Vec<String> = parse_list(&lines).iter().map(|p| p.to_string()).collect();
        assert_eq!(names, ["@vue/cli==5.0.8", "is-number==6.0.0"]);
    }
}
