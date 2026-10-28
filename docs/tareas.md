# Tareas de repoke

Lista viva del proyecto. Se marcan, añaden y anotan aquí las tareas, los hallazgos menores y las preguntas abiertas. Las decisiones y aprendizajes van al cerebro (`Proyectos/repoke`).

## Estado al cierre de la sesión del 28-10-2026

Publicado: **v0.3.0** (28-10 por la tarde; antes v0.2.1) en GitHub (5 binarios), crates.io, el tap `paveldelpozo/tap` y el plugin de Claude Code (activo en todos los perfiles de Pavel); landing bilingüe en https://repoke.paveldelpozo.es (Dokploy, proyecto «repoke» → «Landing», `applicationId` `llp0KBoAGGxkY8k40Q9WG`, se redespliega sola); plugins oficiales en `paveldelpozo/repoke-plugins` (`omz`, `vscode`). En el Mac de Pavel: repoke instalado con cargo desde el código local, plugins `omz` y `vscode`, `[brew] greedy = true` y el modo programado a las 03:00.

Lo siguiente, por orden (actualizado el 28-10 por la noche, antes de reiniciar la sesión):

1. **v0.4.0 publicada** (28-10, noche, la lanzó Pavel): GitHub con 5 binarios, crates.io, tap y landing. Incluye asdf, el cifrado del backup y el aviso de cookies de GA4 de la landing.
2. **Pavel, 29-10 por la mañana:** `repoke schedule status` para ver cómo fue la primera ejecución programada (y `~/Library/Logs/repoke/last-run.log` si algo falló).
3. **Un compañero de Pavel, como beta-tester:** `brew install` desde el tap tras `brew trust paveldelpozo/tap`.
4. **En espera, hasta que Pavel pueda probarlo en una imagen de Docker o en su portátil con Ubuntu:** probar en Linux lo que solo tiene tests de parseo (apt, dnf, pacman, zypper, flatpak, snap). Windows (winget, scoop, choco) sigue sin máquina donde probarlo.
5. **Cifrado del backup:** hecho con el crate `age` (28-10, noche). Pavel puede probarlo en su terminal: `repoke export --encrypt -o /tmp/snap` y `HOME=$(mktemp -d) repoke init --from /tmp/snap --dry-run`.
6. **Pruebas que solo puede hacer Pavel:** las preguntas interactivas de `export` e `init` en una terminal real; un cask que pida contraseña (`.pkg`); `[node] remove_old = true` en real (solo se ha probado con `--dry-run`).

Decisiones que tiene que tomar Pavel:

- **Índice comunitario de plugins:** dónde vive (otro repositorio, o el mismo `repoke-plugins` con una sección aparte) y quién revisa lo que entra.
- **deno y volta:** Pavel prefiere mantenerlos pendientes (28-10); propuesta original de descartarlos. deno no sabe decir qué globales están desactualizados, y volta lleva tiempo sin mantenimiento.

Hecho el 28-10 por la tarde: imagen Open Graph; robustez (`engines.node`, casks con contraseña, bloqueo, rotación del historial); landing en Astro; plugin de Claude probado en conversación; v0.3.0 publicada.

Hecho el 28-10 por la noche (en los commits sin subir): timeouts por gestor (`[timeouts]`); `--refresh` solo en gestores con índice; `engines.node` también en `init`; yarn 1 y bun; cargo en paralelo; `[node] remove_old`; Node LTS en el catálogo de `init`; pistas de brew para fórmulas desconocidas y taps que no se pueden clonar; herramientas MCP `export` e `init_preview`; documentación EN/ES, landing (con la salida real de `repoke list`), skill de Claude y README al día. Ojo: en el Mac de Pavel yarn 1 falla porque `~/.npmrc` usa `${NODE_AUTH_TOKEN}` sin definir; repoke lo explica.

Normas del proyecto: commits en Conventional Commits y en inglés, firmados con `paveldelpozo@gmail.com`; cada función nueva actualiza la landing y las referencias EN/ES (la CI lo vigila); tras cada cambio, `cargo install --path . --force`; releases con `scripts/release.sh --crate` (sin argumentos calcula la versión semántica).

