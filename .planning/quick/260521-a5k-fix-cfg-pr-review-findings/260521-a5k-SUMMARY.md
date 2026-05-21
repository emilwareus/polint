# Quick Task 260521-a5k: Fix CFG PR Review Findings - Summary

**Completed:** 2026-05-21

## Changes

- Replaced run-local dense IDs in derived CFG stable keys with existing CFG function, block, and edge stable keys.
- Added implicit normal fallthrough edges from non-terminated Go and TS/JS CFG bodies to the normal exit block.
- Strengthened CFG validation for function entry blocks, selected exits, and block reachability labels.
- Added regression coverage for stable derived keys, implicit fallthrough exits, and validation diagnostics.

## Verification

- `cargo test -p polint --lib analysis::cfg::derived --locked` passed.
- `cargo test -p polint --lib analysis::cfg::lower_go --locked` passed.
- `cargo test -p polint --lib analysis::cfg::lower_ts --locked` passed.
- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo test -p polint --lib analysis_kernel::validation::cfg --locked` passed.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed.
- `cargo test -p polint --all-targets --locked` was attempted; library tests passed, then CLI tests failed because rule-host builds hit `No space left on device (os error 28)` while linking/writing Cargo artifacts.
