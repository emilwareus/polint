---
phase: 28-private-semantic-mir-and-place-identity
verified: 2026-05-20T10:04:56Z
status: passed
score: 22/22 must-haves verified
overrides_applied: 0
---

# Phase 28: Private Semantic MIR and Place Identity Verification Report

**Phase Goal:** Add private `analysis::mir` and `analysis::places` lowering for Go and TS/JS function bodies.
**Verified:** 2026-05-20T10:04:56Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

Phase 28 achieved the goal in the actual codebase. The implementation adds a crate-private semantic analysis substrate, deterministic MIR/place identity, Go and TS/JS lowering, private storage and metadata, provider/cache/debug/validation wiring, semantic-MIR eval snapshots, and public boundary proof that no unsupported MIR/place surface was promoted.

Review fixes from commits `9a39e5a`, `c597b95`, and `3444e56` were included in this verification.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `analysis` is crate-private and contains MIR/place contracts only. | VERIFIED | `crates/polint/src/lib.rs` declares `pub(crate) mod analysis;`; MIR/place modules are under `crates/polint/src/analysis`. |
| 2 | MIR and place rows use run-local dense IDs plus deterministic stable keys, not parser AST objects. | VERIFIED | `ids.rs`, `stable_key.rs`, `places.rs`, `mir/body.rs`, and `mir/op.rs` define owned IDs, stable keys, spans, statuses, and row contracts; no AST node is stored in emitted rows. |
| 3 | Place identity covers locals, parameters, globals, temporaries, call returns, unknown roots, fields/properties, indexes, deref-like projections, await results, and unknown projections. | VERIFIED | `PlaceRoot` and `PlaceProjection` variants cover the required vocabulary, with unit coverage for stable keys and deterministic ID assignment. |
| 4 | `AnalysisDb` owns private semantic MIR artifacts through `SemanticStore`. | VERIFIED | `analysis/store.rs` defines `SemanticStore`; `core/mod.rs` exposes crate-private replacement/accessors only. |
| 5 | MIR body, operation, place, and unsupported rows receive Phase 21 metadata. | VERIFIED | `FactFamily::{MirBody, MirOperation, Place, UnsupportedSemantic}` metadata refresh paths exist and metadata tests passed. |
| 6 | Replacement normalizes rows deterministically and clears stale MIR metadata. | VERIFIED | `replace_semantic_mir` normalizes/remaps rows and storage tests cover stale removal, deterministic reassignment, and dangling references. |
| 7 | Go bodies lower into deterministic MIR bodies, operations, places, and unsupported rows. | VERIFIED | `lower_go_mir` emits `MirOutput`; Go lowering tests passed, including deterministic call sites and unsupported semantics. |
| 8 | Go lowering covers parameters, locals, globals, temporaries, fields, indexes, assignments, reads, branches, returns, call-shaped operations, and unknown roots. | VERIFIED | Go lowering tests cover function places, statement/control operation shapes, declarations, compound assignments, calls, and unsupported constructs. |
| 9 | Go parser nodes remain local to lowering and do not escape emitted MIR/place rows. | VERIFIED | Row contracts store polint-owned spans, stable keys, roots, projections, values, and statuses; no tree-sitter node type is present in MIR/place row structs. |
| 10 | TS/JS bodies lower into deterministic MIR bodies, operations, places, and unsupported rows. | VERIFIED | `lower_ts_mir` emits `MirOutput`; TS/JS lowering tests passed for deterministic operation and place output. |
| 11 | TS/JS lowering covers parameters, locals, globals, temporaries, properties, indexes, assignments, reads, branches, returns, call-shaped operations, and unknown roots. | VERIFIED | TS/JS lowering tests cover function places, property/index projections, assignment modes, control shapes, calls, and unsupported semantics. |
| 12 | Oxc AST references remain local to lowering and do not escape emitted MIR/place rows. | VERIFIED | Emitted TS/JS rows contain only polint-owned row data; no Oxc AST object is stored in MIR/place contracts. |
| 13 | A private `polint.semantic_mir` provider runs after symbol/topology facts and before downstream consumers. | VERIFIED | `analysis/provider.rs`, `analysis_kernel/mod.rs`, and `analysis_kernel/provider.rs` wire the provider and manifest. |
| 14 | MIR/place cache identity includes provider/schema, source, lifecycle, config, plan/provider parameters, upstream syntax/semantic/topology, and absent extension/model/tool slots. | VERIFIED | `analysis/cache_key.rs` and `analysis_kernel/incremental/keys.rs` contain semantic MIR layer key helpers and tests. |
| 15 | Validation rejects malformed semantic MIR rows and conservative precision violations. | VERIFIED | `analysis/validate.rs` and `analysis_kernel/validation.rs` validate stable keys, refs, spans, projections, unsupported evidence, and precision ceilings. |
| 16 | Internal eval observes MIR bodies, operations, places, and unsupported rows through crate-private debug data. | VERIFIED | `eval/observed.rs` reads semantic MIR sections from `metadata_debug_json_for_test`. |
| 17 | Native semantic-MIR eval fixture proves deterministic Go and TS/JS snapshots. | VERIFIED | `tests/eval-fixtures/semantic-mir/core/expected.polint-eval.toml` and fixture tests passed. |
| 18 | Fixture assertions cover supported places and explicit unsupported semantics, not downstream CFG/domain results. | VERIFIED | The semantic-MIR fixture asserts MIR/place/unsupported rows only; no CFG, call-graph, or dataflow facts were introduced. |
| 19 | Public check JSON, inspect rule JSON, polint test JSON, CLI help, SDK, runner, crate root, README, and docs do not expose semantic MIR/place internals. | VERIFIED | `semantic_mir_internals_stay_private` passed in the full CLI test suite. |
| 20 | Existing public rule-author behavior through `polint::sdk::prelude::*` and `polint::runner::run_cli` remains compatible. | VERIFIED | The public-boundary temp repo rule imports only supported SDK/runner APIs and runs successfully. |
| 21 | No public `Mir`, `Places`, `Cfg`, `CallGraph`, or `DataFlow` view was promoted. | VERIFIED | No-leak source scans and CLI tests passed; unsupported views remain absent from public surfaces. |
| 22 | Review findings are fixed in final state. | VERIFIED | Stable keys avoid dense IDs, unsupported refs remap after sorting, Go var/compound assignment lowering is covered, and Go multi-argument calls preserve all argument places. |

