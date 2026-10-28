# Command reference

Every command takes `--help`. Lists of managers or packages are comma separated: `--only brew,npm`. Manager ids are the ones `repoke list` prints; packages are written `name` or `manager:name` (`eslint`, `npm:eslint`).

## repoke list

The package managers repoke supports on this system, in two groups: the ones found, with a ✓ and their version, and the ones that are not installed. Plugins appear after the built-in managers and are marked as such.

| Option | |
|--------|---|
| `--json` | Print JSON instead of text |

## repoke check

What is outdated, per manager, with the installed and the latest version. It changes nothing. Managers are checked side by side.

| Option | |
|--------|---|
| `--only <ids>` | Only these managers |
| `--exclude <ids>` | Skip these managers |
| `--hold <packages>` | Leave these packages out |
| `--refresh` | Refresh package indexes first (`brew update`, or a plugin's `refresh`) |
| `--json` | Print JSON instead of text |

Exits 0 when everything is up to date, 100 when there are updates, 1 when a manager could not be checked.

## repoke upgrade

Upgrades what is outdated. In a terminal it shows a list with everything selected: space toggles a package, the right arrow selects all, the left arrow none, Escape cancels. Outside a terminal it needs `--yes`.

| Option | |
|--------|---|
| `--only <ids>` | Only these managers |
| `--exclude <ids>` | Skip these managers |
| `--package <packages>` | Only these packages |
| `--hold <packages>` | Leave these packages alone |
| `--refresh` | Refresh package indexes first |
| `-y`, `--yes` | Upgrade everything selected without asking |
| `--dry-run` | Print the commands that would run, change nothing |
| `--no-sudo` | Skip managers that need root instead of calling `sudo` |
| `--json` | Print one JSON event per line |

Managers run one after another, each with its own spinner showing the last line of its output. If a batch fails, its packages are retried one at a time, so one bad package doesn't hold back the rest. A manager that needs root runs through `sudo -n`; in a terminal repoke asks for the password once, up front. Homebrew casks that install a `.pkg` or remove system services ask for the password themselves, so they run in a batch of their own: in a terminal repoke asks first, with `--yes` they run only if `sudo` still remembers the password, and `--no-sudo` skips them.

npm, pnpm and Yarn 1 globals whose new version declares an `engines.node` range the Node in use doesn't meet are skipped with the version they need, instead of installing and then failing to run.

Only one `upgrade` or `init` runs at a time: a second one exits with 75 straight away.

Exits 0 when everything selected was upgraded, 1 when something failed, 75 when another run was in progress, 130 when the list was cancelled.

## repoke export

Saves what each manager has installed on purpose (not dependencies) to a folder another machine can restore. In a terminal it asks, manager by manager, whether to include its config files.

| Option | |
|--------|---|
| `--only <ids>` | Only these managers |
| `--exclude <ids>` | Skip these managers |
| `-o`, `--output <folder>` | Where to write it (default: `repoke-<host>-<date>`) |
| `--config` | Include every manager's config files without asking |
| `--no-config` | Leave all config files out |
| `-y`, `--yes` | Ask nothing: config only with `--config`, and secret-looking values are removed |
| `--encrypt` | Include every config file, secrets and all, encrypted with a password |
| `--force` | Replace a snapshot already in the folder |
| `--json` | Print JSON instead of text |

The folder holds `repoke.toml`, one line per package (`cask:firefox`, `eslint==10.13.0`), and `files/` with the config files at their path under your home. Lines that look like secrets (`_authToken=…`, `password: …`, private keys) are found before saving: interactively you choose to remove their values, leave the file out or keep it; unattended, the values are always removed. A config file that is a link into a git repository is recorded as coming from your dotfiles instead of being copied.

With `--encrypt`, or by choosing to keep a file's secrets encrypted, config files go into `files.age` with their secrets intact, encrypted with a password that repoke asks for (or reads from `REPOKE_PASSWORD`). It is a standard [age](https://age-encryption.org) file: `age -d files.age` opens it without repoke. `repoke.toml` stays readable.

## repoke init

Sets up a machine, from a snapshot or from a catalog of common tools.

| Option | |
|--------|---|
| `--from <folder>` | A snapshot written by `repoke export` |
| `--tools <ids>` | Catalog tools to install without the picker |
| `--only <ids>` | Only these managers |
| `--exclude <ids>` | Skip these managers |
| `-y`, `--yes` | Install everything without asking |
| `--dry-run` | Show what would be installed and restored, change nothing |
| `--no-sudo` | Skip managers that need root |
| `--no-config` | Don't restore config files |
| `--overwrite` | Replace config files that differ without asking; yours are backed up first |
| `--json` | Print JSON instead of text |

With `--from`, managers that don't exist on this system are skipped, only what is missing is installed, and Homebrew goes first, since it often installs the other managers. A manager that is still missing is offered through Homebrew. Config files that are missing are restored, equal ones are left alone, and different ones are kept unless you choose the snapshot's copy; yours is saved as `<file>.repoke-<date>.bak`. Values removed from secrets are filled back from the file already on the machine. When the snapshot has encrypted config files, the password is asked before anything is installed (or read from `REPOKE_PASSWORD`); a `--dry-run` without it lists them as not compared.

Without `--from`, repoke offers a catalog of tools (git, gh, ripgrep, fd, bat, fzf, neovim, VS Code…) and installs each with the first manager on the machine that has it.

## repoke schedule

Runs repoke on its own through the system scheduler: a launchd agent on macOS, a systemd user timer on Linux, Task Scheduler on Windows. A run missed while the machine was asleep or off happens at the next wake or boot.

### repoke schedule install

Registers the periodic run, replacing an earlier one.

| Option | |
|--------|---|
| `--every <daily\|weekly>` | How often (default: daily; weekly runs are on Mondays) |
| `--at <HH:MM>` | Local time, 24-hour clock (default: 03:00) |
| `--check-only` | Only check and notify; don't upgrade |
| `--only <ids>` | Only these managers |
| `--exclude <ids>` | Skip these managers |
| `--dry-run` | Print the scheduler files and commands, register nothing |

By default the job runs `repoke upgrade --yes --no-sudo`. Schedulers start jobs with a bare `PATH`, so `install` saves yours (with fnm's per-shell folder swapped for its `default` alias); run it again after changing your `PATH`. Upgrading repoke doesn't touch the schedule: the job runs the `repoke` on your PATH, which Homebrew, cargo and `self-update` keep current. Without systemd it prints the crontab line to add instead.

### repoke schedule status

Whether a run is scheduled, when, and how the last one went. `--json` prints it as JSON.

### repoke schedule run

What the scheduler runs, and a way to try the job now. It keeps the output of the last run and a `history.jsonl` in `~/Library/Logs/repoke` (`~/.local/state/repoke` on Linux, `%LOCALAPPDATA%\repoke` on Windows), and sends a system notification when something was upgraded, failed or is pending. The history keeps the last 1000 runs.

### repoke schedule remove

Unregisters the run. `--dry-run` prints what would be removed.

## repoke self-update

Updates repoke. If Homebrew or cargo installed it, it runs `brew upgrade repoke` or `cargo install repoke --locked`; otherwise it downloads the release for your system, checks its SHA-256 and swaps the binary in place.

| Option | |
|--------|---|
| `--check` | Only say whether a newer version exists; exits 100 when it does |
| `-y`, `--yes` | Update without asking |
| `--json` | Print JSON instead of text |

In a terminal, repoke also mentions a new version at most once a day. `REPOKE_NO_UPDATE_CHECK=1` turns that off.

## repoke mcp

An MCP server over stdio, so AI agents can use repoke as a tool. It has six tools:

| Tool | What it does |
|------|--------------|
| `list_managers` | The managers on the machine and their versions |
| `check_updates` | What is outdated; takes `only`, `exclude`, `hold`, `refresh` |
| `upgrade` | Upgrades; takes `only`, `exclude`, `hold`, `packages` and `dry_run`. It only previews unless `dry_run` is `false`, and never uses sudo |
| `export` | Saves a snapshot to `output`; takes `only`, `exclude` and `include_config` (config files are left out by default) |
| `init_preview` | What `repoke init` would do with a snapshot (`from`) or catalog `tools`, changing nothing; takes `only` and `exclude` |
| `schedule_status` | The schedule and the last run |

In Claude Code, `/plugin install repoke --marketplace paveldelpozo/repoke` adds the server and a skill on when to use it. In Codex, add to `~/.codex/config.toml`:

```toml
[mcp_servers.repoke]
command = "repoke"
args = ["mcp"]
```

## repoke plugin

Managers added without changing repoke: a TOML file in `~/.config/repoke/managers/`, or a `repoke-<id>` executable on your PATH. How to write one: the [plugin guide](plugins.html).

| Command | |
|---------|---|
| `repoke plugin list [--json]` | The plugins found, and any problem with them |
| `repoke plugin new <id> [--exec] [-o <path>]` | A template to start from: TOML by default, a script with `--exec` |
| `repoke plugin search [--json]` | Official and community plugins, with downloads and votes |
| `repoke plugin install <id> [-y] [--force]` | Downloads an official plugin, checks it against the sha256 pinned in the index, shows its code and asks before installing it |
| `repoke plugin remove <id>` | Removes a plugin that `plugin install` installed |

## Configuration

`~/.config/repoke/config.toml` (`%APPDATA%\repoke\config.toml` on Windows, or the file in `REPOKE_CONFIG`):

```toml
exclude = ["mas"]                # managers left out unless named in --only
hold = ["eslint", "pnpm:pnpm"]   # never upgraded nor reported, as name or manager:name
plugin_timeout = "10m"           # longest a plugin command may run (600, 90s, 10m, 1h; 0 = no limit)

[timeouts]                       # longest one of a manager's commands may run, checks included
brew = "30m"                     # by manager id; plugins too, overriding plugin_timeout

[brew]
greedy = true                    # also casks that update themselves

[node]
remove_old = true                # fnm and nvm: once a line's new release runs, uninstall its older ones
```

| Variable | |
|----------|---|
| `REPOKE_CONFIG` | Use another config file |
| `REPOKE_PLUGIN_TIMEOUT` | Overrides `plugin_timeout` for one run |
| `REPOKE_NO_PLUGINS` | Ignore every plugin for one run |
| `REPOKE_PLUGIN_INDEX` | Another plugin index, a URL or a local file |
| `REPOKE_NO_UPDATE_CHECK` | Never mention a new repoke version |
| `REPOKE_PASSWORD` | Password for encrypted config files in `export --encrypt` and `init --from`, without asking |
| `NO_COLOR` | Output without colors |

## JSON output

`check --json` prints one object per manager: `id`, `name`, `version`, `packages` (each with `name`, `current`, `latest`) and `error`.

`upgrade --json` and `init --json` print one event per line, as they happen:

| Event | When |
|-------|------|
| `check_failed` | A manager could not be checked |
| `running` | A command starts |
| `planned` | With `--dry-run`, the commands that would run |
| `done` | Packages upgraded or installed, with `seconds` |
| `failed` | Packages that failed, with the `error` and sometimes a `hint` |
| `skipped` | Packages left out, with the `reason` |
| `retrying` | A batch failed and is retried one package at a time |
| `config` | `init` only: what happened to a config file |
| `summary` | Totals, the whole run's `elapsed_seconds` and a per-manager breakdown |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Done, or everything is up to date |
| 100 | `check` and `self-update --check`: there is something to update |
| 1 | A manager failed to check, upgrade or install |
| 2 | Invalid arguments or config, or `--yes` needed outside a terminal |
| 75 | `upgrade` and `init`: another `upgrade` or `init` is running |
| 130 | The interactive list was cancelled |
