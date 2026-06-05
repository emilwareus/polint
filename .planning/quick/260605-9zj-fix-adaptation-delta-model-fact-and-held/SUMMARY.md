# Quick Task 260605-9zj Summary

## Status

Complete.

## Changes

- Persisted accepted and rejected adaptation model facts as crate-private audit rows in `AnalysisDb`.
- Projected adaptation model audit rows into eval observations so adapted deltas can count real `AdaptationModel` facts.
- Added a partition-aware adaptation delta path that computes held-out unknown, precision, recall, runtime, and cache-scope reporting.
- Replaced the structural held-out test with a fixture-backed regression that reads `tests/eval-fixtures/adaptation-model/held-out-delta/partition.toml`.

## Verification

- `cargo test -p polint eval::delta --locked`
- `make check`
