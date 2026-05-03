# Quick Task 260503-ba9 Summary

**Date:** 2026-05-03
**Status:** Complete

## Completed

- Added `examples/multiple-rules` to demonstrate one local Cargo rule-pack crate
  registering multiple repo-local rules.
- Added mixed TSX and Go fixture files that trigger `local/no-raw-colors` and
  `local/go-import-boundaries` from the same rule host.
- Added README guidance explaining when one rule-pack `Cargo.toml` is the right
  shape versus one package per rule.
- Added CLI e2e coverage proving the example uses one manifest and emits both
  rule IDs.
- Replaced the example workspace wildcard with explicit example package members
  so Cargo does not treat rule-pack `src/` directories as packages.

## Verification

- `cargo metadata --no-deps --format-version 1`
- `cargo fmt --all -- --check`
- `cargo test -p polint-cli --test cli checked_in_multiple_rules_example_uses_one_rule_pack_crate -- --nocapture`
- `cargo test -p polint-cli --test cli checked_in_examples_are_runnable_cli_fixtures -- --nocapture`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
