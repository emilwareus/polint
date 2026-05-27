---
phase: 32-summary-kernel-and-direct-summaries
verified: 2026-05-27T07:32:55Z
status: passed
score: 7/7 plans verified from closeout artifacts
overrides_applied: 0
gaps: []
reverification: closeout-artifact-reconciliation
---

# Phase 32 Verification: Summary Kernel and Direct Summaries

## Result

PASS. Phase 32 satisfies `SAE-INT-02`.

## Evidence Reviewed

- `32-01-SUMMARY.md`: summary kernel contracts and domain vocabulary.
- `32-02-SUMMARY.md`: summary store and AnalysisDb integration.
- `32-03-SUMMARY.md`: summary builder and direct summary computation.
- `32-04-SUMMARY.md`: provider and cache identity.
- `32-05-SUMMARY.md`: summary validation and debug JSON.
- `32-06-SUMMARY.md`: direct-summary eval fixture coverage.
- `32-07-SUMMARY.md`: public-boundary proof.

## Verification Commands Recorded In Phase Summaries

- `cargo test -p polint --lib analysis::summaries --locked`
- `cargo test -p polint --lib direct_summaries --locked`
- `cargo test -p polint --lib analysis_kernel::validation --locked`
- `cargo test -p polint --lib eval_native_fixture_runner_direct_summaries_fixture_passes --locked`
- Full library and integration test evidence recorded in the final summaries: 993 lib tests and 122 integration tests passed.
- `cargo fmt --all -- --check`

## Requirement Coverage

| Requirement | Status | Evidence |
|---|---|---|
| SAE-INT-02 | passed | Summary keys, stores, typed domains, local/direct summaries, effects, return/TITO, memory-touch approximations, metadata, cache identity, validation, eval fixtures, and no-leak proof were implemented and verified. |

## Closeout Note

This verification file was restored during v1.2 archival reconciliation from existing phase summaries. No product code was changed.
