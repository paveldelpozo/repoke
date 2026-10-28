use std::path::Path;

use crate::manager::{Manager, Package, Step, capture, needs_sudo};

pub struct SoftwareUpdate;

impl Manager for SoftwareUpdate {
    fn id(&self) -> &'static str {
        "softwareupdate"
    }

    fn name(&self) -> &'static str {
        "macOS Software Update"
    }

    fn binary(&self) -> &'static str {
        "softwareupdate"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn version(&self, _bin: &Path) -> Option<String> {
        let out = capture("sw_vers", ["-productVersion"]).ok()?;
        Some(out.stdout.trim().to_string()).filter(|v| !v.is_empty())
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let text = capture(bin, ["--list"])?.expect(&[0])?;
        Ok(parse_list(&text).into_iter().map(|u| u.package).collect())
    }

    fn needs_root(&self, _bin: &Path) -> bool {
        needs_sudo()
    }

    /// Updates that restart the Mac (macOS itself) stay with System Settings: installed from
    /// here they could reboot unattended, and on Apple silicon they ask for the user's password.
    fn incompatible(&self, bin: &Path, packages: &[Package]) -> Vec<(Package, String)> {
        let Ok(text) = capture(bin, ["--list"]).and_then(|o| o.expect(&[0])) else {
            return Vec::new();
        };
        let restart: Vec<Update> = parse_list(&text)
            .into_iter()
            .filter(|u| u.restart)
            .collect();
        packages
            .iter()
            .filter(|p| restart.iter().any(|u| u.package.name == p.name))
            .map(|p| {
                let reason = "needs a restart: install it from System Settings > Software Update";
                (p.clone(), reason.to_string())
            })
            .collect()
    }

    fn upgrade_steps(&self, _bin: &Path, packages: &[Package]) -> Vec<Step> {
        let labels = packages.iter().map(|p| p.name.clone());
        vec![Step::new(
            ["--install".to_string()].into_iter().chain(labels),
            packages,
        )]
    }
}

struct Update {
    package: Package,
    restart: bool,
}

/// `* Label: <label>` followed by an indented `Title: …, Version: …, Action: restart, ` line.
fn parse_list(text: &str) -> Vec<Update> {
    let mut updates: Vec<Update> = Vec::new();
    for line in text.lines() {
        if let Some(label) = line.trim_start().strip_prefix("* Label:") {
            updates.push(Update {
                package: Package {
                    name: label.trim().to_string(),
                    current: String::new(),
                    latest: String::new(),
                },
                restart: false,
            });
        } else if let Some(update) = updates.last_mut()
            && line.trim_start().starts_with("Title:")
        {
            for field in line.split(',').map(str::trim) {
                if let Some(version) = field.strip_prefix("Version:") {
                    update.package.latest = version.trim().to_string();
                } else if field == "Action: restart" {
                    update.restart = true;
                }
            }
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_updates_and_which_restart() {
        let text = "Software Update Tool\n\nFinding available software\nSoftware Update found the following new or updated software:\n* Label: Command Line Tools for Xcode-26.1\n\tTitle: Command Line Tools for Xcode, Version: 26.1, Size: 912345KiB, Recommended: YES, \n* Label: macOS Tahoe 26.1-25B78\n\tTitle: macOS Tahoe 26.1, Version: 26.1, Size: 7096144KiB, Recommended: YES, Action: restart, \n";
        let updates = parse_list(text);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].package.name, "Command Line Tools for Xcode-26.1");
        assert_eq!(updates[0].package.latest, "26.1");
        assert!(!updates[0].restart);
        assert!(updates[1].restart);
        assert!(parse_list("Software Update Tool\n\nFinding available software\n").is_empty());
    }
}
