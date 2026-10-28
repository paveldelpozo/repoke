# Referencia de comandos

Todos los comandos aceptan `--help`. Las listas de gestores o de paquetes van separadas por comas: `--only brew,npm`. Los ids de los gestores son los que imprime `repoke list`; los paquetes se escriben `nombre` o `gestor:nombre` (`eslint`, `npm:eslint`).

## repoke list

Los gestores de paquetes que repoke admite en este sistema, si cada uno está instalado y en qué versión. Los plugins salen después de los gestores integrados.

| Opción | |
|--------|---|
| `--json` | Imprime JSON en vez de texto |

## repoke check

Lo que está desactualizado, por gestor, con la versión instalada y la última. No cambia nada. Los gestores se consultan a la vez.

| Opción | |
|--------|---|
| `--only <ids>` | Solo estos gestores |
| `--exclude <ids>` | Salta estos gestores |
| `--hold <paquetes>` | Deja fuera estos paquetes |
| `--refresh` | Refresca antes los índices (`brew update`, o el `refresh` de un plugin) |
| `--json` | Imprime JSON en vez de texto |

Sale con 0 si todo está al día, con 100 si hay actualizaciones y con 1 si algún gestor no se ha podido consultar.

## repoke upgrade

Actualiza lo que está desactualizado. En una terminal enseña una lista con todo marcado: la barra espaciadora marca o desmarca un paquete, la flecha derecha marca todo, la izquierda nada, y Escape cancela. Fuera de una terminal necesita `--yes`.

| Opción | |
|--------|---|
| `--only <ids>` | Solo estos gestores |
| `--exclude <ids>` | Salta estos gestores |
| `--package <paquetes>` | Solo estos paquetes |
| `--hold <paquetes>` | No toca estos paquetes |
| `--refresh` | Refresca antes los índices |
| `-y`, `--yes` | Actualiza todo lo seleccionado sin preguntar |
| `--dry-run` | Imprime los comandos que lanzaría, sin cambiar nada |
| `--no-sudo` | Salta los gestores que necesitan root en vez de usar `sudo` |
| `--json` | Imprime un evento JSON por línea |

Los gestores van uno detrás de otro, cada uno con su indicador mostrando la última línea de su salida. Si un lote falla, sus paquetes se reintentan de uno en uno, para que un paquete problemático no frene al resto. Un gestor que necesita root se ejecuta con `sudo -n`; en una terminal, repoke pide la contraseña una vez, al principio. Los casks de Homebrew que instalan un `.pkg` o quitan servicios del sistema piden la contraseña por su cuenta, así que van en un lote aparte: en una terminal repoke la pide antes, con `--yes` solo se lanzan si `sudo` aún la recuerda, y con `--no-sudo` se saltan.

Los globales de npm, pnpm y Yarn 1 cuya nueva versión declara en `engines.node` un rango que no cumple el Node en uso se saltan, indicando qué versión necesitan, en vez de instalarse y luego no arrancar.

Solo puede haber un `upgrade` o un `init` a la vez: si ya hay uno en marcha, el segundo sale enseguida con 75.

Sale con 0 si se actualizó todo lo seleccionado, con 1 si algo falló, con 75 si había otra ejecución en marcha y con 130 si se canceló la lista.

## repoke export

Guarda lo que cada gestor tiene instalado a propósito (sin dependencias) en una carpeta que otra máquina puede restaurar. En una terminal pregunta, gestor por gestor, si incluir sus ficheros de configuración.

| Opción | |
|--------|---|
| `--only <ids>` | Solo estos gestores |
| `--exclude <ids>` | Salta estos gestores |
| `-o`, `--output <carpeta>` | Dónde guardarla (por defecto, `repoke-<equipo>-<fecha>`) |
| `--config` | Incluye la configuración de todos los gestores sin preguntar |
| `--no-config` | Deja fuera toda la configuración |
| `-y`, `--yes` | No pregunta nada: configuración solo con `--config`, y se quitan los valores que parecen secretos |
| `--encrypt` | Incluye toda la configuración, con sus secretos, cifrada con una contraseña |
| `--force` | Sustituye una foto que ya esté en la carpeta |
| `--json` | Imprime JSON en vez de texto |

La carpeta contiene `repoke.toml`, una línea por paquete (`cask:firefox`, `eslint==10.13.0`), y `files/`, con los ficheros de configuración en su ruta dentro de tu carpeta personal. Las líneas que parecen secretos (`_authToken=…`, `password: …`, claves privadas) se detectan antes de guardar: en una terminal eliges quitar sus valores, dejar el fichero fuera o guardarlo tal cual; sin terminal, los valores se quitan siempre. Un fichero de configuración que es un enlace a un repositorio git se apunta como parte de tus dotfiles en vez de copiarse.