## Fase 1 · Esqueleto y consulta (hecha)

- [x] Proyecto Rust con clap y serde, trait `Manager` y un fichero por gestor.
- [x] `repoke list`: gestores soportados en el sistema, ruta y versión.
- [x] `repoke check`: pendientes en paralelo, `--only`, `--exclude`, `--json`, códigos de salida 0/100/1/2.
- [x] Proveedores brew (formulae y casks), npm global y pnpm global, con tests de parseo.
- [x] CI en local con `scripts/ci.sh`: fmt, clippy para macOS, Linux y Windows, tests en el host y en Linux con Docker si está arrancado. El workflow de GitHub queda solo manual (el presupuesto de Actions se agotó el 28-10).
- [x] Hook de pre-push activado (28-10, a petición de Pavel): `scripts/ci.sh --quick` antes de cada push.

## Fase 2 · Actualizar (hecha)

- [x] `repoke upgrade` interactivo: lista con todo marcado (espacio marca, → todo, ← nada); Esc cancela con código 130.
- [x] Modo automático: `--yes`, `--dry-run`, `--only`, `--exclude`, `--hold`, salida JSON por líneas (un evento por paso).
- [x] Sudo: npm comprueba si su prefijo global es escribible. Siempre `sudo -n`; en interactivo se pide antes la contraseña con `sudo -v`. `--no-sudo` salta esos gestores.
- [x] Resumen final con actualizados, fallidos, saltados y planificados; los errores incluyen las últimas 20 líneas de salida.
- [x] `~/.config/repoke/config.toml` (o `REPOKE_CONFIG`) con `exclude` y `hold`; rechaza claves desconocidas.
- [x] `--refresh` lanza `brew update` antes de consultar.
- [x] Spinners en paralelo por gestor al consultar y, al actualizar, con la última línea de salida y el tiempo transcurrido. Solo por stderr y con terminal.
- [x] Primera prueba real en el Mac de Pavel (28-10): brew bien, npm y pnpm fallaron. Corregido:
  - npm instalaba `@latest`, que resolvió a npm 12.2.1 (exige node ≥ 22.22.2, Pavel tiene 22.13.1), y como npm instala todo o nada, cayeron los 4 globales. Ahora se instala la versión exacta que mostró `outdated` y, si falla un lote, se reintenta paquete a paquete.
  - pnpm se rechaza a sí mismo con `add -g` (ERR_PNPM_GLOBAL_PNPM_INSTALL): ahora usa `pnpm self-update <versión>`.
  - La línea con la selección se resume ("6 packages") y los avisos `[WARN]` / `npm warn` no entran en el mensaje de error.
- [x] Repetir la prueba real tras las correcciones: Pavel confirma que funciona (28-10).
- [x] Antes de instalar un global de npm o pnpm, comparar su `engines.node` con la versión de node en uso: se salta con la versión que necesita (`src/managers/engines.rs`). También en `init`, con la versión de la foto o la última si no hay.
- [x] Casks de brew que piden contraseña (`pkg`, `installer` o `uninstall` con pkgutil/launchctl/kext/script, según `brew info --json=v2`): van en un lote aparte; en terminal se pide `sudo -v` antes, con `--yes` solo si sudo la recuerda, con `--no-sudo` se saltan. Falta verlo con un cask real de ese tipo.
- [x] Salida agrupada por gestor: una línea por paquete con ✓/✗, versiones y tiempo; los fallos enseñan el comando y las últimas 10 líneas. Tabla resumen final por gestor con totales y duración (28-10, a petición de Pavel). Respeta `NO_COLOR`.
- [x] composer: `global require` con `--with-all-dependencies`; sin él, laravel/installer 5.32 falló porque `laravel/prompts` estaba fijado en el lock.
- [x] Timeouts por gestor: tabla `[timeouts]` en `config.toml` por id; vale para consultas y actualizaciones, y en un plugin manda sobre `plugin_timeout`.
- [x] Con `--refresh`, el spinner solo dice "refreshing" en los gestores que tienen índice (`Manager::has_refresh`).

