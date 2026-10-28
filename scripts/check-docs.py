"""Fails when a command or an option in `repoke --help` is missing from the command reference, in
English or Spanish (docs/commands.md, docs/es/commands.md), so the site follows every new feature.

Usage: python3 scripts/check-docs.py <path to the repoke binary>
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = [ROOT / "docs" / "commands.md", ROOT / "docs" / "es" / "commands.md"]


def help_of(binary, path):
    return subprocess.run([binary, *path, "--help"], capture_output=True, text=True, check=True).stdout


def walk(binary, path=()):
    """Every command path with its long options, from the help output."""
    text = help_of(binary, path)
    options = {o for o in re.findall(r"--([a-z][a-z-]*)", text) if o not in ("help", "version")}
    yield path, options
    commands = re.search(r"^Commands:\n((?:  .+\n)+)", text, re.M)
    if commands:
        for line in commands.group(1).splitlines():
            name = line.split()[0]
            if name != "help":
                yield from walk(binary, (*path, name))


def main():
    binary = sys.argv[1]
    missing = []
    for doc in DOCS:
        text = doc.read_text()
        for path, options in walk(binary):
            if not path:
                continue
            command = "repoke " + " ".join(path)
            if command not in text:
                missing.append(f"{doc.relative_to(ROOT)}: `{command}`")
            missing += [f"{doc.relative_to(ROOT)}: `{command} --{o}`" for o in sorted(options) if f"--{o}" not in text]
    for line in missing:
        print(f"not documented: {line}")
    sys.exit(1 if missing else 0)


if __name__ == "__main__":
    main()
