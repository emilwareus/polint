# Phase 53 Pattern Map

**Phase:** 53 - Cache & Solver Budgets Consolidation
**Generated:** 2026-06-05

## Closest Existing Patterns

### Cache-Key Recipe Tests

- `crates/polint/src/analysis/solver/cache_key.rs` reconstructs the exact solver parameter digest parts list and adds focused invalidation tests for algorithm versions and budget changes.
- `crates/polint/src/go/semantic/cache_key.rs` provides the clearest positive/negative digest pattern: sidecar/toolchain/lifecycle changes invalidate; intentionally irrelevant lifecycle fields preserve the hit.
- `crates/polint/src/analysis/adaptation/cache_key.rs` proves behavior-affecting model changes and budget changes invalidate while deterministic reorder preserves the digest.
- `crates/polint/src/analysis/semantic_graph/cache_key.rs` uses comments plus locked parameter tests to document present and deferred dependency-index inputs.

### Budget Evidence

- `crates/polint/src/analysis/solver/budget.rs` is the shared budget vocabulary. `BudgetStatus::BudgetExceeded` is the single run-level signal; sub-budget structs hold driver-specific knobs.
- `crates/polint/src/eval/go_rta.rs`, `crates/polint/src/eval/ts_tokens.rs`, and `crates/polint/src/eval/ts_object_model.rs` are the native executable proof style for budget exhaustion and determinism.
- `crates/polint/src/analysis/unknown_taxonomy/collect.rs` is the Phase 52 aggregation seam for budget/setup/model/unsupported rows.

### Reporting

- `crates/polint/src/eval/report.rs` keeps internal metrics schema defaulted and deterministic.
- `crates/polint/src/eval/markdown.rs` renders provider cache stats and avoids transient paths/timestamps in snapshots.
- `crates/polint/src/eval/performance.rs` is the natural place to extend provider/runtime performance data if RSS measurements are captured per provider or per run.

## Planning Implications

- Reuse production digest helpers in tests; do not create a parallel digest registry.
- Put broad cache proof behind small native/temp fixtures, not external benchmark corpora.
- Keep budget reasons private and stable, with public visibility only through existing unknown/report surfaces.
- Add RSS fields as defaulted internal report fields so older fixture JSON remains deserializable.