## Fase 3 · Más gestores (hecha en código; falta probar los de Linux y Windows)

- [x] Núcleo: un paso puede tener varios comandos; cada gestor decide cómo localizarse (`locate`) y cómo invocarse (`invocation`), para nvm, que es una función de shell.
- [x] fnm: cada línea mayor instalada de node es un "paquete" que sube a la última de su misma línea (nunca salta de mayor). Mueve el `default` si es de esa línea y reinstala los globales **desde la versión en uso**, que es la `default`, no la más alta instalada.
- [x] nvm: igual que fnm, con `--reinstall-packages-from` y `alias default`. Se detecta por `$NVM_DIR` o `~/.nvm`.
- [x] pip: solo `--user`, sin `--break-system-packages`.
- [x] uv: herramientas y `uv self update` (solo si se instaló con el instalador propio).
- [x] cargo: crates del registro, consultando `cargo search`; ignora las instalaciones desde ruta o git.
- [x] rustup: toolchains y rustup.
- [x] mas, gem (salta el Ruby del sistema en macOS), composer global (mueve la restricción con `global require`), mise.
- [x] Linux: apt (simulación sin root), dnf, pacman (siempre `-Syu` completo, usa `checkupdates` si está), zypper, snap, flatpak.
- [x] Windows: winget, scoop, choco. winget y scoop no tienen JSON: se leen sus tablas por la posición de las columnas.
- [x] Probados de verdad en el Mac de Pavel (28-10): brew, npm, pnpm, fnm, pip, uv, cargo, rustup, composer.
- [ ] Probar apt, dnf, pacman, zypper y flatpak en contenedores Docker (`scripts/ci.sh` ya tiene el paso; falta arrancar Docker y añadir imágenes por distro).
- [ ] Probar winget, scoop y choco en un Windows real; la salida de winget cambia con el idioma y la versión.
- [ ] Probar mas, nvm, gem y mise en una máquina que los tenga.
- [x] yarn global (solo Yarn 1; explica el fallo por variables sin definir en `.npmrc`) y bun global, probados con datos reales (bun en una carpeta aislada, `BUN_INSTALL`).
- [x] asdf (0.16 o posterior, el binario en Go): lee las versiones globales de `~/.tool-versions` (no `asdf current`, que depende de la carpeta), las compara con `asdf latest --all` y actualiza con `install` + `set -u`; las versiones antiguas quedan instaladas. `init` hace `plugin add`, `install` y `set -u`. Probado de principio a fin con asdf 0.20.2 y jq en `debian:stable-slim` (28-10): `list`, `check`, `upgrade`, `export` e `init`.
- [x] `softwareupdate` de macOS (28-10, noche, lo pidió Pavel): `--list` y `--install <label>` con sudo; las que piden reinicio (el propio macOS) se retienen con aviso para hacerlas desde Ajustes, porque podrían reiniciar el Mac sin nadie delante y en Apple silicon piden la contraseña del usuario. Probado en el Mac de Pavel sin actualizaciones pendientes; el formato con actualizaciones sale de la documentación, falta verlo con una real.
- [ ] Pendientes: deno, volta (no están en el Mac de Pavel).
- [x] cargo consulta crates.io de 8 en 8 en paralelo (con 4 crates, de 1,8 s a 0,8 s).
- [x] fnm y nvm: con `[node] remove_old = true`, tras instalar la nueva versión de una línea y comprobar que arranca (`exec … node --version`), se desinstalan las anteriores de esa línea. Desactivado por defecto: un `.node-version` puede fijar una versión exacta. Probado con `--dry-run`, no en real.

## Fase 4 · `init` y `export` (hecha)

