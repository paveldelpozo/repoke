use std::path::Path;

use crate::manager::{Manager, Package, Step, capture};

pub struct Mas;

impl Manager for Mas {
    fn id(&self) -> &'static str {
        "mas"
    }

    fn name(&self) -> &'static str {
        "Mac App Store"
    }

    fn binary(&self) -> &'static str {
        "mas"
    }

    fn supported(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        Ok(parse_outdated(&capture(bin, ["outdated"])?.expect(&[0])?)
            .into_iter()
            .map(|(_, package)| package)
            .collect())
    }

    /// Apps are listed by name but upgraded by App Store id, so the ids are looked up again.
    fn upgrade_steps(&self, bin: &Path, packages: &[Package]) -> Vec<Step> {
        let listed = capture(bin, ["outdated"])
            .and_then(|o| o.expect(&[0]))
            .map(|text| parse_outdated(&text))
            .unwrap_or_default();
        let ids = packages.iter().filter_map(|p| {
            listed
                .iter()
                .find(|(_, app)| app.name == p.name)
                .map(|(id, _)| id.clone())
        });
        vec![Step::new(
            ["upgrade".to_string()].into_iter().chain(ids),
            packages,
        )]
    }
}

/// `497799835 Xcode (15.0 -> 15.1)`; names can contain spaces and parentheses.
fn parse_outdated(text: &str) -> Vec<(String, Package)> {
    text.lines()
        .filter_map(|line| {
            let (id, rest) = line.trim().split_once(char::is_whitespace)?;
            if !id.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let open = rest.rfind('(')?;
            let (current, latest) = rest[open + 1..].trim_end_matches(')').split_once("->")?;
            Some((
                id.to_string(),
                Package {
                    name: rest[..open].trim().to_string(),
                    current: current.trim().to_string(),
                    latest: latest.trim().to_string(),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_outdated_apps() {
        let text = "497799835 Xcode (15.0 -> 15.1)\n1295203466  Microsoft Remote Desktop (Beta)  (10.9.4 -> 10.9.5)\n";
        let apps = parse_outdated(text);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].0, "497799835");
        assert_eq!(apps[1].1.name, "Microsoft Remote Desktop (Beta)");
        assert_eq!(apps[1].1.latest, "10.9.5");
    }
}
