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
    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let mut args = vec!["upgrade".to_string()];
        args.extend(greedy().map(String::from));
        args.extend(packages.iter().map(|p| p.name.clone()));
        vec![Step::new(args, packages)]
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
    fn install_steps(&self, _bin: &Path, packages: &[Installed]) -> Vec<Step> {
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
        steps.extend(Step::install_all(&["install", "--cask"], &of(Some("cask"))));
        steps
    }
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
    fn parses_empty_report() {
        assert!(
            parse_outdated(r#"{"formulae": [], "casks": []}"#)
                .unwrap()
                .is_empty()
        );
    }
}