- [x] Motor común (`src/run.rs`) para `upgrade` e `init`: spinners, sudo, reintento paquete a paquete, informe y resumen.
- [x] Cada gestor sabe listar lo que se instaló a propósito (`installed`), instalar (`install_steps`) y decir sus ficheros de configuración (`config_files`). zypper aún no exporta.
- [x] `repoke export`: carpeta con `repoke.toml` (`kind:nombre==versión`, porque brew usa `@` en `python@3.12`) y `files/`. Pregunta gestor a gestor si incluir su configuración; `--config`, `--no-config`, `--yes`, `-o`, `--force`, `--json`.
- [x] Secretos: detecta `_authToken`, `password`, `token`, claves privadas… y deja las referencias `${VAR}`. En interactivo, quitar valores, dejar el fichero fuera o guardarlo tal cual; en desatendido, siempre quita los valores. Se copia con los mismos permisos.
- [x] Dotfiles: un enlace a un repositorio git se apunta con su remoto en vez de copiarse.
- [x] `repoke init --from`: salta los gestores de otro SO, instala solo lo que falta (todo marcado en la lista), Homebrew primero y ofrece instalar con brew el gestor que siga faltando.
- [x] Restauración: compara, restaura lo que falta, deja lo igual, y lo distinto lo conserva salvo que elijas la copia de la foto (`--overwrite` sin preguntar), con copia `.repoke-<fecha>.bak`. Los valores de secretos borrados se rellenan con los del fichero que ya hay: nunca se pierde un token real.
- [x] `repoke init` sin `--from`: catálogo de 26 herramientas (`src/catalog.toml`) con su nombre en cada gestor; usa el primero disponible. `--tools` para no usar la lista.
- [x] Probado en el Mac de Pavel: exportación real (84 paquetes de brew, `.npmrc` con el token quitado y la referencia `${NODE_AUTH_TOKEN}` intacta), `init --from` de su propia foto sin nada que hacer, y restauración real en un `HOME` falso.
- [ ] Probar las preguntas interactivas de `export` e `init` en una terminal real.
- [x] Cifrar el backup con contraseña (28-10, noche): `export --encrypt`, o la opción «Keep them, encrypted» por fichero, guarda la configuración con sus secretos en un único `files.age` (formato age, un solo scrypt por exportación o restauración; `age -d` lo abre sin repoke). `init` pide la contraseña antes de instalar nada (tres intentos) o la lee de `REPOKE_PASSWORD`; restaura con permisos `600`; `--dry-run` sin contraseña los marca como no comparados. Crate `age` 0.12: +66 KB al binario. Probado con un `HOME` falso; falta probar las preguntas interactivas en una terminal real y abrir un `files.age` con la herramienta `age`.
- [x] Node en el catálogo: `node@lts` con fnm (resuelve la línea LTS más nueva con `ls-remote --lts`) o nvm (`lts/*`), y queda como la versión por defecto. Solo se ofrece si fnm o nvm ya están instalados.
- [ ] Configuraciones que no son de un gestor (`~/.gitconfig`, `~/.zshrc`): vendrán con los plugins oficiales (fase 9).
- [x] Al restaurar en otra máquina: el lote fallido ya se reintentaba paquete a paquete; ahora brew explica las fórmulas o casks que no conoce (tap no configurado, renombrado) y los taps que no puede clonar (privados o borrados).

## Fase 5 · Modo programado (hecha e instalada)

- [x] `repoke schedule install --every daily|weekly --at HH:MM [--check-only] [--only/--exclude] [--dry-run]`: launchd (LaunchAgent), timer de systemd de usuario o Programador de tareas. Sin systemd, imprime la línea de crontab.
- [x] Por defecto actualiza (`upgrade --yes --no-sudo`), porque Pavel pidió "que mantenga actualizado el sistema"; `--check-only` solo consulta y avisa.
- [x] `schedule run`: lo que llama el planificador. Guarda la salida de la última ejecución y un `history.jsonl`; notifica (osascript en macOS, notify-send en Linux) solo si se actualizó, falló o hay pendientes.
- [x] `schedule status` (también `--json`) y `schedule remove`.
- [x] PATH: el planificador arranca con un PATH mínimo. Se guarda el del usuario sin duplicados y cambiando la carpeta por sesión de fnm (`fnm_multishells/…`, que desaparece) por `$FNM_DIR/aliases/default/bin`.
- [x] Probado en el Mac: plist validado con `plutil`, `schedule run` en modo consulta con un HOME falso, `status` con el historial.
- [x] Instalado en el Mac de Pavel (28-10).
- [ ] Comprobar el 29-10 por la mañana con `repoke schedule status` que la ejecución de las 03:00 fue bien (Pavel).
- [ ] Windows: no hay notificación (habría que usar un toast de PowerShell).
- [ ] Probar en Linux con systemd y sin él.
- [x] Evitar dos ejecuciones a la vez: `upgrade` e `init` bloquean `run.lock` en la carpeta de estado (`File::try_lock`, por eso el MSRV pasa a 1.89); el segundo sale con 75.
- [x] Rotar `history.jsonl`: guarda las últimas 1000 ejecuciones.

