#!/bin/sh
# Builds the site into $1 (default site/dist); see build.py.
set -eu
exec python3 "$(dirname "$0")/build.py" "$@"
