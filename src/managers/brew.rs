use std::path::Path;

use serde::Deserialize;

use std::slice;

use crate::manager::{Installed, Manager, Package, Step, capture, lines};

pub struct Brew;

impl Manager for Brew {
    fn id(&self) -> &'static str {
        "brew"
    }

    fn name(&self) -> &'static str {
        "Homebrew"
    }

    fn binary(&self) -> &'static str {
        "brew"
    }

    fn supported(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "linux"))
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let mut args = vec!["outdated", "--json=v2"];
        args.extend(greedy());
        let json = capture(bin, args)?.expect(&[0, 1])?;
        parse_outdated(&json)
    }

    /// Without `--greedy`, `brew upgrade` leaves a self-updating cask alone even when named.
    /// Casks that ask for a password go in a step of their own, so the prompt can be handled.
    fn upgrade_steps(&self, bin: &Path, packages: &[Package]) -> Vec<Step> {
        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        let asking = password_casks(bin, &names);
        let (password, quiet): (Vec<Package>, Vec<Package>) = packages
            .iter()
            .cloned()
            .partition(|p| asking.contains(&p.name));
        let step = |packages: &[Package]| {
            let mut args = vec!["upgrade".to_string()];
            args.extend(greedy().map(String::from));
            args.extend(packages.iter().map(|p| p.name.clone()));
            Step::new(args, packages)
        };
        let mut steps = Vec::new();
        if !quiet.is_empty() {
            steps.push(step(&quiet));
        }
        if !password.is_empty() {
            steps.push(step(&password).with_password());
        }
        steps
    }

    fn hint(&self, packages: &[Package], error: &str) -> Option<String> {
        // The cask's app was deleted or moved by hand, so brew has nothing to replace.
        if !(error.contains("App source") && error.contains("is not there")) {
            return None;
        }
        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        let names = names.join(" ");
        Some(format!(
            "the app was removed outside Homebrew: bring it back with `brew reinstall --cask {names}`, or forget it with `brew uninstall --cask --force {names}`"
        ))
    }

    fn refresh(&self, bin: &Path) -> Result<(), String> {
        capture(bin, ["update"])?.expect(&[0]).map(drop)
    }

    /// Official `homebrew/*` taps come with Homebrew and are left out.
    fn installed(&self, bin: &Path) -> Result<Vec<Installed>, String> {
        let mut packages: Vec<Installed> = lines(bin, ["tap"])?
            .into_iter()
            .filter(|tap| !tap.starts_with("homebrew/"))
            .map(|tap| Installed::new(tap).kind("tap"))
            .collect();
        packages.extend(
            lines(bin, ["leaves", "--installed-on-request"])?
                .into_iter()
                .map(Installed::new),
        );
        packages.extend(
            lines(bin, ["list", "--cask", "-1"])?
                .into_iter()
                .map(|cask| Installed::new(cask).kind("cask")),
        );
        Ok(packages)
    }

    /// Taps first, so the formulae and casks that come from them resolve.
    fn install_steps(&self, bin: &Path, packages: &[Installed]) -> Vec<Step> {
        let of = |kind: Option<&str>| -> Vec<Installed> {
            packages
                .iter()
                .filter(|p| p.kind.as_deref() == kind)
                .cloned()
                .collect()
        };
        let mut steps: Vec<Step> = of(Some("tap"))
            .iter()
            .map(|tap| Step::install(["tap", &tap.name], slice::from_ref(tap)))
            .collect();
        steps.extend(Step::install_all(&["install"], &of(None)));
        let casks = of(Some("cask"));
        let names: Vec<&str> = casks.iter().map(|c| c.name.as_str()).collect();
        let asking = password_casks(bin, &names);
        let (password, quiet): (Vec<Installed>, Vec<Installed>) =
            casks.into_iter().partition(|c| asking.contains(&c.name));
        steps.extend(Step::install_all(&["install", "--cask"], &quiet));
        steps.extend(
            Step::install_all(&["install", "--cask"], &password)
                .into_iter()
                .map(Step::with_password),
        );
        steps
    }
}

/// The casks among `names` that run `sudo` themselves. When brew can't tell, none are, and a
/// prompt shows up as it did before.
fn password_casks(bin: &Path, names: &[&str]) -> Vec<String> {
    if names.is_empty() {
        return Vec::new();
    }
    let mut args = vec!["info", "--json=v2"];
    args.extend(names);
    capture(bin, args)
        .and_then(|out| out.expect(&[0]))
        .and_then(|json| parse_password_casks(&json))
        .unwrap_or_default()
}