**Score:** 22/22 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint/src/analysis/places.rs` | Place identity model and stable keys | VERIFIED | Exists with crate-private place roots, projections, statuses, stable context, and builder tests. |
| `crates/polint/src/analysis/mir/body.rs` | MIR body contracts | VERIFIED | Exists with `MirBody`, `MirOutput`, status, normalization, and row contracts. |
| `crates/polint/src/analysis/mir/op.rs` | MIR operations and unsupported semantic rows | VERIFIED | Exists with operation kinds, call/assign/control shapes, unsupported rows, domains, and conservative actions. |
| `crates/polint/src/analysis/store.rs` | SemanticStore replacement boundary | VERIFIED | Exists and owns normalized semantic MIR output. |
| `crates/polint/src/core/mod.rs` | AnalysisDb semantic MIR storage and metadata | VERIFIED | Contains `replace_semantic_mir`, crate-private accessors, metadata refresh, and storage tests. |
| `crates/polint/src/analysis/mir/lower_go.rs` | Go tree-sitter to MIR lowering | VERIFIED | Exists and lowers Go bodies into owned MIR/place rows. |
| `crates/polint/src/analysis/mir/lower_ts.rs` | Oxc TS/JS to MIR lowering | VERIFIED | Exists and lowers TS/JS bodies into owned MIR/place rows. |
| `crates/polint/src/analysis/provider.rs` | Semantic MIR provider derivation | VERIFIED | Exists with language output merge/remapping and provider stats. |
| `crates/polint/src/analysis/cache_key.rs` | Semantic MIR cache identity | VERIFIED | Exists and participates in layer key construction. |
| `crates/polint/src/analysis/validate.rs` | Semantic MIR validation | VERIFIED | Exists with row/reference/evidence checks. |
| `crates/polint/src/analysis_kernel/provider.rs` | Provider manifest | VERIFIED | Contains the private `polint.semantic_mir` manifest. |
| `crates/polint/src/analysis_kernel/debug.rs` | Test-only semantic MIR debug output | VERIFIED | Emits deterministic private debug rows for eval/tests. |
| `crates/polint/src/eval/observed.rs` | MIR eval observation | VERIFIED | Observes semantic MIR rows through crate-private debug data. |
| `tests/eval-fixtures/semantic-mir/core/expected.polint-eval.toml` | Expected semantic MIR eval snapshot | VERIFIED | Exists and passed fixture tests. |
| `crates/polint/tests/cli.rs` | Public no-leak and compatibility proof | VERIFIED | Contains `semantic_mir_internals_stay_private`, which passed in the full workspace run. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `lib.rs` | `analysis` module | `pub(crate) mod analysis` | WIRED | New analysis substrate is internal to the crate. |
| `mir/body.rs` | `places.rs` | `PlaceId` references | WIRED | MIR rows reference normalized place IDs. |
| `core/mod.rs` | `analysis/store.rs` | `SemanticStore` | WIRED | AnalysisDb stores semantic MIR through the private store boundary. |
| `lower_go.rs` | MIR/place contracts | `MirOutput` and `PlaceTableBuilder` | WIRED | Go lowering emits owned semantic rows. |
| `lower_ts.rs` | MIR/place contracts | `MirOutput` and `PlaceTableBuilder` | WIRED | TS/JS lowering emits owned semantic rows. |
| `analysis_kernel/mod.rs` | `analysis/provider.rs` | `derive_semantic_mir_with_cache_stats` | WIRED | Kernel runs semantic MIR provider after upstream semantic/topology layers. |
| `analysis_kernel/incremental/keys.rs` | `analysis/cache_key.rs` | semantic MIR layer key | WIRED | Cache identity includes provider and upstream input digests. |
| `analysis_kernel/validation.rs` | `analysis/validate.rs` | validation bridge | WIRED | Kernel validation covers semantic MIR rows. |
| `eval/observed.rs` | `analysis_kernel/debug.rs` | `metadata_debug_json_for_test` | WIRED | Eval observes private MIR rows without public APIs. |
| `cli.rs` | public SDK/CLI/docs surfaces | no-leak assertions | WIRED | Public output and source scans remain private. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Formatting | `cargo fmt --all -- --check` | exit 0 | PASS |
| Go multi-argument call regression | `cargo test -p polint --locked --lib analysis::mir::lower_go::operations::go_call_operations_are_shape_evidence_with_deterministic_call_sites` | 1 passed | PASS |
| Full workspace regression | `cargo test --workspace --locked` | all tests passed | PASS |
| Schema drift | `node "$HOME/.codex/get-shit-done/bin/gsd-tools.cjs" verify schema-drift 28` | `drift_detected: false` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SAE-SEM-03 | 28-01 through 28-07 | polint has a private semantic MIR and normalized place identity for Go and TS/JS function bodies, with deterministic lowering snapshots and explicit unsupported operations. | SATISFIED | All seven plans declare and address `SAE-SEM-03`; roadmap success criteria and all plan must-haves are verified through code, metadata, validation, eval fixtures, review fixes, and public no-leak tests. |

No orphaned Phase 28 requirement IDs were found. `.planning/REQUIREMENTS.md` maps Phase 28 to `SAE-SEM-03`, and all seven plan frontmatters declare `requirements: [SAE-SEM-03]`.

### Human Verification Required

None. This phase is internal Rust/provider/cache/eval behavior with executable tests and source-level public-boundary verification. No visual, realtime, or external service behavior requires manual confirmation.

### Gaps Summary

No gaps found. Phase 28 is ready for Phase 29 local CFG/control-dependence work over the private MIR/place substrate.

---

_Verified: 2026-05-20T10:04:56Z_
_Verifier: Codex_
