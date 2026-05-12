# Quick Task 260512-yml: Replace unsound serde_yml dependency - Summary

**Date:** 2026-05-12
**Status:** Complete
**Code Commit:** `c2f678e`

## What Changed

- Replaced `serde_yml` with the maintained `serde_norway` YAML serde crate for baseline parsing.
- Removed the `RUSTSEC-2025-0068` cargo-deny advisory from the dependency tree.
- Preserved the existing compact baseline parser behavior and tests.

## Verification

- `cargo deny --all-features --locked check`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test -p polint baseline::tests --locked`
- `cargo test --workspace --all-features --locked`