/// A `.pkg` installs with `sudo installer`, and removing what one left behind (receipts, system
/// services, kernel extensions, uninstall scripts) also runs as root during an upgrade.
fn parse_password_casks(json: &str) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Info {
        #[serde(default)]
        casks: Vec<Cask>,
    }
    #[derive(Deserialize)]
    struct Cask {
        token: String,
        artifacts: Vec<serde_json::Map<String, serde_json::Value>>,
    }
    let info: Info =
        serde_json::from_str(json).map_err(|e| format!("unexpected brew output: {e}"))?;
    let as_root = |artifact: &serde_json::Map<String, serde_json::Value>| {
        artifact.keys().any(|k| k == "pkg" || k == "installer")
            || artifact
                .get("uninstall")
                .and_then(|u| u.as_array())
                .is_some_and(|directives| {
                    directives.iter().any(|d| {
                        ["pkgutil", "launchctl", "kext", "script", "early_script"]
                            .iter()
                            .any(|k| d.get(k).is_some())
                    })
                })
    };
    Ok(info
        .casks
        .into_iter()
        .filter(|c| c.artifacts.iter().any(as_root))
        .map(|c| c.token)
        .collect())
}

fn greedy() -> Option<&'static str> {
    crate::config::current().brew.greedy.then_some("--greedy")
}

#[derive(Deserialize)]
struct Report {
    formulae: Vec<Item>,
    casks: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    name: String,
    installed_versions: Vec<String>,
    current_version: String,
}

fn parse_outdated(json: &str) -> Result<Vec<Package>, String> {
    let report: Report =
        serde_json::from_str(json).map_err(|e| format!("unexpected brew output: {e}"))?;
    Ok(report
        .formulae
        .into_iter()
        .chain(report.casks)
        .map(|item| Package {
            name: item.name,
            current: item.installed_versions.last().cloned().unwrap_or_default(),
            latest: item.current_version,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formulae_and_casks() {
        let json = r#"{
            "formulae": [{"name": "git", "installed_versions": ["2.50.0", "2.51.0"], "current_version": "2.52.0", "pinned": false}],
            "casks": [{"name": "firefox", "installed_versions": ["140.0"], "current_version": "141.0"}]
        }"#;
        let packages = parse_outdated(json).unwrap();
        assert_eq!(
            packages,
            vec![
                Package {
                    name: "git".into(),
                    current: "2.51.0".into(),
                    latest: "2.52.0".into()
                },
                Package {
                    name: "firefox".into(),
                    current: "140.0".into(),
                    latest: "141.0".into()
                },
            ]
        );
    }

    #[test]
    fn explains_casks_whose_app_is_gone() {
        let casks = [Package {
            name: "caskhub".into(),
            current: "0.9.0".into(),
            latest: "0.9.3".into(),
        }];
        let error =
            "Error: caskhub: It seems the App source '/Applications/CaskHub.app' is not there.";
        assert!(
            Brew.hint(&casks, error)
                .unwrap()
                .contains("brew reinstall --cask caskhub")
        );
        assert!(Brew.hint(&casks, "Error: network down").is_none());
    }

    #[test]
    fn finds_casks_that_ask_for_a_password() {
        let json = r#"{"formulae": [{"name": "git"}], "casks": [
            {"token": "zoom", "artifacts": [{"uninstall": [{"launchctl": ["us.zoom.updater"], "pkgutil": "us.zoom.pkg"}]}, {"pkg": ["zoom.pkg"]}]},
            {"token": "vorssaint", "artifacts": [{"uninstall": [{"quit": "com.vorssaint.utils"}]}, {"app": ["Vorssaint.app"]}, {"zap": [{"launchctl": ["x"]}]}]},
            {"token": "karabiner", "artifacts": [{"uninstall": [{"script": {"executable": "uninstall.sh", "sudo": true}}]}]}
        ]}"#;
        assert_eq!(parse_password_casks(json).unwrap(), ["zoom", "karabiner"]);
    }

    #[test]
    fn parses_empty_report() {
        assert!(
            parse_outdated(r#"{"formulae": [], "casks": []}"#)
                .unwrap()
                .is_empty()
        );
    }
}
