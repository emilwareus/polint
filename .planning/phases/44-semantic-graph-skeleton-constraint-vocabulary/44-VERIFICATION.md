---
phase: 44-semantic-graph-skeleton-constraint-vocabulary
verified: 2026-05-30T00:00:00Z
status: passed
score: 4/4 success criteria + 16/16 plan must-have truths verified
overrides_applied: 0
re_verification:
  previous_status: none
  note: initial verification
---

# Phase 44: Semantic Graph Skeleton & Constraint Vocabulary Verification Report

**Phase Goal:** polint has a private shared semantic graph with stable identities, typed edges, and a closed constraint vocabulary that language frontends emit into — the architectural keystone for the unified solver.
**Verified:** 2026-05-30
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Private `analysis::semantic_graph` with typed `NodeKind` (7 kinds), `EdgeKind` (4 kinds), outgoing/incoming/by-kind indexes, validation, provider manifest, cache key | VERIFIED | `facts.rs:27-35` NodeKind = 7 payload-carrying variants composing `FunctionId/CallSiteId/ScopeId/PlaceId/ObjectTokenId/ModuleNodeId/PackageId`; `facts.rs:63-68` EdgeKind = Call/MemberOf/Alloc/Flow; `store.rs:147-154` four BTreeMap indexes (`nodes_by_kind`, `edges_by_kind`, `constraints_by_kind`, `outgoing`, `incoming`); `validate.rs:23` `validate_semantic_graph`; `provider.rs:627` manifest `polint.semantic_graph`; `cache_key.rs:4` schema label. Module is `pub(crate) mod semantic_graph` (`analysis/mod.rs:29`). 39 lib tests pass. |
| 2 | Constraint vocabulary is a closed 7-variant enum with snapshot fixtures asserting emitted shapes | VERIFIED | `constraints.rs:43-80` `ConstraintKind` = CopyEdge/Alloc/FieldLoad/FieldStore/CallConstraint/ModelEdge/TypeConstraint, pinned order, `constraint_kind_has_exactly_7_variants` lock test passes; Go + TS snapshot fixtures (`tests/eval-fixtures/semantic-graph/{go_graph,ts_graph}/`) assert >=1 node/edge/constraint, >=1 Call edge + CallConstraint, byte-stable total-ordered output — 4 snapshot tests pass. |
| 3 | Dependency index for shared-graph cache layer designed, listing every contributing input; deferred inputs reserved/documented | VERIFIED | `cache_key.rs:19-45` self-documents present-now vs deferred (MIR/CFG/summaries → Phase 47, adaptation models → Phase 49, solver budgets → Phase 51/53), zero deferred inputs digested; `provider.rs:626-651` manifest `inputs` slice mirrors the note with 11 present families. Provider output digest folds 8 upstream provider digests + schema + parameters (`provider.rs:44-71`, D-17). Deferral is intentional and documented per CONTEXT D-11 / SC3. |
| 4 | Public-boundary proof: `analysis::semantic_graph` + constraint enum stay `pub(crate)`, never reachable from `polint::sdk::prelude::*`; leak gate green | VERIFIED | All types `pub(crate)` (grep: no `pub ` non-crate items); `public_surface_leak.rs` unmodified, ALLOWED_PRELUDE = 97 entries unchanged (no semantic_graph types added); `cargo test -p polint --test public_surface_leak` = 5/5 pass; defensive cross-check rejects any `pub use crate::analysis::` in prelude. |

**Score:** 4/4 success criteria verified.

### Plan Must-Have Truths

