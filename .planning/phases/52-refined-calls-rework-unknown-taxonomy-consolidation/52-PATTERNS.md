# Phase 52: Refined-Calls Rework & Unknown Taxonomy Consolidation - Patterns

**Generated:** 2026-06-05
**Source:** Inline pattern mapping from 52-CONTEXT.md and current codebase scan

## Closest Existing Analogs

### Refined-Call Provider And Store

- `crates/polint/src/analysis/refined_calls/provider.rs`
  - Current provider shape: `derive_refined_calls_with_cache_stats` builds `RefinedCallOutput`, computes a provider output digest, records `CacheStats`, and persists through `AnalysisDb::replace_refined_call_facts`.
  - Existing issue for Phase 52: it currently layers base direct targets plus `framework`, `go`, `ts_js`, `summaries`, and `extensions` producer modules. Phase 52 should make solver output canonical for dynamic/refined edges.

- `crates/polint/src/analysis/refined_calls/facts.rs`
  - Contract to preserve: `RefinedCallEdgeFact` fields are already rich enough for projection: site, caller, target function/symbol/synthetic target, algorithm, tier, status, reason, provenance, precision, validation, confidence, evidence, input stable keys, stable key.

- `crates/polint/src/analysis/refined_calls/store.rs`
  - Normalization/index pattern: sort by stable key and semantic fields, index by site/caller/target/status/algorithm/provenance/tier, reject duplicate stable keys and duplicate IDs.

### Solver Derived Edges

- `crates/polint/src/analysis/solver/facts.rs`
  - Source fact shape: `DerivedEdgeFact` carries `source`, `target`, `status`, `precision`, `stable_key`, and `DerivedEdgeProvenance`.
  - Precision ceiling: solver-derived edges never map to exact precision.

- `crates/polint/src/analysis/solver/store.rs`
  - Read API: `SolverStore::derived_edges()` and `edges_for_constraint_kind(kind)`.
  - Determinism pattern: normalize rows by stable key, then assign dense IDs.

- `crates/polint/src/analysis/solver/provider.rs`
  - Cache-key pattern: fold provider/version/schema/parameter digests, upstream output digests, budget knobs, row stable keys, status/precision, and provenance.

### Downstream Consumers

- `crates/polint/src/analysis/data_flow/direct_calls.rs`
  - Main compatibility consumer. It derives call boundary nodes/edges from `db.refined_call_edges()`, resolving normal call edges for `CallTargetStatus::Resolved` and producing unresolved call edges otherwise.
  - Phase 52 should preserve this interface.

- `crates/polint/src/analysis/evidence/provider.rs`
  - Evidence derives from data-flow and CFG facts, not directly from solver internals. Solver provenance should not bypass data-flow/refined-call boundaries.

### Public Unknowns CLI

- `crates/polint/src/cli/mod.rs`
  - Current stable path: top-level `polint unknowns --cap <capability> --format json`.
  - Current inspect path: `polint inspect rule`.
  - Phase 52 target path: add `polint inspect unknowns --format json` and keep the top-level command as a compatibility alias through a shared renderer.

- `docs/schemas/polint-unknowns-v1.json`
  - Schema file exists. Prefer optional field additions over breaking existing fields.

- `crates/polint/tests/cli.rs`
  - Current CLI tests already cover unknowns, public no-leak markers, facts list/sample, and docs snippets.

## Implementation Patterns To Preserve

- Use `pub(crate)` for every new graph-engine/internal type.
- Keep public JSON stable, deterministic, schema-versioned, and free of absolute temp paths.
- Assign dense IDs only after stable-key sorting.
- Fold every new behavior-affecting input into provider digests.
- Report budget/setup/unsupported/rejected states explicitly instead of dropping them.
- Keep public no-leak tests passing without adding v1.3 internals to `ALLOWED_PRELUDE`.

## Plan File Map

| Plan | Main analogs | Primary responsibility |
|------|--------------|------------------------|
| `52-01` | `refined_calls/provider.rs`, `solver/facts.rs`, `solver/store.rs` | Project solver-derived edges into `RefinedCallEdgeFact` and update refined-call cache identity. |
| `52-02` | `data_flow/direct_calls.rs`, `evidence/provider.rs`, eval fixtures | Prove downstream compatibility and remove/demote old heuristic refined-call producers. |
| `52-03` | current `cli::unknowns` row generation, public fact-view status mappers | Add private unknown-taxonomy aggregation and deterministic internal row model. |
| `52-04` | `InspectCommand::Rule`, `UnknownsArgs`, `docs/schemas/polint-unknowns-v1.json`, `cli.rs` tests | Add `polint inspect unknowns --format json`, preserve compatibility alias, update docs/schema/tests, and run final verification. |
