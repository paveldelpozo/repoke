"""Builds the site into a folder (default site/dist): English at /, Spanish at /es/.

The landing pages come from site/pages/<lang>/index.html; the command reference and the plugin
guide are rendered from docs/ (the Spanish ones from docs/es/), so the site never drifts from the
repository. site/list-output.txt is a real `repoke list` run, shown on both landing pages, and the
version shown comes from Cargo.toml.
"""

import html
import json
import shutil
import sys
import tomllib
from pathlib import Path

from render import render

SITE = Path(__file__).resolve().parent
ROOT = SITE.parent
BASE = {"en": "/", "es": "/es/"}
DOCS = {"en": ROOT / "docs", "es": ROOT / "docs" / "es"}
ASSETS = ["style.css", "site.js", "logo.svg", "favicon.svg"]

# The suits mark the four sections of the landing page, as they mark the four aces in the logo.
SUITS = {
    "spade": ("ink", "M12 2.5 L19.2 10 C21.6 12.4 21.3 16.2 18.6 17.9 C16.7 19.1 14.4 18.8 12.9 17.3 L14.2 21.5 H9.8 L11.1 17.3 C9.6 18.8 7.3 19.1 5.4 17.9 C2.7 16.2 2.4 12.4 4.8 10 Z"),
    "heart": ("red", "M12 21 L4.6 13.4 C2.2 11 2.5 7.1 5.2 5.3 C7.5 3.8 10.4 4.4 12 6.6 C13.6 4.4 16.5 3.8 18.8 5.3 C21.5 7.1 21.8 11 19.4 13.4 Z"),
    "club": ("ink", "M12 3.2 A3.9 3.9 0 0 1 15.6 8.6 A3.9 3.9 0 1 1 13.1 15.6 L14.3 21.5 H9.7 L10.9 15.6 A3.9 3.9 0 1 1 8.4 8.6 A3.9 3.9 0 0 1 12 3.2 Z"),
    "diamond": ("red", "M12 2.5 L19 12 L12 21.5 L5 12 Z"),
}


def suit(name):
    color, path = SUITS[name]
    return f'<svg class="suit {color}" viewBox="0 0 24 24" aria-hidden="true"><path d="{path}"/></svg>'


def fill(template, values):
    for key, value in values.items():
        template = template.replace("{{" + key + "}}", value)
    return template


def page(layout, strings, lang, name, content, main_class=""):
    s = strings[lang]
    title, description = s["pages"][name]
    file = "" if name == "index" else f"{name}.html"
    current = file or "index"

    def link(href, label):
        if not href.startswith("http"):
            href = BASE[lang] + href
        mark = ' aria-current="page"' if href.endswith(current + ".html") else ""
        return f'      <a href="{href}"{mark}>{label}</a>'

    values = {key: html.escape(value) for key, value in s.items() if isinstance(value, str)}
    values.update(
        lang=lang,
        title=html.escape(title),
        description=html.escape(description),
        url_en=BASE["en"] + file,
        url_es=BASE["es"] + file,
        selected_en=' selected' if lang == "en" else "",
        selected_es=' selected' if lang == "es" else "",
        home=BASE[lang],
        nav="\n".join(link(href, label) for href, label in s["nav"]),
        main_class=main_class,
        content=content,
    )
    return fill(layout, values)


def build(out):
    # Cargo.toml changes only in a release commit, so its version is the latest one published.
    with open(ROOT / "Cargo.toml", "rb") as cargo:
        version = tomllib.load(cargo)["package"]["version"]
    layout = (SITE / "layout.html").read_text().replace("{{version}}", version)
    strings = json.loads((SITE / "strings.json").read_text())
    listing = html.escape((SITE / "list-output.txt").read_text().rstrip("\n"))
    out.mkdir(parents=True, exist_ok=True)
    for asset in ASSETS:
        shutil.copy(SITE / asset, out / asset)

    for lang in ("en", "es"):
        folder = out / BASE[lang].strip("/")
        folder.mkdir(parents=True, exist_ok=True)
        landing = fill(
            (SITE / "pages" / lang / "index.html").read_text(),
            {**{n: suit(n) for n in SUITS}, "list_output": listing, "version": version},
        )
        (folder / "index.html").write_text(page(layout, strings, lang, "index", landing))
        for name in ("commands", "plugins"):
            source = DOCS[lang] / f"{name}.md"
            relative = source.relative_to(ROOT).as_posix()
            note = (
                f'    <p class="note">{strings[lang]["built_from"]} '
                f'<a href="https://github.com/paveldelpozo/repoke/blob/main/{relative}">{relative}</a>.</p>'
            )
            content = render(source.read_text()) + "\n" + note
            (folder / f"{name}.html").write_text(
                page(layout, strings, lang, name, content, ' class="doc"')
            )


if __name__ == "__main__":
    build(Path(sys.argv[1]) if len(sys.argv) > 1 else SITE / "dist")