| Plan | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 44-01 | Private module, all `pub(crate)`, registered in analysis/mod.rs | VERIFIED | `analysis/mod.rs:29`, mod.rs D-09 guard present |
| 44-01 | NodeKind 7-variant + EdgeKind 4-variant, byte-stable pinned order | VERIFIED | `node_kind_has_exactly_7_variants`, `edge_kind_has_exactly_4_variants`, `edge_kind_sorts_in_pinned_declaration_order` all pass |
| 44-01 | Each NodeKind composes existing identity newtype (D-04) | VERIFIED | `node_kind_composes_existing_identity_newtypes` passes; `Module(ModuleNodeId)` not `ModuleId` |
| 44-01 | Dense SemanticNodeId/SemanticEdgeId after stable-key sort | VERIFIED | `ids.rs` newtypes + contract test; `normalized_assigns_dense_ids_after_stable_key_sort` |
| 44-01 | stable_key from length-prefixed labeled-parts recipe | VERIFIED | `build.rs` composes via `semantic_stable_key`/`stable_key_from_parts`, never dense IDs |
| 44-01 | Store builds nodes-by-kind/edges-by-kind/outgoing/incoming after normalization (D-14) | VERIFIED | `store.rs:147-154`; `incoming_adjacency_is_built_and_consistent_with_outgoing` passes |
| 44-02 | ConstraintKind closed 7-variant enum (D-08) | VERIFIED | `constraints.rs:43-80` + lock test |
| 44-02 | ConstraintFact mirrors PointsToConstraintFact `{id,kind,status,precision,stable_key}` (D-10) | VERIFIED | `constraints.rs:112-121`; `constraint_fact_mirrors_points_to_shape` passes |
| 44-02 | D-09 separation from PointsToConstraintKind, no merge/import | VERIFIED | `constraints.rs:21-40` conceptual-map doc; imports only PointsToStatus/PointsToPrecision field types |
| 44-02 | Constraints emitted from existing v1.2 facts + Call/MemberOf/Alloc edges (D-11) | VERIFIED | `build.rs:46` `build_semantic_graph` reads db; emits Call edges, CallConstraint, MemberOf, CopyEdge |
| 44-02 | ModelEdge reserved, zero emitted, documented (D-11) | VERIFIED | `build.rs:485-487` test asserts 0 ModelEdge; `ModelEdge` fieldless reserved variant |
| 44-02 | Build mutates no upstream fact family (D-13) | VERIFIED | `build_is_read_only_and_deterministic` passes |
| 44-03 | Provider registered between type_value_alias and refined_calls (D-16) | VERIFIED | `provider.rs:627,816,846,903,1293` (manifest + 3 order vectors + report row); provider order tests pass |
| 44-03 | Provider stores facts, cache key digests every consumed output + version, empty-output sentinel (D-17) | VERIFIED | `provider.rs:44-71` folds 8 upstream digests; `empty_db_produces_empty_output_sentinel_digest`, `upstream_digest_change_invalidates_output_digest` pass |
| 44-03 | validate() rejects dangling endpoints, duplicate keys, Exact precision (D-15) | VERIFIED | `validate.rs` checks all three; `exact_equivalent_precision_node_is_rejected` passes |
| 44-03 | Determinism gate + public-surface-leak gate stay green (D-18) | VERIFIED | determinism_gate 6/6, public_surface_leak 5/5 |

### Required Artifacts

| Artifact | Status | Details |
| --- | --- | --- |
| `semantic_graph/facts.rs` | VERIFIED | NodeKind/EdgeKind/SemanticPrecision/fact families + lock tests |
| `semantic_graph/constraints.rs` | VERIFIED | ConstraintKind 7-variant + ConstraintFact + D-09 map |
| `semantic_graph/store.rs` | VERIFIED | normalized() + 4 indexes + referential validation |
| `semantic_graph/build.rs` | VERIFIED | build_semantic_graph read-only projection |
| `semantic_graph/provider.rs` | VERIFIED | derive_semantic_graph_with_cache_stats + digest |
| `semantic_graph/cache_key.rs` | VERIFIED | SEMANTIC_GRAPH_SCHEMA_LABEL + SC3 doc + lock tests |
| `semantic_graph/validate.rs` | VERIFIED | validate_semantic_graph structural + precision ceiling |
| `tests/eval-fixtures/semantic-graph/{go,ts}_graph/` | VERIFIED | both fixtures exist, schema_version present |
| `analysis/ids.rs` | VERIFIED | SemanticNodeId/EdgeId/ConstraintId newtypes contract-tested |

### Key Link Verification

