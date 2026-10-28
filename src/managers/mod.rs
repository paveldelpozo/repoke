mod apt;
mod brew;
mod cargo;
mod choco;
mod composer;
mod dnf;
mod flatpak;
mod fnm;
mod gem;
mod mas;
mod mise;
mod node_versions;
mod npm;
mod nvm;
mod pacman;
mod pip;
mod pnpm;
mod rustup;
mod scoop;
mod snap;
mod table;
mod uv;
mod winget;
mod zypper;

use crate::manager::Manager;

/// System package managers first, then language and tool managers.
pub fn all() -> Vec<&'static dyn Manager> {
    let managers: [&'static dyn Manager; 22] = [
        &brew::Brew,
        &mas::Mas,
        &apt::Apt,
        &dnf::Dnf,
        &pacman::Pacman,
        &zypper::Zypper,
        &snap::Snap,
        &flatpak::Flatpak,
        &winget::Winget,
        &scoop::Scoop,
        &choco::Choco,
        &npm::Npm,
        &pnpm::Pnpm,
        &fnm::Fnm,
        &nvm::Nvm,
        &pip::Pip,
        &uv::Uv,
        &cargo::Cargo,
        &rustup::Rustup,
        &gem::Gem,
        &composer::Composer,
        &mise::Mise,
    ];
    managers.into_iter().filter(|m| m.supported()).collect()
}
