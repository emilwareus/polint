# Quick Task 260512-yml: Replace unsound serde_yml dependency

**Date:** 2026-05-12
**Status:** In Progress

## Goal

Make PR #10 mergeable by fixing the failing `cargo deny` advisory check for
`RUSTSEC-2025-0068` on `serde_yml`.

## Plan

1. Replace `serde_yml` with a maintained YAML serde crate that satisfies the
   workspace license policy.
2. Update the compact baseline parser error type and manifest lockfile.
3. Verify with baseline tests, workspace checks, and local `cargo deny`.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test -p polint baseline::tests --locked`
- `cargo test --workspace --all-features --locked`
- `cargo deny --all-features --locked check`
