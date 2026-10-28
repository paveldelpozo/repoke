use std::path::Path;

use crate::manager::{Manager, Package, capture};

pub struct Pnpm;

impl Manager for Pnpm {
    fn id(&self) -> &'static str {
        "pnpm"
    }

    fn name(&self) -> &'static str {
        "pnpm (global)"
    }

    fn binary(&self) -> &'static str {
        "pnpm"
    }

    fn outdated(&self, bin: &Path) -> Result<Vec<Package>, String> {
        let json = capture(bin, &["outdated", "--global", "--format", "json"])?.expect(&[0, 1])?;
        super::npm::parse_outdated(&json)
    }
}
