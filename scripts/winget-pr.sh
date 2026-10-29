#!/usr/bin/env bash
# Opens the pull request that brings a release to winget: the manifests in dist/winget go to a
# branch of your fork of microsoft/winget-pkgs, through the GitHub API, so the huge repository is
# never cloned. wingetcreate does the same but only runs on Windows. Needs `gh`.
# Usage: scripts/winget-pr.sh X.Y.Z
set -euo pipefail
cd "$(dirname "$0")/.."

version=${1:?usage: scripts/winget-pr.sh X.Y.Z}
upstream=microsoft/winget-pkgs
id=PavelDelPozo.repoke
folder="manifests/p/PavelDelPozo/repoke/$version"
login=$(gh api user -q .login)
fork="$login/winget-pkgs"
branch="repoke-$version"

if ! grep -qs "PackageVersion: $version$" "dist/winget/$id.yaml"; then
  echo "dist/winget doesn't hold the $version manifests: run scripts/manifests.sh $version" >&2
  exit 1
fi

gh repo fork "$upstream" --clone=false >/dev/null 2>&1 || true
gh repo sync "$fork" --branch master >/dev/null
base=$(gh api "repos/$fork/git/ref/heads/master" -q .object.sha)
gh api -X POST "repos/$fork/git/refs" -f ref="refs/heads/$branch" -f sha="$base" >/dev/null

for file in dist/winget/*.yaml; do
  gh api -X PUT "repos/$fork/contents/$folder/$(basename "$file")" \
    -f message="$id $version: $(basename "$file")" -f branch="$branch" \
    -f content="$(base64 <"$file" | tr -d '\n')" >/dev/null
done

# The first submission is a new package; every later one is a new version of it.
if gh api "repos/$upstream/contents/manifests/p/PavelDelPozo/repoke" >/dev/null 2>&1; then
  title="New version: $id version $version"
else
  title="New package: $id version $version"
fi
gh pr create --repo "$upstream" --head "$login:$branch" --base master --title "$title" --body "$(cat <<EOF
$id $version: [repoke](https://github.com/paveldelpozo/repoke) finds the package managers on a system and keeps them up to date. Portable zip (x64) from the GitHub release.

- [ ] Have you signed the [Contributor License Agreement](https://cla.opensource.microsoft.com/microsoft/winget-pkgs)?
- [x] Have you checked that there aren't other open [pull requests](https://github.com/microsoft/winget-pkgs/pulls) for the same manifest update/change?
- [x] This PR only modifies one (1) manifest
- [x] Have you validated your manifest against the [schema](https://github.com/microsoft/winget-cli/tree/master/schemas/JSON/manifests/v1.9.0)?
- [ ] Have you tested your manifest locally with \`winget install --manifest\`? (no Windows machine at hand)
EOF
)"
