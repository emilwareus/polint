---
phase: 44-semantic-graph-skeleton-constraint-vocabulary
plan: 03
subsystem: api
tags: [semantic-graph, rust, provider, cache-key, byte-stability, determinism, eval-fixtures]

# Dependency graph
requires:
  - phase: 44-semantic-graph-skeleton-constraint-vocabulary
    provides: 44-01 node/edge skeleton + 44-02 ConstraintKind vocabulary + build_semantic_graph projection
  - phase: 43-reachability-and-root-semantics
    provides: reachability provider/cache_key/validate analog + N=10 seeded-permutation determinism gate
  - phase: 42-benchmark-identity-renderers-dedup
    provides: public_surface_leak.rs CI gate (ALLOWED_PRELUDE)
provides:
  - polint.semantic_graph provider (derive_semantic_graph_with_cache_stats) with 7-phase pipeline + output digest
  - SEMANTIC_GRAPH_SCHEMA_LABEL + frozen parameter digest + lock tests + SC3 present-vs-deferred inputs doc
  - validate_semantic_graph (structural + precision-ceiling) wired between validate_type_value_alias and validate_refined_calls
  - AnalysisDb::replace_semantic_graph_facts store path + semantic node/edge/constraint accessors
  - ProviderManifest registered between polint.type_value_alias and polint.refined_calls (all order vectors + report row)
  - Go + TS/JS byte-stable snapshot fixtures asserting >=1 node/edge/constraint
affects: [47-unified-call-graph-solver, 49-adaptation-model-layer, 51-cache-and-solver-budgets]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provider output digest folds every consumed upstream provider output digest + provider/schema/parameter, with an empty-output sentinel and #[serde(skip)] dense IDs (D-17)"
    - "In-scope dependency-digest re-derivation at a mid-pipeline splice: clone the Option before it is moved into a downstream consumer, or unwrap_or_else(Digest::absent)"
    - "SC3 present-vs-deferred dependency-index inputs self-documented in BOTH cache_key.rs and the manifest inputs slice; zero deferred inputs digested until a producer lands"
    - "Snapshot-gate eval test sourcing a byte-stable debug JSON from metadata_debug_json_for_test, total-ordered by stable key per family"

key-files:
  created:
    - crates/polint/src/analysis/semantic_graph/cache_key.rs
    - crates/polint/src/analysis/semantic_graph/validate.rs
    - crates/polint/src/analysis/semantic_graph/provider.rs
    - crates/polint/src/analysis/semantic_graph/debug.rs
    - crates/polint/src/eval/semantic_graph_snapshot.rs
    - tests/eval-fixtures/semantic-graph/go_graph/ (expected.polint-eval.toml + repo/{.polint.toml,go.mod,main.go})
    - tests/eval-fixtures/semantic-graph/ts_graph/ (expected.polint-eval.toml + repo/{.polint.toml,package.json,src/app.ts})
  modified:
    - crates/polint/src/analysis/semantic_graph/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/mod.rs

key-decisions:
  - "Precision ceiling rejects the exact-EQUIVALENT tier the graph types can carry (SemanticPrecision::ResolvedStatic) since SemanticPrecision/PointsToPrecision have no literal Exact variant; a reject_exact_precision(FactPrecision) helper preserves the literal producer-level contract"
  - "replace_semantic_graph_facts routes through SemanticGraphStore::from_output so normalization + referential validation happen on store, returning Result for the dangling-reference case"
  - "Re-derived in-scope dependency digests by cloning the reachability Option before the push and adding .clone() at the identity/abstract_domains/symbol/topology move sites, rather than restructuring the kernel run"
  - "Snapshot test lives in a dedicated eval::semantic_graph_snapshot gate (not eval::determinism_gate, which auto-enrolls the provider unmodified) per D-22"

patterns-established:
  - "Pattern: provider output digest with empty-output sentinel + every consumed upstream digest, mirroring reachability/provider.rs (S4/S5)"
  - "Pattern: dual self-documentation of SC3 present-vs-deferred inputs in cache_key.rs and the manifest inputs slice, with zero deferred inputs actually digested"

requirements-completed: [GRAPH-01, GRAPH-02]

# Metrics
duration: 15min
completed: 2026-05-30
---

# Phase 44 Plan 03: Provider, Validation & Snapshot Fixtures Summary

