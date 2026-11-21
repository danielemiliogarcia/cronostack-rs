# Releasing CronoStack

A release is published by pushing a **YubiKey-signed, annotated version tag**
(e.g. `0.1.0`) that points to a commit on `main`. The
[release workflow](.github/workflows/release.yml) then:

1. Verifies the tag's OpenPGP signature against the allowlisted key in
   `.github/release-signing-key.asc`, and checks that the tagged commit is
   reachable from `main` (`scripts/verify-release-tag.sh`).
2. Checks that the version metadata agrees (`scripts/verify-release-metadata.sh`).
3. Runs the tests, lints and format check.
4. Builds each target natively, runs the command-line and GUI smoke tests,
   packages the archive and tests the installers.
5. Publishes the GitHub Release only if every target succeeds. Each published
   file gets a GitHub build-provenance attestation.
6. Downloads and runs the published installer on every platform.

## Targets

| Runner | Rust target | Archive |
|---|---|---|
| Ubuntu 24.04 x86-64 | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Ubuntu 24.04 ARM64 | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| macOS 15 Apple silicon | `aarch64-apple-darwin` | `.tar.gz` |
| Windows 2025 x86-64 | `x86_64-pc-windows-msvc` | `.zip` |

- The Windows binary links its C runtime statically, so no Visual C++
  redistributable is needed.
- macOS notarization and Windows Authenticode signing are not configured yet.

Each archive contains the binary, `README.md`, `CHANGELOG.md`, `LICENSE`,
the license files of every bundled dependency and of the
display font (`THIRD_PARTY_LICENSES/`), and a `RELEASE-MANIFEST.txt` with the version,
target, compiler and source commit.

The web app isn't part of a release: the [Web app workflow](.github/workflows/pages.yml)
publishes it to GitHub Pages on every push to `main`.

## Compiler and dependencies

- `RUST_RELEASE_VERSION` and `rust-toolchain.toml` pin the exact compiler
  (currently 1.100.0); `scripts/verify-release-metadata.sh` checks they match.
- `.cargo/config.toml` only accepts crate versions published at least 14 days
  ago.
- Builds use `--locked` against the committed `Cargo.lock`.

## Prepare a release

1. Update the version in `Cargo.toml` (and `Cargo.lock`, via `cargo check`)
   and in `VERSION`.
2. Move the notes from `Unreleased` to a `## [VERSION] - YYYY-MM-DD` heading
   in `CHANGELOG.md`.
3. Run the local checks:

   ```bash
   ./scripts/verify-release-metadata.sh
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets --locked
   ```

4. Merge the release pull request into `main`.

## Tag and publish

```bash
git switch main
git pull --ff-only
version=$(tr -d '[:space:]' < VERSION)
git tag -s "$version" -m "CronoStack $version"   # touch the YubiKey
git tag -v "$version"
git push origin "$version"
```

Watch the **Release** workflow. If it fails before publishing, fix the problem
on `main`, delete the tag (`git push origin :refs/tags/$version` and
`git tag -d $version`), and tag again.

## Rotating the signing key

Update `.github/release-signing-key.asc` and both fingerprints in
`scripts/verify-release-tag.sh` together, in a reviewed pull request, before
the first tag signed with the new key.
