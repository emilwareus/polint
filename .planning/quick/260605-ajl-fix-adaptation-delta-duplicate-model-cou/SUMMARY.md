# Quick Task 260605-ajl Summary

## Status

Complete.

## Changes

- Deduplicated accepted/rejected adaptation model fact counters across repeated per-case observations.
- Excluded held-out cases from top-level adapted delta counters and runtime ratio when a held-out partition is supplied.
- Added regressions for duplicate model fact counting and held-out-only delta separation.

## Verification

- `cargo test -p polint eval::delta --locked`
- `make lint`