## Fase 6 · Distribución (publicada)

- [x] Licencia `MIT OR Apache-2.0` (elegida por Pavel), metadatos de crates.io en `Cargo.toml`, perfil `release` con `strip` y LTO. `cargo publish --dry-run` pasa.
- [x] `scripts/release.sh <versión> [--dry-run] [--crate]`, en local como en hvf: CI, subida de versión, binarios de macOS (arm64 e Intel), Linux musl (x86_64 y arm64, con cargo-zigbuild) y Windows (mingw), archivos con `.sha256`, etiqueta, release de GitHub con notas desde el log, fórmula del tap y, con `--crate`, `cargo publish`. Sin todos los objetivos no publica, porque `self-update` busca el suyo.
- [x] `repoke self-update [--check] [--yes] [--json]`: con Homebrew o cargo lanza `brew upgrade repoke` o `cargo install repoke --locked`; si es un binario, descarga el de su objetivo, comprueba el SHA-256 y lo cambia de forma atómica (con vuelta atrás). `GITHUB_TOKEN` permite probarlo con el repositorio privado.
- [x] Aviso de versión nueva como mucho una vez al día, solo en terminal y nunca con `--json` ni en la tarea programada (`REPOKE_NO_UPDATE_CHECK` lo apaga).
- [x] Dry-run en el Mac: los cinco binarios generados y verificados con `file`, checksum comprobado con `shasum -c` y fórmula pasada por `brew style`.
- [x] zig 0.17 y cargo-zigbuild 0.24 instalados (28-10, con permiso de Pavel): el dry-run genera los cinco objetivos; los de Linux son estáticos.
- [x] Tap público `paveldelpozo/homebrew-tap` creado con la fórmula de v0.1.0 (28-10). Homebrew 7 obliga a `brew trust paveldelpozo/tap` antes de instalar desde un tap de terceros: va en el README y tendrá que ir en la landing.
- [x] Repositorio de repoke **público** (28-10), tras reescribir el historial: commits con `paveldelpozo@gmail.com` y sin la IP del servidor. `self-update` ya lee las releases sin token.
- [x] Commit del tap reescrito con el Gmail y subido (28-10).
- [ ] Probar `brew install` desde el tap una vez hecho `brew trust`: lo hará un compañero de Pavel como beta-tester.
- [x] Primera release `v0.1.0` (28-10, privada): cinco binarios con su `.sha256`. `repoke self-update --check` con `GITHUB_TOKEN` la lee bien y detecta la instalación por cargo.
- [x] Publicado en crates.io: `repoke 0.1.0` (28-10). Las próximas versiones, con `scripts/release.sh --crate`.
- [x] Token de crates.io revocado y sustituido por uno nuevo (Pavel, 28-10).
- [x] Correo de los commits: `paveldelpozo@gmail.com` (elegido por Pavel), configurado en el repo.
- [ ] Más adelante, la fórmula en homebrew-core cuando cumpla sus requisitos de popularidad.

## Fase 7 · Agentes (hecha; falta probarlo en una conversación)

