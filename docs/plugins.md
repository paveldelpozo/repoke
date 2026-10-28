# Writing a repoke plugin

repoke knows 24 package managers out of the box. A plugin adds another one, and it then takes part in everything: `list`, `check`, `upgrade`, `export`, `init`, `schedule` and the MCP server. There are two kinds:

- **A TOML file** that says which commands to run and how to read their output. Enough for most tools that can print what is outdated.
- **An executable** named `repoke-<id>`, in any language, that answers a small JSON protocol. For tools that need logic, like a git clone or several commands per check.

Official plugins live in [repoke-plugins](https://github.com/paveldelpozo/repoke-plugins): `repoke plugin search` lists them and `repoke plugin install <id>` shows a plugin's code, asks, and copies it into `~/.local/bin` or the managers folder. `REPOKE_PLUGIN_INDEX` points those commands at another index, a URL or a local file.

`repoke plugin new <id>` writes a TOML template to start from, and `repoke plugin new <id> --exec` a shell script. `repoke plugin list` shows the plugins repoke found and any problem with them. Two examples are in [`examples/plugins`](../examples/plugins): GitHub CLI extensions (TOML) and oh-my-zsh (executable).

A plugin's id must not clash with a built-in manager or with another plugin; `plugin list` reports the clash and the plugin is not loaded. `REPOKE_NO_PLUGINS=1` turns all plugins off for one run.

Plugins run with your user's permissions, like any command on your PATH. Read one before installing it.

Every command repoke runs for a plugin, upgrades included, is stopped after 10 minutes and reported as a failure of that plugin. Change the limit with `plugin_timeout` in `~/.config/repoke/config.toml` (`600`, `90s`, `10m`, `1h`, or `0` for none), or for one run with `REPOKE_PLUGIN_TIMEOUT`.

## TOML managers

repoke reads every `*.toml` file in `~/.config/repoke/managers/` (`%APPDATA%\repoke\managers\` on Windows).

```toml
id = "gh-ext"
name = "GitHub CLI extensions"
binary = "gh"               # every command below runs this program
# os = ["macos", "linux"]   # optional; all systems by default
# needs_root = true         # upgrades go through sudo, and are skipped with --no-sudo

[outdated]
command = ["extension", "upgrade", "--all", "--dry-run"]
format = "lines"
pattern = '^\[(?P<name>[^\]]+)\]: would have upgraded from (?P<current>\S+) to (?P<latest>\S+)'

[upgrade]
command = ["extension", "upgrade", "{name}"]
```

The manager counts as installed when `binary` is on the PATH, and its version is the first version-looking word of `binary --version`.

### Reading output

`format = "json"` reads `items` as a JSON array and takes `name`, `current` and `latest` from each item. Numbers are turned into text; missing fields are left empty. For a tool whose `outdated --json` printed `{"tools": [{"id": "x", "installed": ["1.0"], "available": "1.1"}]}`:

```toml
[outdated]
command = ["outdated", "--json"]
format = "json"
items = "/tools"            # JSON pointer to the list; "" when the output is the list
name = "id"                 # a field of each item, or a JSON pointer inside it
current = "/installed/0"
latest = "available"
```

`format = "lines"` matches a regular expression against every line of output and keeps the lines that match. Named groups give the fields; `name` is required. The example at the top reads lines like `[gh-dash]: would have upgraded from v4.1.0 to v4.2.0`.

A command must exit 0 unless you list other accepted codes in `ok_codes = [0, 1]`; some tools exit 1 when they find something outdated.

### Commands and placeholders

- `{packages}` as an argument expands to every package being upgraded, as separate arguments. One command upgrades them all.
- An argument containing `{name}` or `{version}` makes one command per package, with the placeholders replaced by its name and target version (`["install", "{name}@{version}"]`). A failure is then reported for that package only.
- Without placeholders the command runs once, whatever was selected (`["self-update"]`).

### Optional sections

| Section | Used by | Format |
|---------|---------|--------|
| `[refresh]` | `check --refresh`, `upgrade --refresh` | `command` |
| `[installed]` | `export` | like `[outdated]`, with `name` and `version` |
| `[install]` | `init` | like `[upgrade]`; `{version}` is the version recorded in the snapshot |

Unknown keys are an error, so a typo shows up in `repoke plugin list` instead of being ignored.

## Executable plugins, protocol 1

An executable plugin is a program called `repoke-<id>` (`repoke-<id>.exe` or `.cmd` on Windows) anywhere on your PATH. When two have the same name, the first on the PATH wins, as with any command.

repoke runs it with one subcommand, stdin closed, and reads stdout. A non-zero exit is a failure, and the last lines of stderr become the error shown to the user. Never wait for input: there is nobody to answer.

### `describe`

Runs every time repoke starts, so keep it instant: no network, no slow commands.

```json
{"protocol": 1, "id": "omz", "name": "oh-my-zsh", "os": ["macos", "linux"], "needs_root": false, "capabilities": []}
```

- `protocol`: the version of this document the plugin follows. repoke refuses plugins newer than it understands and asks the user to update.
- `id`: must match the file name, `repoke-<id>`.
- `os`: optional; values are `macos`, `linux` and `windows`. Empty means all.
- `needs_root`: optional; when true, `upgrade` runs through `sudo -n`, and `--no-sudo` skips it.
- `capabilities`: optional subcommands it implements, among `refresh`, `installed` and `install`.

### `detect`

Whether the tool the plugin manages is on this machine.

```json
{"installed": true, "version": "7ea697f"}
```

When `installed` is false, repoke treats the manager as absent, and skips it.

### `outdated`

What can be upgraded, as a list. An empty list means everything is up to date.

```json
[{"name": "oh-my-zsh", "current": "7ea697f", "latest": "17a2b4c"}]
```

### `upgrade <name>...`

Upgrades the packages named, which are always names that `outdated` returned. Print progress as you go: repoke shows the last line next to its spinner. Exit 0 when everything was upgraded. If one package can fail without the others, repoke retries them one at a time after a failed batch, so failing the whole call is fine.

### `refresh` (capability)

Updates the local index before `outdated`, for `--refresh`.

### `installed` (capability)

What the user installed on purpose, for `repoke export`.

```json
[{"name": "gh-dash", "version": "4.2.0"}]
```

An item may add `"kind"`, a string your plugin uses to tell apart things with the same name (a source, a channel). It comes back to you in `install` only through the name, so keep it informational.

### `install <name>...` (capability)

Installs those packages on a new machine, for `repoke init`.

### Versioning

Additions that older repoke versions can ignore (a new optional field, a new capability) keep protocol 1. A change that would make an older repoke misread a plugin gets a new protocol number, and repoke keeps understanding the older ones.

## Testing a plugin

```sh
repoke plugin list                      # found, and valid?
repoke list                             # installed, and which version?
repoke check --only <id>                # what is outdated
repoke upgrade --only <id> --dry-run    # the commands it would run
```
