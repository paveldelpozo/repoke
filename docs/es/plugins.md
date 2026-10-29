# Cómo escribir un plugin de repoke

repoke conoce 26 gestores de paquetes de serie. Un plugin añade otro, y a partir de ahí participa en todo: `list`, `check`, `upgrade`, `export`, `init`, `schedule` y el servidor MCP. Hay dos tipos:

- **Un fichero TOML** que dice qué comandos ejecutar y cómo leer su salida. Basta para casi cualquier herramienta capaz de decir qué tiene desactualizado.
- **Un ejecutable** llamado `repoke-<id>`, en el lenguaje que quieras, que responde a un protocolo JSON pequeño. Para herramientas que necesitan lógica, como un clon de git o varios comandos por consulta.

Los plugins oficiales están en [repoke-plugins](https://github.com/paveldelpozo/repoke-plugins): `repoke plugin search` los lista y `repoke plugin install <id>` enseña el código de uno, pregunta y lo copia en `~/.local/bin` o en la carpeta de gestores. Cada fichero está fijado en el índice por el sha256 de la versión revisada, e `install` rechaza uno que no coincida. `REPOKE_PLUGIN_INDEX` hace que esos comandos usen otro índice, una URL o un fichero local; uno local, útil mientras escribes un plugin, puede no llevar las sumas.

Los plugins de la comunidad están en el mismo índice, en `community/`, y `repoke plugin search` los lista después de los oficiales, con su autor, sus descargas (↓) y sus votos (▲). Cada uno entra con un pull request que se revisa antes de listarlo; después, la suma fija esa versión. Para votar un plugin se vota a favor su Discussion, donde los comentarios hacen de opiniones.

`repoke plugin new <id>` escribe una plantilla TOML para empezar, y `repoke plugin new <id> --exec`, un script de shell. `repoke plugin list` muestra los plugins que repoke ha encontrado y cualquier problema que tengan. Hay dos ejemplos en [`examples/plugins`](../../examples/plugins): las extensiones de GitHub CLI (TOML) y oh-my-zsh (ejecutable).

El id de un plugin no puede coincidir con el de un gestor integrado ni con el de otro plugin; `plugin list` avisa del choque y el plugin no se carga. `REPOKE_NO_PLUGINS=1` desactiva todos los plugins durante una ejecución.

Los plugins se ejecutan con los permisos de tu usuario, como cualquier comando de tu PATH. Léelo antes de instalarlo.

Cada comando que repoke lanza para un plugin, actualizaciones incluidas, se detiene a los 10 minutos y cuenta como un fallo de ese plugin. El límite se cambia con `plugin_timeout` en `~/.config/repoke/config.toml` (`600`, `90s`, `10m`, `1h`, o `0` para quitarlo), o para una sola ejecución con `REPOKE_PLUGIN_TIMEOUT`.

## Gestores en TOML

repoke lee todos los `*.toml` de `~/.config/repoke/managers/` (`%APPDATA%\repoke\managers\` en Windows).

```toml
id = "gh-ext"
name = "GitHub CLI extensions"
binary = "gh"               # todos los comandos de abajo ejecutan este programa
# os = ["macos", "linux"]   # opcional; por defecto, todos los sistemas
# needs_root = true         # actualiza con sudo, y --no-sudo lo salta

[outdated]
command = ["extension", "upgrade", "--all", "--dry-run"]
format = "lines"
pattern = '^\[(?P<name>[^\]]+)\]: would have upgraded from (?P<current>\S+) to (?P<latest>\S+)'

[upgrade]
command = ["extension", "upgrade", "{name}"]
```

El gestor cuenta como instalado cuando `binary` está en el PATH, y su versión es la primera palabra con forma de versión que imprime `binary --version`.

### Leer la salida

`format = "json"` lee `items` como una lista JSON y toma `name`, `current` y `latest` de cada elemento. Los números pasan a texto y los campos que faltan quedan vacíos. Para una herramienta cuyo `outdated --json` imprimiera `{"tools": [{"id": "x", "installed": ["1.0"], "available": "1.1"}]}`:

```toml
[outdated]
command = ["outdated", "--json"]
format = "json"
items = "/tools"            # puntero JSON a la lista; "" cuando la salida es la lista
name = "id"                 # un campo de cada elemento, o un puntero JSON dentro de él
current = "/installed/0"
latest = "available"
```

`format = "lines"` aplica una expresión regular a cada línea de la salida y se queda con las que encajan. Los grupos con nombre dan los campos; `name` es obligatorio. El ejemplo de arriba lee líneas como `[gh-dash]: would have upgraded from v4.1.0 to v4.2.0`.

Un comando tiene que salir con 0, salvo que indiques otros códigos válidos en `ok_codes = [0, 1]`; algunas herramientas salen con 1 cuando encuentran algo desactualizado.

### Comandos y marcadores

- `{packages}` como argumento se expande a todos los paquetes que se actualizan, cada uno como un argumento. Un solo comando los actualiza todos.
- Un argumento que contiene `{name}` o `{version}` genera un comando por paquete, con los marcadores sustituidos por su nombre y su versión de destino (`["install", "{name}@{version}"]`). Así, un fallo se atribuye solo a ese paquete.
- Sin marcadores, el comando se ejecuta una vez, se haya elegido lo que se haya elegido (`["self-update"]`).

### Secciones opcionales

| Sección | La usa | Formato |
|---------|--------|---------|
| `[refresh]` | `check --refresh`, `upgrade --refresh` | `command` |
| `[installed]` | `export` | como `[outdated]`, con `name` y `version` |
| `[install]` | `init` | como `[upgrade]`; `{version}` es la versión guardada en la foto |

### Ficheros de configuración

`config` enumera ficheros, relativos a tu carpeta personal, que `repoke export` ofrece guardar con los paquetes del gestor y que `repoke init` vuelve a poner, igual que la configuración de los gestores integrados: los valores que parecen secretos se quitan salvo que los cifres, y los enlaces a un repositorio de dotfiles se guardan como enlaces.

```toml
id = "git"
name = "Git config"
binary = "git"
config = [".gitconfig", ".config/git/config", ".config/git/ignore"]
```

Un gestor puede ser solo eso: sin `[outdated]` ni `[upgrade]` no tiene nada que actualizar, y `export` se lleva sus ficheros. Las rutas tienen que quedarse dentro de la carpeta personal.

Las claves desconocidas son un error, así que una errata aparece en `repoke plugin list` en vez de ignorarse.

## Plugins ejecutables, protocolo 1

Un plugin ejecutable es un programa llamado `repoke-<id>` (`repoke-<id>.exe` o `.cmd` en Windows) en cualquier carpeta de tu PATH. Si hay dos con el mismo nombre, gana el primero del PATH, como con cualquier comando.

repoke lo ejecuta con un subcomando, con la entrada estándar cerrada, y lee la salida estándar. Una salida distinta de 0 es un fallo, y las últimas líneas de stderr son el error que ve el usuario. No esperes nunca una respuesta: no hay nadie para darla.

### `describe`

Se ejecuta cada vez que arranca repoke, así que tiene que ser inmediato: sin red y sin comandos lentos.

```json
{"protocol": 1, "id": "omz", "name": "oh-my-zsh", "os": ["macos", "linux"], "needs_root": false, "capabilities": []}
```

- `protocol`: la versión de este documento que sigue el plugin. repoke rechaza los plugins más nuevos de lo que entiende y pide actualizar repoke.
- `id`: tiene que coincidir con el nombre del fichero, `repoke-<id>`.
- `os`: opcional; los valores son `macos`, `linux` y `windows`. Vacío significa todos.
- `needs_root`: opcional; si es true, `upgrade` se ejecuta con `sudo -n` y `--no-sudo` lo salta.
- `capabilities`: subcomandos opcionales que implementa, entre `refresh`, `installed` e `install`.
- `config`: opcional; ficheros de configuración, relativos a la carpeta personal, que `export` puede llevarse, como en los [gestores en TOML](#ficheros-de-configuración).

### `detect`

Si la herramienta que gestiona el plugin está en esta máquina.

```json
{"installed": true, "version": "7ea697f"}
```

Si `installed` es false, repoke da el gestor por ausente y lo salta.

### `outdated`

Lo que se puede actualizar, como lista. Una lista vacía significa que todo está al día.

```json
[{"name": "oh-my-zsh", "current": "7ea697f", "latest": "17a2b4c"}]
```

### `upgrade <name>...`

Actualiza los paquetes indicados, que siempre son nombres que devolvió `outdated`. Imprime el progreso sobre la marcha: repoke enseña la última línea junto a su indicador. Sal con 0 si todo se actualizó. Si un paquete puede fallar sin arrastrar a los demás, repoke los reintenta de uno en uno tras un lote fallido, así que no pasa nada por hacer fallar la llamada entera.

### `refresh` (capacidad)

Actualiza el índice local antes de `outdated`, para `--refresh`.

### `installed` (capacidad)

Lo que el usuario instaló a propósito, para `repoke export`.

```json
[{"name": "gh-dash", "version": "4.2.0"}]
```

Un elemento puede añadir `"kind"`, un texto con el que tu plugin distingue cosas con el mismo nombre (un origen, un canal). En `install` solo te vuelve el nombre, así que úsalo como información.

### `install <name>...` (capacidad)

Instala esos paquetes en una máquina nueva, para `repoke init`.

### Versiones del protocolo

Lo que las versiones anteriores de repoke pueden ignorar (un campo opcional nuevo, una capacidad nueva) mantiene el protocolo 1. Un cambio que haría que una versión anterior leyera mal un plugin recibe un número de protocolo nuevo, y repoke sigue entendiendo los anteriores.

## Probar un plugin

```sh
repoke plugin list                      # ¿lo encuentra, y es válido?
repoke list                             # ¿está instalado, y en qué versión?
repoke check --only <id>                # qué tiene desactualizado
repoke upgrade --only <id> --dry-run    # los comandos que lanzaría
```

## Compartir un plugin

Abre un pull request en [repoke-plugins](https://github.com/paveldelpozo/repoke-plugins) que añada el plugin como `community/<id>/repoke-<id>` (o `<id>.toml`) y su entrada en `index.toml` con `tier = "community"`, tu usuario de GitHub como `author` y sin `sha256`. Indica en qué máquina y con qué versiones lo has probado. Cuando se revisa, quien lo revisa fija la versión y abre su Discussion; cada cambio posterior vuelve a pasar por revisión.