Con `--encrypt`, o si eliges guardar cifrados los secretos de un fichero, la configuración va a `files.age` con sus secretos intactos, cifrada con una contraseña que repoke te pide (o lee de `REPOKE_PASSWORD`). Es un fichero [age](https://age-encryption.org) estándar: `age -d files.age` lo abre sin repoke. `repoke.toml` sigue siendo legible.

## repoke init

Prepara una máquina, a partir de una foto o de un catálogo de herramientas habituales.

| Opción | |
|--------|---|
| `--from <carpeta>` | Una foto escrita por `repoke export` |
| `--tools <ids>` | Herramientas del catálogo que instalar sin la lista |
| `--only <ids>` | Solo estos gestores |
| `--exclude <ids>` | Salta estos gestores |
| `-y`, `--yes` | Instala todo sin preguntar |
| `--dry-run` | Enseña lo que instalaría y restauraría, sin cambiar nada |
| `--no-sudo` | Salta los gestores que necesitan root |
| `--no-config` | No restaura la configuración |
| `--overwrite` | Sustituye sin preguntar los ficheros de configuración que difieren; antes guarda una copia de los tuyos |
| `--json` | Imprime JSON en vez de texto |

Con `--from`, los gestores que no existen en este sistema se saltan, solo se instala lo que falta y Homebrew va primero, porque a menudo instala los demás gestores. Si sigue faltando algún gestor, se ofrece instalarlo con Homebrew. Los ficheros de configuración que faltan se restauran, los iguales no se tocan y los que difieren se conservan salvo que elijas la copia de la foto; el tuyo se guarda como `<fichero>.repoke-<fecha>.bak`. Los valores quitados de los secretos se rellenan con los del fichero que ya hay en la máquina. Si la foto tiene configuración cifrada, la contraseña se pide antes de instalar nada (o se lee de `REPOKE_PASSWORD`); un `--dry-run` sin ella la enseña como no comparada.

Sin `--from`, repoke ofrece un catálogo de herramientas (git, gh, ripgrep, fd, bat, fzf, neovim, VS Code…) e instala cada una con el primer gestor de la máquina que la tenga.

## repoke schedule

Ejecuta repoke por su cuenta mediante el planificador del sistema: un agente de launchd en macOS, un temporizador de systemd de usuario en Linux y el Programador de tareas en Windows. Si la máquina estaba dormida o apagada a la hora prevista, la ejecución se hace al despertar o arrancar.

### repoke schedule install

Registra la ejecución periódica, sustituyendo la anterior si la había.

| Opción | |
|--------|---|
| `--every <daily\|weekly>` | Cada cuánto (por defecto, `daily`; `weekly` es los lunes) |
| `--at <HH:MM>` | Hora local, en formato de 24 horas (por defecto, 03:00) |
| `--check-only` | Solo consulta y avisa; no actualiza |
| `--only <ids>` | Solo estos gestores |
| `--exclude <ids>` | Salta estos gestores |
| `--dry-run` | Imprime los ficheros y comandos del planificador, sin registrar nada |

Por defecto, la tarea ejecuta `repoke upgrade --yes --no-sudo`. Los planificadores arrancan las tareas con un `PATH` mínimo, así que `install` guarda el tuyo (cambiando la carpeta por sesión de fnm por su alias `default`); vuelve a lanzarlo si cambias el `PATH`. Actualizar repoke no afecta a la programación: la tarea ejecuta el `repoke` de tu PATH, que Homebrew, cargo y `self-update` mantienen al día. Sin systemd, imprime la línea de crontab que hay que añadir.

### repoke schedule status

Si hay una ejecución programada, cuándo, y cómo fue la última. Con `--json`, la imprime en JSON.

### repoke schedule run

Lo que ejecuta el planificador, y una forma de probar la tarea en el momento. Guarda la salida de la última ejecución y un `history.jsonl` en `~/Library/Logs/repoke` (`~/.local/state/repoke` en Linux, `%LOCALAPPDATA%\repoke` en Windows), y manda una notificación del sistema cuando se actualizó algo, algo falló o hay algo pendiente. El historial guarda las últimas 1000 ejecuciones.

### repoke schedule remove

Quita la ejecución programada. Con `--dry-run`, imprime lo que quitaría.

## repoke self-update

Actualiza repoke. Si lo instaló Homebrew o cargo, ejecuta `brew upgrade repoke` o `cargo install repoke --locked`; si no, descarga la release para tu sistema, comprueba su SHA-256 y sustituye el binario.

| Opción | |
|--------|---|
| `--check` | Solo dice si hay una versión más nueva; sale con 100 si la hay |
| `-y`, `--yes` | Actualiza sin preguntar |
| `--json` | Imprime JSON en vez de texto |

En una terminal, repoke también avisa de una versión nueva, como mucho una vez al día. `REPOKE_NO_UPDATE_CHECK=1` lo desactiva.

## repoke mcp

Un servidor MCP por stdio, para que los agentes de IA usen repoke como herramienta. Tiene seis herramientas:

| Herramienta | Qué hace |
|-------------|----------|
| `list_managers` | Los gestores de la máquina y sus versiones |
| `check_updates` | Lo que está desactualizado; admite `only`, `exclude`, `hold` y `refresh` |
| `upgrade` | Actualiza; admite `only`, `exclude`, `hold`, `packages` y `dry_run`. Solo previsualiza salvo que `dry_run` sea `false`, y nunca usa sudo |
| `export` | Guarda una foto en `output`; admite `only`, `exclude` e `include_config` (por defecto, sin ficheros de configuración) |
| `init_preview` | Lo que haría `repoke init` con una foto (`from`) o con herramientas del catálogo (`tools`), sin cambiar nada; admite `only` y `exclude` |
| `schedule_status` | La programación y la última ejecución |

En Claude Code, `/plugin install repoke --marketplace paveldelpozo/repoke` añade el servidor y una skill que le dice cuándo usarlo. En Codex, añade a `~/.codex/config.toml`:

```toml
[mcp_servers.repoke]
command = "repoke"
args = ["mcp"]
```

## repoke plugin

Gestores añadidos sin cambiar repoke: un fichero TOML en `~/.config/repoke/managers/` o un ejecutable `repoke-<id>` en tu PATH. Cómo escribir uno: la [guía de plugins](plugins.html).

| Comando | |
|---------|---|
| `repoke plugin list [--json]` | Los plugins encontrados, y cualquier problema que tengan |
| `repoke plugin new <id> [--exec] [-o <ruta>]` | Una plantilla para empezar: TOML por defecto, un script con `--exec` |
| `repoke plugin search [--json]` | Los plugins oficiales |
| `repoke plugin install <id> [-y] [--force]` | Descarga un plugin oficial, lo comprueba con el sha256 fijado en el índice, enseña su código y pregunta antes de instalarlo |
| `repoke plugin remove <id>` | Quita un plugin que instaló `plugin install` |

## Configuración

`~/.config/repoke/config.toml` (`%APPDATA%\repoke\config.toml` en Windows, o el fichero de `REPOKE_CONFIG`):

```toml
exclude = ["mas"]                # gestores fuera salvo que los nombre --only
hold = ["eslint", "pnpm:pnpm"]   # nunca se actualizan ni se informan, como nombre o gestor:nombre
plugin_timeout = "10m"           # tiempo máximo de un comando de plugin (600, 90s, 10m, 1h; 0 = sin límite)

[timeouts]                       # tiempo máximo de cada comando de un gestor, consultas incluidas
brew = "30m"                     # por id de gestor; también plugins, y entonces manda sobre plugin_timeout

[brew]
greedy = true                    # también los casks que se actualizan solos

[node]
remove_old = true                # fnm y nvm: cuando la nueva versión de una línea arranca, desinstala las anteriores
```

| Variable | |
|----------|---|
| `REPOKE_CONFIG` | Usa otro fichero de configuración |
| `REPOKE_PLUGIN_TIMEOUT` | Sustituye a `plugin_timeout` en una ejecución |
| `REPOKE_NO_PLUGINS` | Ignora todos los plugins en una ejecución |
| `REPOKE_PLUGIN_INDEX` | Otro índice de plugins, una URL o un fichero local |
| `REPOKE_NO_UPDATE_CHECK` | No avisa nunca de versiones nuevas de repoke |
| `REPOKE_PASSWORD` | Contraseña de la configuración cifrada en `export --encrypt` e `init --from`, sin preguntarla |
| `NO_COLOR` | Salida sin colores |

## Salida en JSON

`check --json` imprime un objeto por gestor: `id`, `name`, `version`, `packages` (cada uno con `name`, `current` y `latest`) y `error`.

`upgrade --json` e `init --json` imprimen un evento por línea, según ocurren:

| Evento | Cuándo |
|--------|--------|
| `check_failed` | Un gestor no se ha podido consultar |
| `running` | Empieza un comando |
| `planned` | Con `--dry-run`, los comandos que se lanzarían |
| `done` | Paquetes actualizados o instalados, con `seconds` |
| `failed` | Paquetes que fallaron, con el `error` y a veces una pista (`hint`) |
| `skipped` | Paquetes que se dejaron fuera, con el motivo (`reason`) |
| `retrying` | Un lote falló y se reintenta paquete a paquete |
| `config` | Solo `init`: qué pasó con un fichero de configuración |
| `summary` | Totales, la duración de toda la ejecución (`elapsed_seconds`) y el desglose por gestor |

## Códigos de salida

| Código | Significado |
|--------|-------------|
| 0 | Hecho, o todo está al día |
| 100 | `check` y `self-update --check`: hay algo que actualizar |
| 1 | Un gestor falló al consultar, actualizar o instalar |
| 2 | Argumentos o configuración no válidos, o falta `--yes` fuera de una terminal |
| 75 | `upgrade` e `init`: hay otro `upgrade` o `init` en marcha |
| 130 | Se canceló la lista interactiva |
