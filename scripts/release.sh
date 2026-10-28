#!/usr/bin/env bash
# Builds the release archives, then tags, publishes the GitHub release and updates the Homebrew
# tap. Releases are made locally because GitHub Actions minutes are not used for this project.
# Versions follow semver, worked out from Conventional Commits since the last tag: a `!` or a
# `BREAKING CHANGE` bumps the major version, `feat:` the minor one, anything else the patch.
# Usage: scripts/release.sh [auto|major|minor|patch|X.Y.Z] [--dry-run] [--crate]
#   --dry-run  build and package into dist/ only; no version bump, tag, push or upload
#   --crate    also `cargo publish` (once the repository is public)
# Linux builds need `cargo install cargo-zigbuild` and `brew install zig`; Windows needs
# `brew install mingw-w64`.
set -euo pipefail
cd "$(dirname "$0")/.."

bump=auto
dry_run=false
crate=false
for arg in "$@"; do
  case "$arg" in
    --dry-run) dry_run=true ;;
    --crate) crate=true ;;
    -*) echo "unknown option: $arg" >&2; exit 2 ;;
    *) bump=$arg ;;
  esac
done

semver='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
last_tag=$(git describe --tags --abbrev=0 --match 'v[0-9]*' 2>/dev/null || true)
current=${last_tag#v}
current=${current:-0.0.0}
range=${last_tag:+$last_tag..}HEAD
if [[ -z "$(git log --oneline "$range")" && "$bump" == auto ]]; then
  echo "nothing to release: no commits since $last_tag"
  exit 0
fi
if [[ "$bump" == auto ]]; then
  subjects=$(git log --format='%s%n%b' "$range")
  if grep -qE '^[a-z]+(\([^)]*\))?!:|^BREAKING CHANGE' <<<"$subjects"; then
    bump=major
  elif grep -qE '^feat(\([^)]*\))?:' <<<"$subjects"; then
    bump=minor
  else
    bump=patch
  fi
fi
IFS=. read -r major minor patch <<<"${current%%-*}"
case "$bump" in
  major) version="$((major + 1)).0.0" ;;
  minor) version="$major.$((minor + 1)).0" ;;
  patch) version="$major.$minor.$((patch + 1))" ;;
  *)
    [[ "$bump" =~ $semver ]] || { echo "the version is auto, major, minor, patch or like 1.2.3" >&2; exit 2; }
    version=$bump
    ;;
esac
if [[ -n "$last_tag" && "$(printf '%s\n%s\n' "$current" "$version" | sort -V | tail -1)" != "$version" ]] \
  || [[ "$version" == "$current" && -n "$last_tag" ]]; then
  echo "$version is not newer than $current" >&2
  exit 2
fi
tag="v$version"
echo "releasing $tag (from ${last_tag:-the first commit}, bump: $bump)"
tap_dir="${TAP_DIR:-$HOME/Documents/Repositories/homebrew-tap}"
repo="paveldelpozo/repoke"
targets=(
  aarch64-apple-darwin
  x86_64-apple-darwin
  aarch64-unknown-linux-musl
  x86_64-unknown-linux-musl
  x86_64-pc-windows-gnu
)

step() { printf '\n== %s\n' "$*"; }

if ! $dry_run; then
  [[ "$(git branch --show-current)" == main ]] || { echo "release from main" >&2; exit 1; }
  [[ -z "$(git status --porcelain)" ]] || { echo "commit or stash your changes first" >&2; exit 1; }
  if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    echo "$tag already exists" >&2
    exit 1
  fi
fi

step "checks"
scripts/ci.sh --quick

if ! $dry_run; then
  step "version $version"
  # Only the first `version =` line is the package's own.
  perl -0pi -e "s/^version = \"[^\"]*\"/version = \"$version\"/m" Cargo.toml
  cargo update --workspace --quiet
fi

build() {
  local target=$1
  rustup target add "$target" >/dev/null
  case "$target" in
    *-apple-darwin)
      cargo build --release --quiet --target "$target"
      ;;
    *-linux-musl)
      command -v cargo-zigbuild >/dev/null || return 1
      cargo zigbuild --release --quiet --target "$target"
      ;;
    *-windows-gnu)
      command -v x86_64-w64-mingw32-gcc >/dev/null || return 1
      CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
        cargo build --release --quiet --target "$target"
      ;;
  esac
}

