#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd -P)
version=$(tr -d '[:space:]' < "$root/VERSION")
release_rust=$(tr -d '[:space:]' < "$root/RUST_RELEASE_VERSION")

fail() {
  printf 'release metadata check failed: %s\n' "$*" >&2
  exit 1
}

printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$' \
  || fail "VERSION is not a supported semantic version: $version"
printf '%s\n' "$release_rust" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
  || fail "RUST_RELEASE_VERSION is not an exact stable Rust version: $release_rust"

cargo_version=$(awk '
  /^\[package\]$/ { package = 1; next }
  /^\[/ { package = 0 }
  package && /^version = / { gsub(/^version = "|"$/, ""); print; exit }
' "$root/Cargo.toml")
[[ "$cargo_version" == "$version" ]] \
  || fail "Cargo.toml version $cargo_version does not match VERSION $version"

toolchain=$(awk -F'"' '/^channel = / { print $2; exit }' "$root/rust-toolchain.toml")
[[ "$toolchain" == "$release_rust" ]] \
  || fail "rust-toolchain.toml channel $toolchain does not match RUST_RELEASE_VERSION $release_rust"

grep -Fq "## [$version]" "$root/CHANGELOG.md" \
  || fail "CHANGELOG.md has no release heading for $version"

if [[ ${GITHUB_REF_TYPE:-} == tag && ${GITHUB_REF_NAME:-} != "$version" ]]; then
  fail "release tag ${GITHUB_REF_NAME:-<missing>} does not match $version"
fi

printf 'release metadata: cronostack %s, Rust %s\n' "$version" "$release_rust"
