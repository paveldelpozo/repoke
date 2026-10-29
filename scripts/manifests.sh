#!/usr/bin/env bash
# Writes the package manifests for a release from the archives' sums in dist/: the Homebrew
# formula (dist/repoke.rb), the Scoop manifest (dist/repoke.json) and the winget manifests
# (dist/winget/). scripts/release.sh calls it; run it alone to redo them for a published version.
# Usage: scripts/manifests.sh X.Y.Z
set -euo pipefail
cd "$(dirname "$0")/.."

version=${1:?usage: scripts/manifests.sh X.Y.Z}
repo="paveldelpozo/repoke"
url="https://github.com/$repo/releases/download/v$version"
description="Finds the package managers on your system and keeps them up to date"

sha() { cut -d' ' -f1 "dist/repoke-$1.sha256" 2>/dev/null || echo "MISSING"; }

cat > dist/repoke.rb <<EOF
# frozen_string_literal: true

# Finds the package managers on a system and keeps them up to date.
class Repoke < Formula
  desc "$description"
  homepage "https://github.com/$repo"
  version "$version"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "$url/repoke-aarch64-apple-darwin.tar.gz"
      sha256 "$(sha aarch64-apple-darwin.tar.gz)"
    end
    on_intel do
      url "$url/repoke-x86_64-apple-darwin.tar.gz"
      sha256 "$(sha x86_64-apple-darwin.tar.gz)"
    end
  end

  on_linux do
    on_arm do
      url "$url/repoke-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$(sha aarch64-unknown-linux-musl.tar.gz)"
    end
    on_intel do
      url "$url/repoke-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$(sha x86_64-unknown-linux-musl.tar.gz)"
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

windows=x86_64-pc-windows-gnu.zip
windows_sha=$(sha "$windows")

# Scoop reads `|` in a license as "or". `autoupdate` lets `checkver` bump it without this script.
cat > dist/repoke.json <<EOF
{
    "version": "$version",
    "description": "$description",
    "homepage": "https://repoke.paveldelpozo.es",
    "license": "MIT|Apache-2.0",
    "architecture": {
        "64bit": {
            "url": "$url/repoke-$windows",
            "hash": "$windows_sha"
        }
    },
    "bin": "repoke.exe",
    "checkver": "github",
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/$repo/releases/download/v\$version/repoke-$windows",
                "hash": {
                    "url": "\$url.sha256"
                }
            }
        }
    }
}
EOF

# winget-pkgs wants one folder per version: manifests/p/PavelDelPozo/repoke/<version>/.
id=PavelDelPozo.repoke
schema=1.9.0
rm -rf dist/winget
mkdir -p dist/winget
cat > "dist/winget/$id.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.$schema.schema.json
PackageIdentifier: $id
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $schema
EOF
cat > "dist/winget/$id.installer.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.$schema.schema.json
PackageIdentifier: $id
PackageVersion: $version
InstallerType: zip
NestedInstallerType: portable
NestedInstallerFiles:
- RelativeFilePath: repoke.exe
  PortableCommandAlias: repoke
Installers:
- Architecture: x64
  InstallerUrl: $url/repoke-$windows
  InstallerSha256: $(tr '[:lower:]' '[:upper:]' <<<"$windows_sha")
ManifestType: installer
ManifestVersion: $schema
EOF
cat > "dist/winget/$id.locale.en-US.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.$schema.schema.json
PackageIdentifier: $id
PackageVersion: $version
PackageLocale: en-US
Publisher: Pavel del Pozo
PublisherUrl: https://github.com/paveldelpozo
PackageName: repoke
PackageUrl: https://repoke.paveldelpozo.es
License: MIT OR Apache-2.0
LicenseUrl: https://github.com/$repo#license
ShortDescription: $description.
Description: repoke finds Homebrew, npm, pip, cargo, winget, Scoop, Chocolatey and the rest of the package managers you have, shows what is outdated and upgrades what you pick, interactively or unattended.
Moniker: repoke
Tags:
- package-manager
- updates
- winget
- scoop
- chocolatey
- cli
ReleaseNotesUrl: https://github.com/$repo/releases/tag/v$version
ManifestType: defaultLocale
ManifestVersion: $schema
EOF