- [x] `repoke upgrade --package nombre,gestor:nombre` para actualizar paquetes sueltos.
- [x] `repoke mcp`: servidor MCP por stdio en el propio binario (sin SDK). Herramientas `list_managers`, `check_updates`, `upgrade` (dry-run por defecto, siempre `--no-sudo`) y `schedule_status`, con anotaciones de solo lectura o destructiva. Negocia las versiones de protocolo 2025-06-18, 2025-03-26 y 2024-11-05. Probado por stdio de principio a fin.
- [x] Plugin de Claude Code en `integrations/claude-code/` (skill `repoke` + `.mcp.json`) y marketplace en `.claude-plugin/marketplace.json`. `claude plugin validate` pasa en los dos. `release.sh` sube también la versión de `plugin.json`.
- [x] README: instalación en Claude Code, Codex y cualquier cliente MCP.
- [x] Plugin activado en todos los perfiles de Claude con `hvf` (`plugin_add`, marketplace `paveldelpozo/repoke`), 28-10.
- [x] Probado en una conversación nueva (28-10): "¿qué tengo desactualizado?" usa `check_updates`.
- [x] Herramientas MCP `export` (sin configuración salvo `include_config`) e `init_preview` (`init --dry-run` desde una foto o del catálogo).

## Fase 8 · Plugins de la comunidad (hecha)

- [x] Plugins ejecutables `repoke-<id>` en el PATH con protocolo JSON versionado (1): `describe`, `detect`, `outdated`, `upgrade` y, opcionales, `refresh`, `installed` e `install`. stdin cerrado, error = salida distinta de 0 con stderr.
- [x] Gestores declarativos en `~/.config/repoke/managers/*.toml`: comandos y lectura de la salida en JSON (punteros JSON) o por líneas (regex con grupos con nombre), con `{packages}`, `{name}` y `{version}`. Las claves desconocidas son error.
- [x] Aparecen en `list`, `check`, `upgrade`, `export`, `init`, `schedule` y el MCP. Un id que choca con uno integrado u otro plugin no se carga y se avisa. `REPOKE_NO_PLUGINS=1` los apaga.
- [x] `repoke plugin list [--json]` y `repoke plugin new <id> [--exec] [-o]` con plantillas que funcionan tal cual.
- [x] Ejemplos en `examples/plugins/`: `gh-extensions.toml` (extensiones de `gh`; Pavel no tiene ninguna, así que está probado solo con tests) y `repoke-omz` (oh-my-zsh, probado de verdad).
- [x] El ejemplo `brew-greedy.toml` se retiró (28-10): `brew outdated --greedy` lista todos los casks, no solo los que se actualizan solos, y duplicaba lo del gestor integrado (terminal-browser salía dos veces). Ahora es la opción `[brew] greedy = true`.
- [x] Pistas para errores conocidos (`hint` en el evento `failed`); la primera, para un cask cuya app se borró a mano.
- [x] Documentación del protocolo y del formato TOML en `docs/plugins.md`, lista para la landing.
- [ ] Índice comunitario de plugins, además del oficial (`plugin search` e `install` ya existen para el oficial; `REPOKE_PLUGIN_INDEX` permite usar otro).
- [x] Tiempo máximo por llamada a un plugin (28-10, a petición de Pavel): 10 minutos por defecto, también en las actualizaciones. `plugin_timeout` en `config.toml` o `REPOKE_PLUGIN_TIMEOUT` (`600`, `90s`, `10m`, `1h`; `0` sin límite). Al pasarse, se corta el proceso y cuenta como fallo de ese plugin.
- [x] Ejemplo instalado en el Mac de Pavel: `~/.local/bin/repoke-omz` (copia). `brew-greedy.toml` sustituido por `[brew] greedy = true` en su `config.toml`.

## Fase 9 · Plugins oficiales (`repoke-plugins`, repositorio aparte)

- [x] Repositorio local `~/Documents/Repositories/repoke-plugins` con `index.toml`, README, licencias y `scripts/check.sh` (comprueba que cada plugin es ejecutable y que `describe` cuadra con el índice).
- [x] `omz`: oh-my-zsh, probado en el Mac de Pavel.
- [x] `vscode` (Python): extensiones con una versión **estable** más nueva en el Marketplace (las pre-release se ignoran y solo se pide el historial completo de las extensiones cuya última versión es pre-release), más `installed` e `install` para `export` e `init`. Probado con las 57 extensiones de Pavel: 4 pendientes, en unos 4 segundos.
- [x] En repoke, `plugin search`, `plugin install <id>` (enseña el código y pregunta; `--yes`, `--force`) y `plugin remove <id>` (solo borra lo que instaló). `REPOKE_PLUGIN_INDEX` para otro índice.
- [x] `paveldelpozo/repoke-plugins` publicado (28-10, lo creó Pavel). Prueba de punta a punta: `plugin search` e `install vscode` desde el índice real de GitHub, y `check` con las 4 extensiones pendientes.
- [ ] tpm, zinit, antidote, lazy.nvim, vim-plug y componentes de gcloud: Pavel no tiene ninguno instalado, así que quedan pendientes hasta poder probarlos en una máquina que los tenga.
- Se quedan en el núcleo, por ser gestores de paquetes: mas, rustup.

