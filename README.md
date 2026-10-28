# repoke

Finds the package managers installed on your system, shows what is outdated and updates what you choose: everything at once, one manager or single packages. It works interactively in a terminal and unattended for scripts, cron and AI agents.

> Status: early development. Only `list` and `check` exist; nothing is upgraded yet.

## Usage

```sh
repoke list                       # supported managers and whether they are installed
repoke check                      # pending updates, read only
repoke check --only brew,pnpm     # restrict to some managers
repoke check --exclude npm --json # machine-readable output
```

### Exit codes

| Code | Meaning |
|------|---------|
| 0    | Everything checked is up to date |
| 100  | Updates are available |
| 1    | At least one manager failed to check (details in the output) |
| 2    | Invalid arguments |

repoke never waits for input when stdin is not a terminal: a manager that tries to prompt fails instead of hanging.

## Supported managers

| Id   | Manager        | OS            | check | upgrade |
|------|----------------|---------------|-------|---------|
| brew | Homebrew       | macOS, Linux  | yes   | planned |
| npm  | npm globals    | all           | yes   | planned |
| pnpm | pnpm globals   | all           | yes   | planned |

Planned: fnm, nvm, pip, uv, cargo, rustup, mas, apt, dnf, pacman, winget, scoop and more. See [docs/tareas.md](docs/tareas.md).

## Development

```sh
scripts/ci.sh           # what CI checks: fmt, clippy for macOS/Linux/Windows, tests
scripts/ci.sh --quick   # same, without the Linux tests in Docker
cargo run -- check
```

The GitHub workflow is manual (`gh workflow run CI`) so pushes don't spend Actions minutes. To run the quick checks before every push, opt in to the bundled hook with `git config core.hooksPath .githooks`. Cross-target clippy needs `rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-msvc`.

Each manager lives in `src/managers/<id>.rs` and implements the `Manager` trait in `src/manager.rs`. Output parsing is a pure function with tests over real captured output.
