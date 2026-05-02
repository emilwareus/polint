# Quick Task 260502-qsd Summary

**Date:** 2026-05-02
**Status:** Complete

## Completed

- Removed the shared `examples/rules` policy pack.
- Added `polint-runner` as rule-host infrastructure with no built-in policy rules.
- Moved example policies into one local crate per example under `.polint/rules/`.
- Updated example configs and READMEs so each example behaves like a small standalone repository.
- Updated CLI e2e coverage to run each example through its own local rule crate.

## Verification

- `cargo metadata --no-deps --format-version 1`
- `cargo fmt --all -- --check`
- `cargo test -p polint-cli --test cli checked_in_examples_are_runnable_cli_fixtures -- --nocapture`
- `cargo test -p polint-cli`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