| From | To | Status | Details |
| --- | --- | --- | --- |
| `analysis_kernel/mod.rs` | `derive_semantic_graph_with_cache_stats` | WIRED | `mod.rs:522-543` run splice between type_value_alias and refined_calls |
| `analysis_kernel/provider.rs` | provider order vectors | WIRED | `polint.semantic_graph` in all 3 vectors + report row |
| `analysis_kernel/validation.rs` | `validate_semantic_graph` | WIRED | `validation.rs:66` called in-sequence |
| `build.rs` | calls/values/access_paths facts | WIRED | reads db, projects edges/constraints, no mutation |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| semantic_graph lib tests | `cargo test -p polint --lib analysis::semantic_graph` | 39 passed | PASS |
| id contract | `cargo test -p polint --lib analysis::ids` | 2 passed | PASS |
| snapshot fixtures emit each kind, byte-stable | `cargo test -p polint --lib eval::semantic_graph_snapshot` | 4 passed | PASS |
| determinism gate (auto-enrolled) | `cargo test -p polint --lib determinism_gate` | 6 passed | PASS |
| provider order | `cargo test -p polint --lib analysis_kernel::provider` | 12 passed | PASS |
| public-surface-leak gate | `cargo test -p polint --test public_surface_leak` | 5 passed | PASS |
| full regression | `cargo test -p polint` | 1756 + 140 + 5 passed, 0 failed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| GRAPH-01 | 44-01, 44-03 | Private semantic_graph with NodeKind/EdgeKind, indexes, validation, manifest, cache key | SATISFIED | SC1, SC3, SC4 all VERIFIED |
| GRAPH-02 | 44-02, 44-03 | Constraint vocabulary frontends emit into; emission verified by snapshot fixtures | SATISFIED | Closed 7-variant ConstraintKind defined + byte-stable; Go/TS snapshot fixtures assert real CopyEdge/CallConstraint/Call-edge emission; zero-emission of Alloc/Field*/Type/ModelEdge is documented honest deferral aligned with SC3 reservations — see assessment below |

Both GRAPH-01 and GRAPH-02 are mapped to Phase 44 only in REQUIREMENTS.md and are accounted for. No orphaned requirements.

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
| --- | --- | --- | --- |
| (none) | TBD/FIXME/XXX scan across all modified files | — | NONE FOUND |

Zero-emission constraint kinds (Alloc/FieldLoad/FieldStore/TypeConstraint/ModelEdge) are NOT stubs: each is a fully-defined closed-enum variant with documented honest-emptiness rationale and a named resolving phase. They do not flow empty data to user-visible output dishonestly — the snapshot fixtures assert only the kinds that emit today, and the vocabulary itself is complete and lock-tested.

## GRAPH-02 Assessment (zero-emission deferral)

GRAPH-02 requires the constraint vocabulary be **defined** and that **constraint emission is verified by snapshot fixtures**. Both hold:

- The closed 7-variant `ConstraintKind` is fully defined, byte-stable, pinned-ordered, and lock-tested (exhaustive-match compile guard).
- Snapshot fixtures (Go + TS) assert real, byte-stable constraint emission: Call edges, CallConstraint, and CopyEdge constraints are emitted and verified.

The 44-02 executor's honest deferral of `Alloc`/`FieldLoad`/`FieldStore`/`TypeConstraint`/`ModelEdge` to zero emission (existing facts carry mismatched identity families — `AllocationTokenId` != `ObjectTokenId`; access paths lack a destination place; no model producer until Phase 49) is the correct D-07 honesty discipline — emitting would fabricate endpoints and inflate recall. The requirement does not mandate non-zero emission of every variant in this phase; it mandates a defined vocabulary frontends emit into, with emission snapshot-verified for what is honestly producible. This is consistent with SC3's documented reservation of producer-less inputs to Phases 47/49/50/51/53. **GRAPH-02 is satisfied, not a gap.**

### Gaps Summary

None. All 4 ROADMAP success criteria and all 16 plan must-have truths are substantively implemented, wired into the kernel, and behaviorally verified by passing tests. The full `cargo test -p polint` regression is green (1756 lib + 140 integration + 5 leak-gate, 0 failures), confirming the orchestrator's provider-manifest snapshot fix landed. The public-surface-leak gate and Phase 43 determinism gate both stay green.

---

_Verified: 2026-05-30_
_Verifier: Claude (gsd-verifier)_