## Landing (publicada en https://repoke.paveldelpozo.es)

- [x] Logo del **repóker de ases** (diseño propio a partir de la idea de Pavel): cuatro ases en abanico con su palo en la esquina, colores alternos y el as de corazones delante; favicon con el as de corazones. `site/logo.svg`, `site/favicon.svg`.
- [x] Dirección visual en `site/DESIGN.md`: colores de la baraja (papel, tinta, rojo), minimalista como brew.sh; tema oscuro con el verde del tapete. Contrastes AA comprobados con el script de antislop.
- [x] Web estática en `site/`: instalación (brew con `brew trust`, cargo, binarios), salida real de `repoke list`, una sección por palo, botones de copiar (sin los comentarios `#`) y cambio de tema. Revisada en escritorio y a 320, 390 y 768 px: sin desbordes y con objetivos de 44 px.
- [x] Guía de plugins (`plugins.html`) generada en el build desde `docs/plugins.md` con un conversor propio en Python (`site/render.py`); el clasificador bloquea descargar `marked`.
- [x] Desplegada en Dokploy (proyecto «repoke» → aplicación «Landing», `applicationId` `llp0KBoAGGxkY8k40Q9WG`), desde GitHub con `site/Dockerfile`, autodespliegue al tocar `site/**` o `docs/plugins.md`. Certificado de Let's Encrypt.
- [x] Español e inglés (28-10): `/` y `/es/`, idioma del navegador en la primera visita y menú para cambiarlo (se recuerda). Tema con menú Sistema / Claro / Oscuro, aplicado antes de pintar.
- [x] Todos los comandos documentados: referencia completa en `docs/commands.md` y `docs/es/commands.md` (página *Commands* / *Comandos*), guía de plugins traducida (`docs/es/plugins.md`), y en la portada el modo programado, el MCP y una tabla con todos los comandos.
- [x] `scripts/check-docs.py` en la CI: falla si `repoke --help` tiene un comando u opción que no está en las dos referencias. **Cada función nueva actualiza la landing.**
- [x] Autodespliegue: los `watchPaths` no incluían las referencias nuevas; ampliados a `site/**`, `docs/commands.md`, `docs/plugins.md`, `docs/es/**` y `Cargo.toml`, y desde entonces redespliega solo.
- [x] La landing muestra la última versión publicada (junto a la instalación y en el pie), leída de `Cargo.toml` en el build: cada release la actualiza sin tocar nada (28-10, a petición de Pavel).
- [x] **Landing convertida a Astro** (28-10): proyecto en `site/` (Astro 7, pnpm, Sätteri para el Markdown), las mismas URLs (`build.format: "preserve"`), la referencia y la guía importadas de `docs/`, la versión de `Cargo.toml`. La imagen de Docker se construye con Node 24 y la sirve nginx; la CI la compila. Las anclas de títulos con `<…>` cambian (`#upgrade-name` en vez de `#upgrade-lt-name-gt`); nada enlazaba a ellas. A partir de ahora, cualquier landing nueva se hace en Astro salvo que Pavel diga lo contrario.
- [x] Imagen para compartir en redes (Open Graph): `site/og.html`, renderizada por `site/og.sh` en `og.png` y `og-es.png`.

## Preguntas abiertas

- Decisión abierta: el índice comunitario de plugins (opciones planteadas a Pavel el 28-10 por la noche). Ya decididas: 0.4.0, asdf, cifrado con `age`, deno y volta pendientes, `softwareupdate` añadido.
