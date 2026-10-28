# Tareas de repoke

Lista viva del proyecto. Se marcan, añaden y anotan aquí las tareas, los hallazgos menores y las preguntas abiertas. Las decisiones y aprendizajes van al cerebro (`Proyectos/repoke`).

## Fase 1 · Esqueleto y consulta (hecha)

- [x] Proyecto Rust con clap y serde, trait `Manager` y un fichero por gestor.
- [x] `repoke list`: gestores soportados en el sistema, ruta y versión.
- [x] `repoke check`: pendientes en paralelo, `--only`, `--exclude`, `--json`, códigos de salida 0/100/1/2.
- [x] Proveedores brew (formulae y casks), npm global y pnpm global, con tests de parseo.
- [x] CI en local con `scripts/ci.sh`: fmt, clippy para macOS, Linux y Windows, tests en el host y en Linux con Docker si está arrancado. El workflow de GitHub queda solo manual (el presupuesto de Actions se agotó el 28-10).
- [ ] Activar el hook de pre-push (`git config core.hooksPath .githooks`): pendiente de que Pavel lo confirme.

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

## Fase 5 · Modo programado

- [ ] `repoke schedule install --every daily|weekly [--only …]`: registra una ejecución periódica de `repoke upgrade --yes --no-sudo` en el planificador nativo (launchd en macOS, timer de systemd de usuario en Linux, Programador de tareas en Windows) en vez de tocar el crontab.
- [ ] `repoke schedule status` y `repoke schedule remove`.
- [ ] Log de cada ejecución en `~/.local/state/repoke/` (o equivalente por SO) y notificación del sistema con el resumen.
- [ ] Decidir si por defecto solo consulta y notifica, o si actualiza directamente.

## Fase 6 · Distribución

- [ ] Releases con binarios para macOS (arm64/x86_64), Linux y Windows.
- [ ] Release final publicada en **crates.io** (`cargo install repoke`) y en **Homebrew**: primero un tap propio (`brew install paveldelpozo/tap/repoke`) y, cuando cumpla los requisitos de popularidad, la fórmula en homebrew-core.
- [ ] Automatizar en `scripts/release.sh` la publicación en los dos sitios (`cargo publish` y actualizar la fórmula del tap con la URL y el sha256 nuevos).
- [ ] Elegir licencia antes de abrir el repositorio.
- [ ] `repoke self-update`, cuando el repositorio sea público (`upgrade` ya es el de los paquetes). Según cómo se instaló:
  - Con Homebrew o con cargo: lo deja en manos de ese gestor (`brew upgrade repoke`, `cargo install repoke`), para no desincronizarlo. Con brew, además, `repoke upgrade` ya lo incluye como un paquete más.
  - Binario descargado: busca la última release en GitHub, comprueba el checksum (y la firma, si se firman las releases) y sustituye el binario de forma atómica, con vuelta atrás si falla.
  - `--check` solo informa y sale con 100 si hay versión nueva, igual que `check`. `--json` para agentes.
- [ ] Aviso discreto de versión nueva al usar repoke, como mucho una vez al día y nunca con `--json` ni fuera de una terminal.

## Fase 7 · Agentes

- [ ] Skill de Claude Code que use `repoke check --json` y `repoke upgrade --yes --json`.
- [ ] Valorar un servidor MCP.

## Fase 8 · Plugins de la comunidad

Cuando todo lo anterior funcione. Que cualquiera pueda añadir un gestor sin tocar el código de repoke.

- [ ] Plugins ejecutables: un programa `repoke-<id>` en el PATH (como los subcomandos de git o cargo) que responde a `detect`, `outdated` y `upgrade` con un protocolo JSON versionado sobre stdin/stdout. Sirve cualquier lenguaje.
- [ ] Gestores declarativos: un `.toml` en `~/.config/repoke/managers/` que describe los comandos y cómo leer su salida (JSON o expresión regular), para los casos que no necesitan código.
- [ ] `repoke plugin list`, y que los plugins salgan en `list`, `check` y `upgrade` como un gestor más.
- [ ] Repositorio o plantilla de ejemplo para escribir un plugin, con tests del protocolo.
- [ ] Documentación en la landing: cómo funciona el protocolo y cómo hacer, probar y publicar un plugin.
- [ ] Decidir cómo se descubren y comparten (¿un índice comunitario?) y qué avisos de seguridad mostrar, porque un plugin ejecuta código en la máquina.

## Fase 9 · Plugins oficiales (`repoke-plugins`, repositorio aparte)

Herramientas que no son gestores de paquetes pero se actualizan solas. Son los primeros plugins que se escriben, para poner a prueba el protocolo de la fase 8 antes de abrirlo a la comunidad. La idea es detectar lo máximo posible de lo que hay instalado.

- [ ] oh-my-zsh: se detecta por `$ZSH` o `~/.oh-my-zsh`; consulta con `git fetch` + comparar con el remoto, actualiza con `omz update`.
- [ ] zinit, antidote y otros gestores de plugins de zsh.
- [ ] tpm (plugins de tmux), plugins de Neovim (lazy.nvim, vim-plug).
- [ ] Extensiones de VS Code (`code --update-extensions`).
- [ ] Componentes de gcloud, Starship.
- [ ] zsh: si viene de brew, ya lo cubre brew; el de macOS (`/bin/zsh`) se actualiza con el sistema, así que solo se informa.
- Se quedan en el núcleo, por ser gestores de paquetes: mas, rustup.

## Landing

- [ ] Landing del proyecto. El icono parte de la idea de un **repóker de ases** (juego de palabras con el nombre). El logo final hay que pedírselo a Pavel o confirmarlo con él antes de usarlo.
- [ ] Sección de instalación con todas las opciones: `brew install`, `cargo install` y binario descargado de las releases de GitHub.
- [ ] Apartado de documentación para plugins (ver fase 8).
- [ ] Desplegar en el Dokploy personal; antes, pedir el dominio y comprobar con `dig` que apunta al servidor.

## Preguntas abiertas

- Mientras el repositorio sea privado, la gente no puede abrir issues. Opción: un repositorio público aparte solo para issues y releases hasta abrir el código.
