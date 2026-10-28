# Tareas de repoke

Lista viva del proyecto. Se marcan, añaden y anotan aquí las tareas, los hallazgos menores y las preguntas abiertas. Las decisiones y aprendizajes van al cerebro (`Proyectos/repoke`).

## Fase 1 · Esqueleto y consulta (hecha)

- [x] Proyecto Rust con clap y serde, trait `Manager` y un fichero por gestor.
- [x] `repoke list`: gestores soportados en el sistema, ruta y versión.
- [x] `repoke check`: pendientes en paralelo, `--only`, `--exclude`, `--json`, códigos de salida 0/100/1/2.
- [x] Proveedores brew (formulae y casks), npm global y pnpm global, con tests de parseo.
- [x] CI en local con `scripts/ci.sh`: fmt, clippy para macOS, Linux y Windows, tests en el host y en Linux con Docker si está arrancado. El workflow de GitHub queda solo manual (el presupuesto de Actions se agotó el 28-10).
- [ ] Activar el hook de pre-push (`git config core.hooksPath .githooks`): pendiente de que Pavel lo confirme.

## Fase 2 · Actualizar (hecha, falta probar una actualización real)

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
- [ ] Repetir la prueba real tras las correcciones.
- [ ] Antes de instalar un global de npm o pnpm, comparar su `engines.node` con la versión de node en uso y avisar, en vez de dejar que falle.
- [ ] Algunos casks de brew piden contraseña con sudo por su cuenta: el aviso queda tapado por el spinner. Detectar los casks y pedir `sudo -v` antes, o mostrar su salida tal cual.
- [ ] Timeouts por gestor en la configuración.
- [ ] Con `--refresh`, el spinner de npm y pnpm dice "refreshing" un instante aunque no tengan índice que refrescar.

## Fase 3 · Más gestores

- [ ] fnm: nueva LTS de node y, opcionalmente, migrar los globales de la versión anterior.
- [ ] nvm: es una función de shell, se detecta por `$NVM_DIR` o `~/.nvm`, no por el PATH.
- [ ] pip: solo paquetes `--user`. El Python de Homebrew es "externally managed" (PEP 668): avisar en vez de usar `--break-system-packages`.
- [ ] uv (`uv self update`, `uv tool upgrade --all`), pipx.
- [ ] cargo (`cargo install-update` si está), rustup.
- [ ] macOS: mas, `softwareupdate` (solo bajo petición explícita).
- [ ] Linux: apt, dnf, pacman, zypper, snap, flatpak.
- [ ] Windows: winget, scoop, choco.
- [ ] Otros: yarn y bun globales, deno, gem, composer global, volta, mise, asdf.

## Fase 4 · `init` y `export`

- [ ] `repoke init` interactivo: elegir de un catálogo qué gestores y paquetes instalar en un equipo nuevo, incluida la instalación de los propios gestores (brew, fnm…).
- [ ] `repoke export`: foto del equipo actual a `repoke.toml`.
- [ ] `repoke init --from repoke.toml --yes`: reproducir esa foto sin preguntas.
- [ ] Configuración en el backup: `export` pregunta gestor por gestor si incluir su configuración y enseña qué ficheros son. Cada gestor declara los suyos (npm `~/.npmrc`, pip `pip.conf`, zsh `~/.zshrc`, oh-my-zsh `custom/`…). Formato: directorio o `.tar.gz` con `repoke.toml` y `files/`, rutas relativas a `~`.
- [ ] Secretos: por defecto no se incluye ninguna configuración. Buscar patrones (`_authToken`, `password`, `token`, claves privadas) y ofrecer quitar el valor, excluir el fichero o cifrar el backup con contraseña.
- [ ] Restaurar sin pisar: `init --from` enseña qué ficheros ya existen y en qué cambian, pregunta antes de sobrescribir y guarda una copia `.bak` del original.
- [ ] Dotfiles: detectar los enlaces simbólicos que apuntan a un repositorio de dotfiles (Pavel tiene `~/.oh-my-zsh -> ~/.dotfiles/...`) y apuntarlos como "viene de ahí" en vez de copiarlos.
- [ ] Cambio de sistema operativo: saltar y avisar de los gestores que no existen en el destino.

## Fase 5 · Modo programado

- [ ] `repoke schedule install --every daily|weekly [--only …]`: registra una ejecución periódica de `repoke upgrade --yes --no-sudo` en el planificador nativo (launchd en macOS, timer de systemd de usuario en Linux, Programador de tareas en Windows) en vez de tocar el crontab.
- [ ] `repoke schedule status` y `repoke schedule remove`.
- [ ] Log de cada ejecución en `~/.local/state/repoke/` (o equivalente por SO) y notificación del sistema con el resumen.
- [ ] Decidir si por defecto solo consulta y notifica, o si actualiza directamente.

## Fase 6 · Distribución

- [ ] Releases con binarios para macOS (arm64/x86_64), Linux y Windows.
- [ ] Tap de Homebrew propio.
- [ ] Publicar en crates.io (`cargo install repoke`).
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
- [ ] Apartado de documentación para plugins (ver fase 8).
- [ ] Desplegar en el Dokploy personal; antes, pedir el dominio y comprobar con `dig` que apunta al servidor.

## Preguntas abiertas

- Mientras el repositorio sea privado, la gente no puede abrir issues. Opción: un repositorio público aparte solo para issues y releases hasta abrir el código.
