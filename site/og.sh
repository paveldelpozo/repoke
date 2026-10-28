#!/bin/sh
# Renders site/og.html into og.png (English) and og-es.png with headless Chrome. The PNGs are
# committed, so only rerun this when the logo or the wording changes.
set -eu
cd "$(dirname "$0")"
chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
for lang in en es; do
  out=og.png
  [ "$lang" = es ] && out=og-es.png
  "$chrome" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size=1200,630 --screenshot="$PWD/$out" "file://$PWD/og.html#$lang" 2>/dev/null
done
