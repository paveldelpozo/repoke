# Tareas de repoke

Lista viva del proyecto. Se marcan, añaden y anotan aquí las tareas, los hallazgos menores y las preguntas abiertas. Las decisiones y aprendizajes van al cerebro (`Proyectos/repoke`).

## Fase 1 · Esqueleto y consulta (hecha)

- [x] Proyecto Rust con clap y serde, trait `Manager` y un fichero por gestor.
- [x] `repoke list`: gestores soportados en el sistema, ruta y versión.
- [x] `repoke check`: pendientes en paralelo, `--only`, `--exclude`, `--json`, códigos de salida 0/100/1/2.
- [x] Proveedores brew (formulae y casks), npm global y pnpm global, con tests de parseo.
- [x] CI en GitHub Actions (fmt, clippy, test en macOS, Linux y Windows).

## Fase 2 · Actualizar

- [ ] `repoke upgrade` interactivo: lista para marcar por gestor y por paquete, opción "todo".
- [ ] Modo automático: `--yes`, `--dry-run`, `--only`, `--exclude`, salida JSON por líneas (un evento por paso).
- [ ] Sudo: detectar qué gestores lo necesitan; `--no-sudo` los salta en vez de bloquearse.
- [ ] Resumen final: qué se actualizó, qué falló y por qué.
- [ ] Fichero de configuración `~/.config/repoke/config.toml`: selección por defecto, paquetes fijados o excluidos, timeouts.
- [ ] `brew update` antes de consultar, opcional (`--refresh`), porque `brew outdated` usa el índice local.

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

## Fase 7 · Agentes

- [ ] Skill de Claude Code que use `repoke check --json` y `repoke upgrade --yes --json`.
- [ ] Valorar un servidor MCP.

## Landing

- [ ] Landing del proyecto. El icono parte de la idea de un **repóker de ases** (juego de palabras con el nombre). El logo final hay que pedírselo a Pavel o confirmarlo con él antes de usarlo.
- [ ] Desplegar en el Dokploy personal; antes, pedir el dominio y comprobar con `dig` que apunta al servidor.

## Preguntas abiertas

- Mientras el repositorio sea privado, la gente no puede abrir issues. Opción: un repositorio público aparte solo para issues y releases hasta abrir el código.
