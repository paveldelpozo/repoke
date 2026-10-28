# Tareas de repoke

Lista viva del proyecto. Se marcan, añaden y anotan aquí las tareas, los hallazgos menores y las preguntas abiertas. Las decisiones y aprendizajes van al cerebro (`Proyectos/repoke`).

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
- [ ] Antes de instalar un global de npm o pnpm, comparar su `engines.node` con la versión de node en uso y avisar, en vez de dejar que falle.
- [ ] Algunos casks de brew piden contraseña con sudo por su cuenta: el aviso queda tapado por el spinner. Detectar los casks y pedir `sudo -v` antes, o mostrar su salida tal cual.
- [x] Salida agrupada por gestor: una línea por paquete con ✓/✗, versiones y tiempo; los fallos enseñan el comando y las últimas 10 líneas. Tabla resumen final por gestor con totales y duración (28-10, a petición de Pavel). Respeta `NO_COLOR`.
- [x] composer: `global require` con `--with-all-dependencies`; sin él, laravel/installer 5.32 falló porque `laravel/prompts` estaba fijado en el lock.
- [ ] Timeouts por gestor en la configuración.
- [ ] Con `--refresh`, el spinner de npm y pnpm dice "refreshing" un instante aunque no tengan índice que refrescar.

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
- [ ] Pendientes: yarn global (solo yarn 1), bun, deno, volta, asdf, `softwareupdate` de macOS (solo bajo petición explícita).
- [ ] cargo consulta crates.io crate a crate; con muchos crates instalados será lento. Valorar la API de crates.io en paralelo o `cargo install-update` si está.
- [ ] fnm y nvm: las versiones viejas se quedan instaladas. Ofrecer limpiarlas cuando la nueva funcione.

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
- [ ] Cifrar el backup con contraseña como alternativa a quitar los secretos.
- [ ] Node en el catálogo (`fnm install --lts`): el formato de catálogo aún no tiene cómo expresarlo.
- [ ] Configuraciones que no son de un gestor (`~/.gitconfig`, `~/.zshrc`): vendrán con los plugins oficiales (fase 9).
- [ ] Al restaurar en otra máquina, avisar de los paquetes que no existen allí (p. ej. taps privados) en vez de dejar que falle brew.

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
- [ ] Evitar dos ejecuciones a la vez (un fichero de bloqueo).
- [ ] Rotar `history.jsonl` cuando crezca.

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
- [ ] Probar `brew install` desde el tap una vez hecho `brew trust` (en el Mac de Pavel convive con la versión de cargo).
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
- [ ] Probarlo en una conversación nueva: pedir "¿qué tengo desactualizado?" y ver que usa `check_updates`.
- [ ] Más herramientas MCP si hacen falta: `export`, `init --dry-run`.

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
- [ ] Averiguar por qué Dokploy no redespliega solo al hacer push (el despliegue del 28-10 hubo que lanzarlo a mano por la API).
- [ ] **Paso final: convertir la landing a Astro** (Pavel, 28-10). A partir de ahora, cualquier landing nueva se hace en Astro salvo que él diga lo contrario.
- [ ] Imagen para compartir en redes (Open Graph) a partir del logo.

## Preguntas abiertas

- Ninguna por ahora. (La de recibir issues con el repositorio privado se resolvió al hacerlo público el 28-10.)