**`polint.semantic_graph` is now a real cached, validated, deterministic kernel provider: a 7-phase pipeline with an output digest folding every consumed upstream provider output, structural + precision-ceiling validation in-sequence, the manifest registered in the correct order slot (auto-enrolled into the Phase 43 determinism gate), and Go + TS/JS fixtures proving byte-stable >=1-of-each-kind constraint emission — GRAPH-01 and GRAPH-02 closed.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-05-30T10:37:50Z
- **Completed:** 2026-05-30T10:52:50Z
- **Tasks:** 3
- **Files modified:** 13 (7 created, 6 modified)

## Accomplishments
- Added `cache_key.rs` (`SEMANTIC_GRAPH_SCHEMA_LABEL = "semantic-graph-facts-1"` + `semantic_graph_provider_parameter_digest()` over a frozen `&[&str]` with `*_locks_parts_list` / `algorithm_version_bump_invalidates` / schema-label lock tests) and a self-documenting SC3 present-vs-deferred dependency-index comment naming the deferred inputs (MIR, CFG, summaries, accepted adaptation models, solver budgets — Phases 47/49/51/53) — zero deferred inputs digested.
- Added `validate.rs` (`validate_semantic_graph`): duplicate-stable-key checks per family, dangling edge-endpoint / constraint-node-ref referential checks, dense-IDs-contiguous-and-stable-key-sorted assertion, and the exact-equivalent precision ceiling — all evidence-bearing `Diagnostic`s with `family`/`stable_key`/`field`/`reason`, never silent drops.
- Added `AnalysisDb::replace_semantic_graph_facts` (routes through `SemanticGraphStore::from_output` → normalize + referential validate, `Result` on dangling refs) plus `semantic_nodes()/edges()/constraints()` accessors.
- Added `provider.rs` (`derive_semantic_graph_with_cache_stats`): build → normalize → output digest over stored stable payloads (empty-output sentinel, never dense IDs) folding in calls/identity/abstract_domains/entrypoints/reachability/type_value_alias output digests + symbol/topology digests (D-17) → store → `output_digest: None` on store error.
- Registered the `polint.semantic_graph` `ProviderManifest` between `polint.type_value_alias` and `polint.refined_calls` with the present-vs-deferred SC3 inputs comment on its `inputs` slice; inserted `"polint.semantic_graph"` into all three `provider_order_for_test` vectors and the `provider_order_report` row block; spliced the provider run + `validate_semantic_graph` call in the correct kernel positions.
- Added `debug.rs` + Go/TS snapshot fixtures + the `eval::semantic_graph_snapshot` gate asserting >=1 node/edge/constraint, a Call edge + CallConstraint, and byte-stable total-ordered debug JSON; the Phase 43 determinism gate stays green via auto-enrollment and the Phase 42 leak gate stays green unmodified.

## Task Commits

Each task was committed atomically:

1. **Task 1: cache_key.rs + validate.rs + AnalysisDb store path** — `dc7dbfaa` (feat)
2. **Task 2: provider.rs pipeline + kernel manifest registration + run splice + validation wiring** — `28a4b835` (feat)
3. **Task 3: Go + TS/JS snapshot fixtures, determinism-gate inheritance, public-surface-leak proof** — `a6afff39` (test)

_Note: the `tdd="true"` tasks were authored as single commits per task because each task's files share a compilation unit (the kernel splice needs the provider; the snapshot gate needs the debug observation); tests and implementation were written and verified together per task._

## Files Created/Modified
- `crates/polint/src/analysis/semantic_graph/cache_key.rs` — schema label, frozen parameter digest, lock tests, SC3 present-vs-deferred doc.
- `crates/polint/src/analysis/semantic_graph/validate.rs` — `validate_semantic_graph` structural + precision-ceiling pass.
- `crates/polint/src/analysis/semantic_graph/provider.rs` — `derive_semantic_graph_with_cache_stats` 7-phase pipeline + output digest.
- `crates/polint/src/analysis/semantic_graph/debug.rs` — `metadata_debug_json_for_test` byte-stable snapshot observation (test-only).
- `crates/polint/src/analysis/semantic_graph/mod.rs` — registered `cache_key`, `validate`, `provider`, and `#[cfg(test)] debug`.
- `crates/polint/src/analysis_kernel/provider.rs` — `SEMANTIC_GRAPH_SCHEMA` const + manifest + 3 order vectors + report row.
- `crates/polint/src/analysis_kernel/mod.rs` — provider run splice + in-scope dependency-digest re-derivation.
- `crates/polint/src/analysis_kernel/validation.rs` — `validate_semantic_graph` call in-sequence.
- `crates/polint/src/core/mod.rs` — `replace_semantic_graph_facts` + accessors + struct fields + imports.
- `crates/polint/src/eval/semantic_graph_snapshot.rs` — Go/TS snapshot gate.
- `crates/polint/src/eval/mod.rs` — registered `#[cfg(test)] semantic_graph_snapshot`.
- `tests/eval-fixtures/semantic-graph/{go_graph,ts_graph}/` — byte-stable snapshot fixtures.

