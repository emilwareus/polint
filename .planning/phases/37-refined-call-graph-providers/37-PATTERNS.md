# Phase 37: Refined Call Graph Providers - Pattern Map

## Existing Patterns To Reuse

### Direct Call Facts

- `crates/polint/src/analysis/calls/facts.rs` defines the base vocabulary for call sites, targets, unresolved calls, algorithms, statuses, precision, and provenance.
- `crates/polint/src/analysis/calls/store.rs` is the closest storage analog: normalized output, deterministic indexes, dangling-site validation, and query helpers.
- `crates/polint/src/analysis/calls/provider.rs` is the closest provider analog: extract facts, normalize, compute output digest from upstream digests and fact stable keys, then replace facts in `AnalysisDb`.

### Precision Substrate

- `crates/polint/src/analysis/types/facts.rs`, `values/facts.rs`, `access_paths/`, `points_to/`, and `aliases/` provide the type/value/place/points-to/alias rows Phase 37 should consume.
- `crates/polint/src/analysis/aliases/provider_stack.rs` shows budgeted derivation and explicit `BudgetExceeded` reporting instead of silent truncation.

### Framework And Extension Inputs

- `crates/polint/src/analysis/entrypoints/facts.rs` provides `FrameworkDispatchEdgeFact` and unresolved framework facts for framework-driven refined calls.
- `crates/polint/src/analysis/extensions/sinks.rs` provides validated extension fact candidate structure, precision ceilings, payload labels, and rejection behavior.

### Kernel/Eval/Public Boundary

- `crates/polint/src/analysis_kernel/provider.rs` owns provider manifest order and schema labels.
- `crates/polint/src/analysis_kernel/mod.rs` owns provider execution order and `KernelRunReport` integration.
- `crates/polint/src/analysis_kernel/debug.rs` and `validation.rs` are the established places for private debug rows and validation checks.
- `crates/polint/src/eval/model.rs` and `tests/eval-fixtures/` define how internal facts become deterministic expected/observed eval rows.

## Recommended File Ownership By Plan

| Plan | Primary Files |
|------|---------------|
| 37-01 | `analysis/ids.rs`, new `analysis/refined_calls/*`, `analysis/mod.rs`, metadata families |
| 37-02 | provider manifest/order, kernel run step, cache key, `AnalysisDb` storage/accessors |
| 37-03 | framework dispatch and summary-assisted refinement logic |
| 37-04 | Go receiver/type refinement logic and tests |
| 37-05 | TS/JS function-token, bounded points-to, and extension/model refinement logic |
| 37-06 | validation, debug, eval fixtures, provider-order expectations, public no-leak tests |

## Planning Constraints

- Keep the direct `polint.calls` provider intact.
- Add refined facts as a private layer with explicit direct-versus-refined delta.
- Put every edge behind precision/status/provenance and budget/setup/unsupported vocabulary.
- Do not add public SDK views or public CLI output for refined calls in Phase 37.
