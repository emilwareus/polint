# Quick Task 260510-dzr: Implement reusable derived metric signals

**Date:** 2026-05-10
**Status:** Complete

## What Changed

- Added derived metric fact families:
  - `FileMetricFact` / `FileMetrics<'_>`
  - `FunctionMetricFact` / `FunctionMetrics<'_>`
  - `ComplexityMetricFact` / `ComplexityMetrics<'_>`
- Wired metric views into static capability derivation and AnalysisPlan support.
- Added a metric derivation pass after Go/TS analysis and before local rules run.
- Added `examples/code-quality-metrics` with three independent rules consuming
  shared metric signals.
- Updated docs, generated `polint add-skill` content, and project agent guidance.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked --quiet`
- Direct CLI smoke:
  - `../../target/debug/polint check --format json --no-cache --fail-on none`
  - `../../target/debug/polint check --shortstat --no-cache --fail-on none --color never`
