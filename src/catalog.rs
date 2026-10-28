use std::collections::BTreeMap;

use serde::Deserialize;

use crate::check::Found;
use crate::manager::Installed;

#[derive(Deserialize)]
struct Catalog {
    tool: Vec<Tool>,
}

#[derive(Clone, Deserialize)]
pub struct Tool {
    pub id: String,
    pub category: String,
    pub description: String,
    via: BTreeMap<String, String>,
}

pub fn tools() -> Vec<Tool> {
    toml::from_str::<Catalog>(include_str!("catalog.toml"))
        .expect("the bundled catalog parses")
        .tool
}

/// The first manager on this machine, in `repoke list` order, that has the tool, and the package
/// it installs. `found` is already in that order.
pub fn resolve(tool: &Tool, found: &[Found]) -> Option<(usize, Installed)> {
    found.iter().enumerate().find_map(|(i, f)| {
        let id = f.manager.id();
        if id == "brew"
            && let Some(cask) = tool.via.get("brew-cask")
        {
            return Some((i, Installed::new(cask).kind("cask")));
        }
        tool.via.get(id).map(|name| (i, Installed::new(name)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managers;

    #[test]
    fn catalog_parses_and_names_real_managers() {
        let ids: Vec<&str> = managers::every().iter().map(|m| m.id()).collect();
        let tools = tools();
        assert!(tools.len() > 20);
        for tool in &tools {
            assert!(!tool.via.is_empty(), "{} has no manager", tool.id);
            for manager in tool.via.keys() {
                assert!(
                    manager == "brew-cask" || ids.contains(&manager.as_str()),
                    "{} names unknown manager {manager}",
                    tool.id
                );
            }
        }
    }
}