rm -rf dist
mkdir -p dist/stage
missing=()
for target in "${targets[@]}"; do
  step "build $target"
  if ! build "$target"; then
    echo "skipped: the cross-compiling tools for $target are not installed (see the top of this script)"
    missing+=("$target")
    continue
  fi
  exe=repoke
  [[ "$target" == *windows* ]] && exe=repoke.exe
  rm -rf dist/stage/*
  cp "target/$target/release/$exe" LICENSE-MIT LICENSE-APACHE README.md dist/stage/
  if [[ "$target" == *windows* ]]; then
    archive="repoke-$target.zip"
    (cd dist/stage && zip -q "../$archive" ./*)
  else
    archive="repoke-$target.tar.gz"
    tar -czf "dist/$archive" -C dist/stage .
  fi
  (cd dist && shasum -a 256 "$archive" > "$archive.sha256")
done
rm -rf dist/stage

if ((${#missing[@]})) && ! $dry_run; then
  echo "not releasing without every target (self-update looks for each one): ${missing[*]}" >&2
  git checkout -- Cargo.toml Cargo.lock
  exit 1
fi

sha() { cut -d' ' -f1 "dist/repoke-$1.tar.gz.sha256" 2>/dev/null || echo "MISSING"; }
url="https://github.com/$repo/releases/download/$tag"
cat > dist/repoke.rb <<EOF
# frozen_string_literal: true

# Finds the package managers on a system and keeps them up to date.
class Repoke < Formula
  desc "Finds the package managers on your system and keeps them up to date"
  homepage "https://github.com/$repo"
  version "$version"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "$url/repoke-aarch64-apple-darwin.tar.gz"
      sha256 "$(sha aarch64-apple-darwin)"
    end
    on_intel do
      url "$url/repoke-x86_64-apple-darwin.tar.gz"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    on_arm do
      url "$url/repoke-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$(sha aarch64-unknown-linux-musl)"
    end
    on_intel do
      url "$url/repoke-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$(sha x86_64-unknown-linux-musl)"
    end
  end

  def install
    bin.install "repoke"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/repoke --version")
  end
end
EOF

if $dry_run; then
  step "dry run, nothing published"
  ls -1 dist
  ((${#missing[@]})) && echo "not built: ${missing[*]}"
  exit 0
fi

step "tag $tag"
# Nothing to commit when the version was already set by hand.
git diff --quiet || git commit --quiet -am "chore(release): $tag"
git tag -a "$tag" -m "$tag"
git push --quiet origin main "$tag"

step "GitHub release"
# Notes grouped by commit type; commits from before Conventional Commits land in "Other changes".
log=$(git log --pretty='%s' "${last_tag:+$last_tag..}$tag^")
section() {
  local title=$1 pattern=$2 lines
  lines=$(grep -E "$pattern" <<<"$log" | sed -E 's/^[a-z]+(\([^)]*\))?!?: /- /' || true)
  [[ -n "$lines" ]] && printf '## %s\n\n%s\n\n' "$title" "$lines"
  return 0
}
notes=$(
  section "Breaking changes" '^[a-z]+(\([^)]*\))?!:'
  section "Features" '^feat(\([^)]*\))?:'
  section "Fixes" '^fix(\([^)]*\))?:'
  section "Other changes" '^(chore|docs|refactor|perf|test|build|ci|style)(\([^)]*\))?:|^[^a-z]|^[a-z]+[^a-z(!:]'
)
gh release create "$tag" dist/*.tar.gz dist/*.zip dist/*.sha256 --repo "$repo" --title "$tag" --notes "$notes"

step "Homebrew tap"
if [[ -d "$tap_dir/.git" ]]; then
  mkdir -p "$tap_dir/Formula"
  cp dist/repoke.rb "$tap_dir/Formula/repoke.rb"
  git -C "$tap_dir" add Formula/repoke.rb
  git -C "$tap_dir" commit --quiet -m "repoke $version"
  git -C "$tap_dir" push --quiet
else
  echo "no tap at $tap_dir (set TAP_DIR); the formula is in dist/repoke.rb"
fi

if $crate; then
  step "crates.io"
  cargo publish
fi

step "released $tag"
