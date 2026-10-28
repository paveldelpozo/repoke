#!/usr/bin/env bash
# Local replacement for the GitHub CI, which stays manual to save Actions minutes.
# Usage: scripts/ci.sh [--quick]   (--quick skips the Linux tests in Docker)
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$*"; }

step "fmt"
cargo fmt --check

step "clippy (host)"
cargo clippy --all-targets -- -D warnings

# clippy does not link, so the other OSes' cfg branches compile here without their toolchains.
for target in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc; do
  step "clippy ($target)"
  if rustup target list --installed | grep -qx "$target"; then
    cargo clippy --all-targets --target "$target" -- -D warnings
  else
    echo "skipped: rustup target add $target"
  fi
done

step "test (host)"
cargo test

step "docs cover every command and option"
cargo build --quiet
python3 scripts/check-docs.py target/debug/repoke
python3 site/build.py "$(mktemp -d)"

if [[ "${1:-}" != "--quick" ]]; then
  step "test (linux, docker)"
  if docker info >/dev/null 2>&1; then
    docker run --rm -v "$PWD":/src -w /src -e CARGO_TARGET_DIR=/tmp/target rust:1 cargo test
  else
    echo "skipped: Docker is not running"
  fi
fi

step "ok"
