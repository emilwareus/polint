---
phase: 29-local-cfg-and-control-dependence
verified: 2026-05-27T07:32:55Z
status: passed
score: 6/6 plans verified from closeout artifacts
overrides_applied: 0
gaps: []
reverification: closeout-artifact-reconciliation
---

# Phase 29 Verification: Local CFG and Control Dependence

## Result

PASS. Phase 29 satisfies `SAE-SEM-04`.

## Evidence Reviewed

- `29-01-SUMMARY.md`: private CFG contracts and storage.
- `29-02-SUMMARY.md`: shared CFG builder and derived reachability, dominance, postdominance, and control-dependence analyses.
- `29-03-SUMMARY.md`: provider, cache identity, validation, and debug wiring.
- `29-04-SUMMARY.md`: Go CFG lowering from private MIR.
- `29-05-SUMMARY.md`: TS/JS CFG lowering from private MIR.
- `29-06-SUMMARY.md`: CFG eval fixture snapshots, public no-leak proof, and unsupported capability honesty.

## Verification Commands Recorded In Phase Summaries

- `cargo test -p polint --lib analysis::cfg --locked`
- `cargo test -p polint --lib analysis::cfg::derived --locked`
- `cargo test -p polint --lib cfg_provider --locked`
- `cargo test -p polint --lib analysis_kernel::validation::cfg --locked`
- `cargo test -p polint --lib analysis_kernel::debug::cfg_debug_json --locked`
- `cargo test -p polint --lib analysis::cfg::lower_go --locked`
- `cargo test -p polint --lib analysis::cfg::lower_ts --locked`
- `cargo test -p polint --lib eval::fixtures::cfg_core --locked`
- `cargo test -p polint --test cli cfg_public_no_leak --locked`
- `cargo test -p polint --test cli cfg_capability_remains_unsupported --locked`
- `cargo test -p polint --all-targets --locked`
- `cargo fmt --all -- --check`

## Requirement Coverage

| Requirement | Status | Evidence |
|---|---|---|
| SAE-SEM-04 | passed | Local CFG nodes, edges, reachability, dominance, postdominance, and control-dependence facts were built over MIR for supported Go and TS/JS constructs, with eval fixtures and public-boundary proof. |

## Closeout Note

This verification file was restored during v1.2 archival reconciliation from existing phase summaries. No product code was changed.
