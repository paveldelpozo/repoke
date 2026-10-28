# repoke

Finds the package managers installed on your system, shows what is outdated and updates what you choose: everything at once, one manager or single packages. It works interactively in a terminal and unattended for scripts, cron and AI agents.

> Status: early development, supporting brew, npm and pnpm.

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

`--only`, `--exclude`, `--hold` and `--refresh` (runs `brew update` first) work with both `check` and `upgrade`. In a terminal, spinners on stderr show what each manager is doing; with `--json` or when piped there are none.

### Unattended use

`upgrade` without `--yes` refuses to run outside a terminal (exit 2) instead of waiting for input. With `--json` it prints one event per line (`check_failed`, `running`, `planned`, `upgraded`, `failed`, `skipped`, then `summary`). A manager that needs root is called with `sudo -n`, so it fails at once when there is no cached password; `--no-sudo` skips it instead.

### Configuration

`~/.config/repoke/config.toml` (`%APPDATA%\\repoke\\config.toml` on Windows, or the file in `REPOKE_CONFIG`):

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

| Id   | Manager        | OS            | check | upgrade |
|------|----------------|---------------|-------|---------|
| brew | Homebrew       | macOS, Linux  | yes   | yes     |
| npm  | npm globals    | all           | yes   | yes     |
| pnpm | pnpm globals   | all           | yes   | yes     |

Planned: fnm, nvm, pip, uv, cargo, rustup, mas, apt, dnf, pacman, winget, scoop and more. See [docs/tareas.md](docs/tareas.md).

## Development

```sh
scripts/ci.sh           # what CI checks: fmt, clippy for macOS/Linux/Windows, tests
scripts/ci.sh --quick   # same, without the Linux tests in Docker
cargo run -- check
```

The GitHub workflow is manual (`gh workflow run CI`) so pushes don't spend Actions minutes. To run the quick checks before every push, opt in to the bundled hook with `git config core.hooksPath .githooks`. Cross-target clippy needs `rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`.

Each manager lives in `src/managers/<id>.rs` and implements the `Manager` trait in `src/manager.rs`. Output parsing is a pure function with tests over real captured output.
