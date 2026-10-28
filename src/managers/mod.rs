mod brew;
mod npm;
mod pnpm;

use crate::manager::Manager;

pub fn all() -> Vec<&'static dyn Manager> {
    let managers: [&'static dyn Manager; 3] = [&brew::Brew, &npm::Npm, &pnpm::Pnpm];
    managers.into_iter().filter(|m| m.supported()).collect()
}
