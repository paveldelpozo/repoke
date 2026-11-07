<p align="center">
  <a href="https://repoke.paveldelpozo.es">
    <img src="docs/assets/repoke-hero.jpg" alt="repoke — Your package managers. One command. Run repoke check to see what needs updating." width="100%">
  </a>
</p>

<h1 align="center"><img src="site/public/logo.svg" width="48" alt=""> repoke</h1>

<p align="center"><strong>Find outdated packages across your machine. Choose what gets upgraded.</strong></p>

<p align="center">
  <a href="https://github.com/paveldelpozo/repoke/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/paveldelpozo/repoke/ci.yml?style=flat-square&amp;label=CI" alt="CI: formatting, Clippy and tests"></a>
  <a href="https://github.com/paveldelpozo/repoke/releases"><img src="https://img.shields.io/github/v/release/paveldelpozo/repoke?style=flat-square&amp;color=B3121F" alt="Latest release"></a>
  <a href="https://crates.io/crates/repoke"><img src="https://img.shields.io/crates/v/repoke?style=flat-square&amp;color=B3121F" alt="crates.io version"></a>
  <a href="https://github.com/paveldelpozo/repoke/releases"><img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-17140F?style=flat-square" alt="Platforms: macOS, Linux, Windows"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-17140F?style=flat-square" alt="License: MIT OR Apache-2.0"></a>
</p>

<p align="center">
  <a href="#quick-start">Quick start</a> ·
  <a href="#supported-managers">Supported managers</a> ·
  <a href="#with-ai-agents">AI agents</a> ·
  <a href="https://repoke.paveldelpozo.es/commands.html">Docs</a> ·
  <a href="https://repoke.paveldelpozo.es/es/">Español</a>
</p>

Homebrew for apps. npm for globals. pip for Python. cargo for Rust. Keeping a development machine current means remembering a different command for each.

**repoke brings 26 built-in managers into one CLI.** It detects the ones you have, checks for updates and runs their native commands for the packages you select. Use it interactively, on a schedule or through an AI agent with MCP.

