# Quick Task 260503-p7f Summary

**Date:** 2026-05-03
**Status:** Complete

## Completed

- Added a root `Makefile` with `make install`.
- The target installs `polint` from `crates/polint-cli` using
  `cargo install --locked --path crates/polint-cli --force`.
- Updated the README local checkout install instructions to prefer
  `make install`.

## Verification

- `make -n install`
- `CARGO_INSTALL_ROOT="$(mktemp -d)" make install`
- temporary installed binary: `polint --version`
- `git diff --check`
