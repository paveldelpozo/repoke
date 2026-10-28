#!/bin/sh
# Builds the site into $1 (default site/dist): the static files, plus plugins.html rendered from
# docs/plugins.md so the guide on the site never drifts from the one in the repository.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
out=${1:-$root/site/dist}
mkdir -p "$out"
cp "$root/site/index.html" "$root/site/style.css" "$root/site/site.js" \
  "$root/site/logo.svg" "$root/site/favicon.svg" "$out/"
python3 "$root/site/render.py" "$root/docs/plugins.md" > "$out/guide.html"
awk -v guide="$out/guide.html" '
  /<!-- docs -->/ { while ((getline line < guide) > 0) print line; next }
  { print }
' "$root/site/plugins.html" > "$out/plugins.html"
rm "$out/guide.html"