## Decisions Made
- **Precision ceiling on the exact-equivalent tier:** `SemanticPrecision` and `PointsToPrecision` carry no literal `Exact` variant by construction, so the node/edge ceiling rejects `SemanticPrecision::ResolvedStatic` (documented in `facts.rs` as the exact-equivalent ceiling). A `reject_exact_precision(FactPrecision::Exact, ...)` helper mirroring `reachability::validate` preserves the literal producer-level contract for callers that map to `FactPrecision`.
- **Store path routes through `from_output`:** `replace_semantic_graph_facts` reuses `SemanticGraphStore::from_output` (normalize + referential validation), returning `Result` so a dangling edge/constraint reference fails the store rather than persisting a malformed graph.
- **Mid-pipeline digest re-derivation:** rather than restructuring the kernel run, cloned the reachability output Option before its push (new `reachability_dependency_output_digest`) and added `.clone()` at the identity/abstract_domains/symbol/topology move sites so the semantic-graph splice can consume in-scope dependency digests.
- **Dedicated snapshot gate:** the byte-stable proof lives in `eval::semantic_graph_snapshot`; `eval::determinism_gate` was NOT edited (the provider auto-enrolls via `provider_manifests()` per D-22).

## Deviations from Plan

None - plan executed exactly as written. The optional `debug.rs` (`metadata_debug_json_for_test`) the plan flagged "ONLY if needed to source the snapshot observation" was needed and added, as anticipated by the plan.

## Issues Encountered
- The pre-commit lint hook runs `cargo clippy -D warnings`; the first Task 1 commit failed because (a) the new files needed `cargo fmt` and (b) `replace_semantic_graph_facts` was dead code until Task 2 wired it. Resolved by formatting and adding a temporary `#[allow(dead_code, reason=...)]` (matching the reachability precedent), removed in Task 2 once the kernel splice made the method live.

## Known Stubs
The constraint kinds `ModelEdge`, `Alloc`, `FieldLoad`/`FieldStore`, and `TypeConstraint` remain honest zero-emission in the minimal projection (inherited from 44-02; documented there with named resolving phases — 47/49 and later v1.3 field/type/object-token plans). This plan's fixtures assert the kinds that DO emit today (Function/Package/Scope/Callsite/Place nodes, Call/MemberOf edges, CallConstraint/CopyEdge constraints), which satisfies the GRAPH-02 ">=1 of each kind the minimal projection emits" acceptance. No new stubs were introduced.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `polint.semantic_graph` is a real cached/validated/deterministic provider in the correct order slot, ready for Phase 47's unified call-graph solver to consume (the digest already reserves the deferred SC3 inputs MIR/CFG/summaries/adaptation-models/solver-budgets, which enter when their producers land).
- The Go + TS/JS snapshot fixtures and the auto-enrolled determinism gate give Phase 47 a byte-stability safety net when it swaps the minimal projection for solver-derived edges/constraints behind the same provider contract.
- No blockers.

---
*Phase: 44-semantic-graph-skeleton-constraint-vocabulary*
*Completed: 2026-05-30*

## Self-Check: PASSED

- FOUND: crates/polint/src/analysis/semantic_graph/cache_key.rs
- FOUND: crates/polint/src/analysis/semantic_graph/validate.rs
- FOUND: crates/polint/src/analysis/semantic_graph/provider.rs
- FOUND: crates/polint/src/analysis/semantic_graph/debug.rs
- FOUND: crates/polint/src/eval/semantic_graph_snapshot.rs
- FOUND: tests/eval-fixtures/semantic-graph/go_graph/expected.polint-eval.toml
- FOUND: tests/eval-fixtures/semantic-graph/ts_graph/expected.polint-eval.toml
- FOUND commit: dc7dbfaa (Task 1)
- FOUND commit: 28a4b835 (Task 2)
- FOUND commit: a6afff39 (Task 3)
