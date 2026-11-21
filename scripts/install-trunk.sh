#!/usr/bin/env bash
# Installs the pinned trunk release (the WebAssembly bundler used for the web
# version) into $1 (default: ~/.local/bin), after checking its SHA-256.
# Linux x86-64 only: it's meant for CI.
set -euo pipefail

version=0.22.1
sha256=2ab8d548fd5ed9cda50906f61fd8654204e7893bd52ebbfd90533862b7d71ee1
install_dir=${1:-"$HOME/.local/bin"}

temporary=$(mktemp -d "${TMPDIR:-/tmp}/trunk.XXXXXX")
trap 'rm -rf -- "$temporary"' EXIT

archive="$temporary/trunk.tar.gz"
curl --proto '=https' --tlsv1.2 -fsSL -o "$archive" \
  "https://github.com/trunk-rs/trunk/releases/download/v$version/trunk-x86_64-unknown-linux-gnu.tar.gz"
echo "$sha256  $archive" | sha256sum --check --quiet
tar -xzf "$archive" -C "$temporary" trunk
mkdir -p "$install_dir"
install -m 0755 "$temporary/trunk" "$install_dir/trunk"
"$install_dir/trunk" --version
