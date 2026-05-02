# Quick Task 260502-dto Summary

**Task:** Improve examples with real minimal linted code, README coverage, and CLI e2e tests
**Date:** 2026-05-02
**Status:** Complete
**Commit:** this commit

## Changes

- Added runnable `.polint.toml` files and minimal real Go/TS/TSX source fixtures across the checked-in examples.
- Added new focused examples for Go complexity, Go import boundaries, Go test quality, TS complexity, and configured denied literals.
- Updated existing example READMEs so each runnable example documents the exact `polint check --profile fast --format json --fail-on none` command.
- Added a table-driven CLI e2e test that runs every checked-in example directory and asserts its expected rule IDs and source files.
- Updated the main README examples list to match the expanded runnable examples.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p polint-cli checked_in_examples_are_runnable_cli_fixtures -- --nocapture`
- `cargo test -p polint-cli`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