> [!NOTE]
> repoke is in early development. Platform support and real-system verification vary by manager; see [the compatibility table](#supported-managers).

## Quick start

Install with **Homebrew** on macOS or Linux:

```sh
brew tap paveldelpozo/tap
brew trust paveldelpozo/tap
brew install repoke
```

Or with **Cargo** (Rust 1.89+):

```sh
cargo install repoke
```

Prefer a binary? [Download a release for macOS, Linux or Windows](https://github.com/paveldelpozo/repoke/releases) and put `repoke` on your `PATH`.

Start with a read-only check, then preview or choose your updates:

```sh
repoke check                 # see what's outdated; no packages changed
repoke upgrade --dry-run     # preview the commands
repoke upgrade               # choose from a list; all packages start selected
```

`check` exits with **100 when updates are available**, 0 when everything is current and 1 when a manager fails. Update repoke itself with `repoke self-update`.

## One place for the work around your packages

| What you want to do | How repoke helps |
| --- | --- |
| See what's installed | `repoke list` detects supported managers on your machine. |
| Choose your updates | Filter by manager, select packages or hold a package back. Preview with `--dry-run`. |
| Set up another machine | Export explicitly installed packages and optional config; restore what's missing. |
| Stop checking manually | Schedule checks or upgrades with your OS scheduler. Get notified when updates are available, applied or fail. |
| Let an agent help | Built-in MCP server, JSON output and unattended operation. |
| Add another manager | Declare commands in TOML or write a small executable plugin. |

### Keep control of each upgrade

```sh
repoke check --only brew,pnpm           # focus on specific managers
repoke upgrade --hold eslint           # leave a package alone
repoke upgrade --package npm:typescript # select one package
repoke upgrade --yes --no-sudo --json   # unattended; skip managers needing sudo
```

> [!IMPORTANT]
> **pacman always performs a full system upgrade (`-Syu`).** Package selections and holds cannot narrow it. Use `--exclude pacman` to leave it out.

### Take your setup with you

```sh
repoke export -o my-setup              # save packages; asks about config files
repoke init --from my-setup --dry-run  # preview installs and config restoration
repoke init --from my-setup            # choose what to restore
```

Config export detects values that look like secrets and offers redaction, omission or password encryption with `--encrypt`. Unattended, unencrypted exports redact detected values. Review snapshots before sharing them. Existing, different config files are kept unless you choose to replace them, with a backup made before replacement.

Starting fresh? `repoke init` opens a curated tool catalog. [Snapshot and restore options →](docs/commands.md#repoke-export)

### Check while you're away

```sh
repoke schedule install --check-only  # daily check at 03:00; no upgrades
repoke schedule status               # inspect the schedule and last result
repoke schedule remove               # remove the schedule
```

For automatic upgrades, use `repoke schedule install`: it runs with `--yes --no-sudo`. Scheduling uses launchd, systemd user timers or Windows Task Scheduler; without systemd, repoke prints a crontab line to add. [Scheduling options →](docs/commands.md#repoke-schedule)

## With AI agents

`repoke mcp` exposes six tools over stdio: `list_managers`, `check_updates`, `upgrade`, `export`, `init_preview` and `schedule_status`. The MCP upgrade tool **previews by default** and never uses sudo; executing an upgrade requires `dry_run: false`.

**Claude Code** — install the plugin for the MCP server and usage skill:

```text
/plugin install repoke --marketplace paveldelpozo/repoke
```

**Codex** — add to `~/.codex/config.toml`:

```toml
[mcp_servers.repoke]
command = "repoke"
args = ["mcp"]
```

For other MCP clients, configure `repoke mcp` as a stdio server. Install repoke first and ensure it is on the client's `PATH`.

Scripts can use `repoke check --json` and `repoke upgrade --yes --no-sudo --json` directly. Without a terminal, `upgrade` requires `--yes` and exits instead of waiting for input. [JSON events and exit codes →](docs/commands.md#json-output)

## Supported managers

"Verified" means checked against the real tool; the rest are built from documented output formats and covered by parser tests, and still need a run on a real system.

| Id       | Manager                     | OS            | Verified | Notes |
|----------|-----------------------------|---------------|----------|-------|
| brew     | Homebrew formulae and casks | macOS, Linux  | macOS    | `--refresh` runs `brew update` |
| softwareupdate | macOS Software Update | macOS        | macOS    | needs sudo; updates that restart the Mac are left to System Settings |
| mas      | Mac App Store               | macOS         |          | mas 7+ needs sudo to install and upgrade |
| apt      | APT                         | Linux         | Debian   | needs sudo; `--refresh` runs `apt-get update` |
| dnf      | DNF                         | Linux         | Fedora   | needs sudo |
| pacman   | pacman                      | Linux         | Arch     | needs sudo; always a full `-Syu`, Arch has no partial upgrades: a selection, `--package` or `--hold` still upgrades everything |
| zypper   | zypper                      | Linux         | openSUSE | needs sudo; `export` takes what `zypper packages --userinstalled` marks as installed by you |
| snap     | Snap                        | Linux         |          | needs sudo |
| flatpak  | Flatpak                     | Linux         |          | |
| winget   | WinGet                      | Windows       |          | |
| scoop    | Scoop                       | Windows       |          | |
| choco    | Chocolatey                  | Windows       |          | run from an elevated shell |
| npm      | npm globals                 | all           | macOS    | installs the exact version listed |
| pnpm     | pnpm globals                | all           | macOS    | updates itself with `self-update` |
| yarn     | Yarn 1 globals              | all           | macOS    | Yarn 2+ has no globals and is left out; `${VAR}`s in `.npmrc` that aren't set are passed empty, as npm treats them |
| bun      | bun globals                 | all           | macOS    | |
| fnm      | Node.js versions via fnm    | all           | macOS    | one entry per major line; moves `default` and reinstalls globals; `[node] remove_old` uninstalls the replaced releases |
| nvm      | Node.js versions via nvm    | macOS, Linux  |          | same as fnm, via `--reinstall-packages-from` |
| pip      | pip `--user` packages       | all           | macOS    | never forces an externally managed Python (PEP 668) |
| uv       | uv tools and uv itself      | all           | macOS    | |
| cargo    | crates from `cargo install` | all           | macOS    | registry crates only |
| rustup   | Rust toolchains and rustup  | all           | macOS    | |
| gem      | RubyGems                    | all           |          | skips the system Ruby on macOS |
| composer | Composer globals            | all           | macOS    | |
| mise     | mise tools                  | all           |          | |
| asdf     | asdf tools                  | macOS, Linux  | Linux    | asdf 0.16+; only the global versions in `~/.tool-versions`; old versions stay installed |

Planned: deno, volta. See [docs/tareas.md](docs/tareas.md).


## Extend repoke

```sh
repoke plugin search          # browse official and community plugins
repoke plugin install vscode  # review its code before confirming installation
repoke plugin new gh-ext      # scaffold a TOML manager
```

Plugins participate in every command, including MCP. Add your own with a TOML file or a `repoke-<id>` executable on your `PATH`.

[Plugin guide](docs/plugins.md) · [Example plugins](examples/plugins) · [Plugin registry](https://github.com/paveldelpozo/repoke-plugins)

## Go deeper

- [Command reference](docs/commands.md): every command, configuration, JSON events and exit codes.
- [Referencia en español](docs/es/commands.md): comandos, configuración y automatización.
- [Roadmap](docs/tareas.md): planned work and known gaps.
- [Report a bug or request a manager](https://github.com/paveldelpozo/repoke/issues): include your OS, manager and repoke version.

<details>
<summary><strong>Developing repoke</strong></summary>

```sh
scripts/ci.sh           # what CI checks: fmt, clippy for macOS/Linux/Windows, tests
scripts/ci.sh --quick   # same, without the Linux tests in Docker
cargo run -- check
scripts/release.sh --dry-run   # next version and every target's archive in dist/, nothing published
scripts/release.sh             # bump, tag, GitHub release, Homebrew tap (--crate: crates.io)
scripts/release.sh minor       # or major, patch, or an exact version like 1.2.0
```

Commits follow [Conventional Commits](https://www.conventionalcommits.org/) so the release script can work out the next [semver](https://semver.org/) version: `feat:` is a minor bump, `fix:` and the other types a patch, and `!` or `BREAKING CHANGE` a major one. Release notes are grouped by those types.

GitHub Actions runs formatting checks, Clippy and tests on macOS, Linux and Windows for every push and pull request. You can also start it manually with `gh workflow run CI`. New runs cancel older runs for the same branch or pull request. To run the quick checks before every push, opt in to the bundled hook with `git config core.hooksPath .githooks`. Cross-target clippy needs `rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`.

The website (https://repoke.paveldelpozo.es) is an Astro project in `site/` that renders `docs/`: `pnpm install && pnpm build` there writes it to `site/dist`. Every command and option in `repoke --help` must appear in `docs/commands.md` and `docs/es/commands.md`; `scripts/ci.sh` checks it.

Each manager lives in `src/managers/<id>.rs` and implements the `Manager` trait in `src/manager.rs`. Output parsing is a pure function with tests over real captured output.


</details>

---

**Fewer update commands to remember?** [Star repoke](https://github.com/paveldelpozo/repoke) so you can find it again, or share it with someone maintaining a development machine. Real-world reports for unverified managers are especially useful.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
