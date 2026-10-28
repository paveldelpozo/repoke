# repoke

Finds the package managers installed on your system, shows what is outdated and updates what you choose: everything at once, one manager or single packages. It works interactively in a terminal and unattended for scripts, cron and AI agents.

> Status: early development.

## Usage

```sh
repoke list                       # supported managers and whether they are installed
repoke check                      # pending updates, read only
repoke check --only brew,pnpm     # restrict to some managers
repoke check --exclude npm --json # machine-readable output

repoke upgrade                    # pick what to upgrade from a list (all preselected)
repoke upgrade --dry-run          # print the commands, change nothing
repoke upgrade --yes              # upgrade everything pending without asking
repoke upgrade --yes --json --no-sudo --hold eslint
```

### Moving to another machine

```sh
repoke export                         # asks, per manager, whether to include its config files
repoke export --yes --config -o snap  # unattended: config included, secret values removed
repoke init --from snap               # pick what to install (all preselected), then restore config
repoke init --from snap --dry-run     # what would be installed and restored
repoke init                           # fresh machine: pick tools from a catalog (ripgrep, fd, gh…)
repoke init --tools ripgrep,fd --yes  # the same without the picker
```

`export` writes a folder with `repoke.toml`, one line per package (`cask:firefox`, `eslint==10.13.0`), and `files/` with the config files at their path under your home. It records only what you installed on purpose, not dependencies. Lines that look like secrets (`_authToken=…`, `password: …`, private keys) are found before saving: interactively you choose to remove their values, leave the file out or keep it; unattended, the values are always removed. `${VAR}` references are kept. A config that is a symlink into a git repository is recorded as coming from your dotfiles (with the repository's remote) instead of being copied.

`init --from` skips managers the machine doesn't have for its OS, installs only what is missing, runs Homebrew first (it often installs the other managers) and offers to install a still-missing manager with Homebrew. Config files are compared first: missing ones are restored, equal ones left alone, and different ones are kept unless you choose the snapshot's copy (or pass `--overwrite`), with yours backed up as `<file>.repoke-<date>.bak`. Removed secret values are filled back from the file already on the machine, so restoring never swaps a real token for the placeholder.

`--only`, `--exclude`, `--hold` and `--refresh` (runs `brew update` first) work with both `check` and `upgrade`. In a terminal, spinners on stderr show what each manager is doing; with `--json` or when piped there are none.

### Unattended use

`upgrade` without `--yes` refuses to run outside a terminal (exit 2) instead of waiting for input. With `--json` it prints one event per line (`check_failed`, `running`, `planned`, `done`, `failed`, `skipped`, `retrying`, then `summary`). `done` and `failed` carry `action` (`upgrade` or `install`) and `seconds`; `summary` has the totals, `elapsed_seconds` for the whole run and a per-manager breakdown. `init` adds a `config` event per config file. A manager that needs root is called with `sudo -n`, so it fails at once when there is no cached password; `--no-sudo` skips it instead.

### Configuration

`~/.config/repoke/config.toml` (`%APPDATA%\repoke\config.toml` on Windows, or the file in `REPOKE_CONFIG`):

```toml
exclude = ["brew"]               # managers left out unless named in --only
hold = ["eslint", "pnpm:pnpm"]   # never upgraded nor reported, as name or manager:name
```

### Exit codes

| Code | Meaning |
|------|---------|
| 0    | `check`: everything is up to date. `upgrade`: everything selected was upgraded |
| 100  | `check`: updates are available |
| 1    | A manager failed to check or upgrade (details in the output) |
| 2    | Invalid arguments or config, or `upgrade` without `--yes` outside a terminal |
| 130  | The interactive selection was cancelled |

repoke never waits for input when stdin is not a terminal: a manager that tries to prompt fails instead of hanging.

## Supported managers

"Verified" means checked against the real tool; the rest are built from documented output formats and covered by parser tests, and still need a run on a real system.

| Id       | Manager                     | OS            | Verified | Notes |
|----------|-----------------------------|---------------|----------|-------|
| brew     | Homebrew formulae and casks | macOS, Linux  | macOS    | `--refresh` runs `brew update` |
| mas      | Mac App Store               | macOS         |          | |
| apt      | APT                         | Linux         |          | needs sudo; `--refresh` runs `apt-get update` |
| dnf      | DNF                         | Linux         |          | needs sudo |
| pacman   | pacman                      | Linux         |          | needs sudo; always a full `-Syu`, Arch has no partial upgrades |
| zypper   | zypper                      | Linux         |          | needs sudo |
| snap     | Snap                        | Linux         |          | needs sudo |
| flatpak  | Flatpak                     | Linux         |          | |
| winget   | WinGet                      | Windows       |          | |
| scoop    | Scoop                       | Windows       |          | |
| choco    | Chocolatey                  | Windows       |          | run from an elevated shell |
| npm      | npm globals                 | all           | macOS    | installs the exact version listed |
| pnpm     | pnpm globals                | all           | macOS    | updates itself with `self-update` |
| fnm      | Node.js versions via fnm    | all           | macOS    | one entry per major line; moves `default` and reinstalls globals |
| nvm      | Node.js versions via nvm    | macOS, Linux  |          | same as fnm, via `--reinstall-packages-from` |
| pip      | pip `--user` packages       | all           | macOS    | never forces an externally managed Python (PEP 668) |
| uv       | uv tools and uv itself      | all           | macOS    | |
| cargo    | crates from `cargo install` | all           | macOS    | registry crates only |
| rustup   | Rust toolchains and rustup  | all           | macOS    | |
| gem      | RubyGems                    | all           |          | skips the system Ruby on macOS |
| composer | Composer globals            | all           | macOS    | |
| mise     | mise tools                  | all           |          | |

Planned: yarn, bun, deno, volta, asdf, `softwareupdate` (opt in). See [docs/tareas.md](docs/tareas.md).

## Development

```sh
scripts/ci.sh           # what CI checks: fmt, clippy for macOS/Linux/Windows, tests
scripts/ci.sh --quick   # same, without the Linux tests in Docker
cargo run -- check
```

The GitHub workflow is manual (`gh workflow run CI`) so pushes don't spend Actions minutes. To run the quick checks before every push, opt in to the bundled hook with `git config core.hooksPath .githooks`. Cross-target clippy needs `rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`.

Each manager lives in `src/managers/<id>.rs` and implements the `Manager` trait in `src/manager.rs`. Output parsing is a pure function with tests over real captured output.
