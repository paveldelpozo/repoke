---
name: repoke
description: Check and upgrade the software installed through package managers on this machine (Homebrew, macOS Software Update, Mac App Store, apt, dnf, pacman, winget, scoop, npm, pnpm, yarn 1 and bun globals, Node versions via fnm or nvm, pip, uv, cargo, rustup, gem, composer, mise, asdf) with repoke. Use when the user asks what is outdated, to update or upgrade their system, tools, Node or global packages, or to schedule updates.
---

# repoke

repoke finds the package managers installed here and checks or upgrades them all with one command. Use its MCP tools (`check_updates`, `upgrade`, `list_managers`, `export`, `init_preview`, `schedule_status`) when they are available; otherwise run the `repoke` command, always with `--json`.

## Checking

Call `check_updates`, or run `repoke check --json`. Exit codes: 0 everything is up to date, 100 updates are available, 1 a manager could not be checked (its `error` says why), 2 bad arguments.

Report the result grouped by manager, with current and latest versions. Say which managers failed to check and why; don't hide them.

## Upgrading

1. Unless the user already said to upgrade everything, ask what to upgrade. Narrow it with `only` (managers), `packages` (`name` or `manager:name`) and `hold` (packages to leave alone).
2. Preview first: `upgrade` with the default `dry_run`, or `repoke upgrade --yes --no-sudo --dry-run --json`. Show the commands it would run.
3. Run it with `dry_run: false`, or `repoke upgrade --yes --no-sudo --json` with the same filters.
4. Report from the events: `done` and `failed` per package (with the error's last lines), `skipped`, then the `summary` totals.

Never upgrade without `--no-sudo` and never run sudo yourself. Managers that need root (apt, dnf, pacman, a system npm) and Homebrew casks that ask for a password come back as `skipped`; tell the user to run `repoke upgrade` in their own terminal for those. npm, pnpm and yarn globals whose new version needs a newer Node are `skipped` too, with the version they need. Exit code 75 means another upgrade is already running: wait and try again.

Node versions under fnm or nvm show up as `node@<major>`: upgrading moves to the newest release of that major line and carries the global npm packages over. Moving to a new major is the user's call.

## Moving to another machine

`export` (or `repoke export --yes --json --output <folder>`) saves what is installed to a folder; config files only with `include_config`, and secret-looking values are removed from them. On the new machine, `init_preview` with `from` shows what `repoke init --from <folder>` would install. Installing is the user's call: suggest the command and let them run it in a terminal.

## Scheduling

`repoke schedule status --json` (or `schedule_status`) says whether a periodic run is set up and how the last one went. Setting one up (`repoke schedule install`) registers a job with the system scheduler, so suggest the command and let the user run it.
