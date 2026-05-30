---
phase: 44-semantic-graph-skeleton-constraint-vocabulary
plan: 02
subsystem: api
tags: [semantic-graph, rust, constraint-vocabulary, closed-enum, byte-stability, points-to, projection]

# Dependency graph
requires:
  - phase: 44-semantic-graph-skeleton-constraint-vocabulary
    provides: 44-01 node/edge skeleton (NodeKind/EdgeKind, SemanticGraphOutput::normalized, SemanticGraphStore indexes, dense IDs)
  - phase: 36-p0-type-value-place-alias-substrate
    provides: points_to::PointsToConstraintFact shape (D-10 mirror target) + PointsToStatus/PointsToPrecision field-type vocabulary
provides:
  - Closed 7-variant ConstraintKind vocabulary (CopyEdge, Alloc, FieldLoad, FieldStore, CallConstraint, ModelEdge, TypeConstraint) composing SemanticNodeId / TypeFactId
  - ConstraintFact family mirroring PointsToConstraintFact exactly (id, kind, status, precision, stable_key)
  - SemanticConstraintId run-local dense newtype (Default for serde-skip digest discipline)
  - SemanticGraphOutput.constraints field + constraint normalization (node-ref remap, stable-key sort, dense reassignment)
  - SemanticGraphStore constraints-by-ConstraintKind index + constraint referential validation
  - build_semantic_graph: read-only projection emitting real-but-minimal nodes/edges/constraints from existing v1.2 facts
affects: [44-03-provider-validation-fixtures, 47-unified-call-graph-solver, 49-adaptation-model-layer]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Payload-carrying closed enum (ConstraintKind): pinned order + derived Ord + serde rename + as_str() + exactly-N-variant + pinned-order lock tests, NO #[repr(u8)]"
    - "ConstraintFact mirrors PointsToConstraintFact shape and reuses its status/precision field-type enums (D-10) without merging the constraint-kind enums (D-09)"
    - "Constraint node refs remapped during normalization (same discipline as edge endpoints) so payloads track post-sort dense node numbering"
    - "build_semantic_graph: intern-by-stable-key node table + read-only projection from db, honest zero-emission for kinds lacking an honest endpoint bridge"

key-files:
  created:
    - crates/polint/src/analysis/semantic_graph/constraints.rs
    - crates/polint/src/analysis/semantic_graph/build.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/semantic_graph/store.rs
    - crates/polint/src/analysis/semantic_graph/mod.rs

key-decisions:
  - "ConstraintFact reuses points_to::PointsToStatus/PointsToPrecision as its status/precision field types (D-10) rather than inventing a redundant enum; D-09 keeps ConstraintKind and PointsToConstraintKind separate with no import/merge"
  - "ModelEdge variant is fieldless (no producer/payload until Phase 49) and emits zero constraints"
  - "build_semantic_graph emits zero Alloc/FieldLoad/FieldStore/TypeConstraint in this minimal pass: each lacks an honest endpoint bridge (AllocationTokenId != ObjectTokenId; access paths carry no destination place), so emitting would fabricate nodes — deferred rather than inflate recall (D-07)"
  - "Constraint node references are remapped during normalize() to the post-sort dense node numbering, mirroring the edge-endpoint remap from 44-01"
  - "Graph rows use Conservative (node/edge) and FlowInsensitive (constraint) precision; the exact ceiling is rejected (validated in Plan 03)"

patterns-established:
  - "Pattern: closed payload-carrying constraint vocabulary with conceptual-map comment to a sibling sub-domain enum and NO code coupling (D-09)"
  - "Pattern: read-only fact-projection builder (intern-by-stable-key) that mutates no upstream family and defers dishonest emissions explicitly"

requirements-completed: [GRAPH-02]

# Metrics
duration: 13min
completed: 2026-05-30
---

# Phase 44 Plan 02: Constraint Vocabulary & Graph Population Summary

**Closed 7-variant `ConstraintKind` vocabulary + `ConstraintFact` family mirroring `PointsToConstraintFact`, `SemanticConstraintId` dense newtype, constraint carrying/indexing/validation in the graph store, and a `build_semantic_graph` read-only projection that emits a real-but-minimal graph (nodes + Call/MemberOf edges + Copy/Call constraints, zero ModelEdge) from already-available v1.2 facts.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-05-30T10:18:14Z
- **Completed:** 2026-05-30T10:31:00Z
- **Tasks:** 3
- **Files modified:** 5 (2 created, 3 modified)

## Accomplishments
- Defined the closed `ConstraintKind` enum (exactly 7 roadmap variants in pinned order, NO `#[repr(u8)]`) with `as_str()` labels and `constraint_kind_has_exactly_7_variants` + pinned-order lock tests; variants reference `SemanticNodeId` / `TypeFactId`, never run-local IDs of other families.
- `ConstraintFact` mirrors `points_to::PointsToConstraintFact` exactly (D-10) and reuses `PointsToStatus`/`PointsToPrecision` as field types; the D-09 conceptual map to `PointsToConstraintKind` is documented in a comment with **no** code coupling (`grep -c "use.*points_to::.*solver"` returns 0).
- Added `SemanticConstraintId` newtype (with `Default` for the `#[serde(skip)]` digest discipline) registered in `assert_small_id_contract`.
- Extended `SemanticGraphOutput` with `constraints: Vec<ConstraintFact>`; `normalized()` remaps each constraint's node references to the post-sort node numbering, then sorts by `(stable_key, id)` and reassigns dense `SemanticConstraintId` by index. `SemanticGraphStore::from_output` builds a constraints-by-`ConstraintKind`-tag index (D-14) and referentially validates every `SemanticNodeId` a constraint references (dangling -> `AnalysisError::InvalidFact`).
- Implemented `build_semantic_graph(db)`: a read-only projection that emits Function/Package/Scope/Callsite/Place nodes (stable keys composed from referenced identity via the length-prefixed recipe, D-06), `Call` edges + `CallConstraint`s from call sites, `MemberOf` edges from scope-in-package containment, and `CopyEdge` constraints from value `PlaceRef`/`CallReturn` facts — emitting **zero** `ModelEdge` (and zero `Alloc`/`FieldLoad`/`FieldStore`/`TypeConstraint`) with documented honest-emptiness rationale, and mutating no upstream fact family (D-13).

## Task Commits

1. **Task 1: ConstraintKind vocabulary + ConstraintFact + SemanticConstraintId** — `d81451fb` (feat)
2. **Task 2: carry and index constraints in SemanticGraphOutput/Store** — `97db0e25` (feat)
3. **Task 3: build_semantic_graph real-but-minimal projection** — `1523c247` (feat)

_Note: the `tdd="true"` tasks were authored as single `feat` commits because the constraints/store/build files share a compilation unit (the store/build references the enum from Task 1); tests and implementation were written and verified together per task._

## Files Created/Modified
- `crates/polint/src/analysis/semantic_graph/constraints.rs` — `ConstraintKind` (7 closed variants), `ConstraintFact`, `as_str()`, D-09 conceptual-map doc, lock tests.
- `crates/polint/src/analysis/semantic_graph/build.rs` — `build_semantic_graph` + `GraphBuilder` (intern-by-stable-key projection), stable-key composition helpers, fixture-db tests.
- `crates/polint/src/analysis/ids.rs` — `SemanticConstraintId` newtype + contract-test registration.
- `crates/polint/src/analysis/semantic_graph/store.rs` — `constraints` field, constraint normalization (node-ref remap + sort + dense reassign), constraints-by-kind index, constraint referential validation, accessors, new tests.
- `crates/polint/src/analysis/semantic_graph/mod.rs` — registered `pub(crate) mod build;` and `pub(crate) mod constraints;`.

## Decisions Made
- Reused `points_to::PointsToStatus`/`PointsToPrecision` as the `ConstraintFact` status/precision field types (D-10) rather than inventing a redundant enum; kept `ConstraintKind` and `PointsToConstraintKind` separate with no import/merge (D-09; folding deferred to Phase 47).
- Made `ModelEdge` fieldless (no payload/producer until Phase 49) and emitted zero rows.
- Deferred `Alloc`/`FieldLoad`/`FieldStore`/`TypeConstraint` emission honestly (see Known Stubs) instead of fabricating endpoints to inflate recall (D-07).
- Remapped constraint node references during `normalized()` to the post-sort dense node numbering, mirroring the 44-01 edge-endpoint remap.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Constraint node-reference remap during normalization**
- **Found during:** Task 2 (`SemanticGraphOutput::normalized`)
- **Issue:** The plan specified sorting+dense-reassigning constraints like nodes/edges, but did not state that each constraint payload's `SemanticNodeId` references (which point at pre-sort node IDs) must be rewritten to the post-sort node numbering. Without this, a constraint's node references would dangle/misreference after node re-densification, silently corrupting the constraints-by-kind index and breaking referential validation — exactly the gap 44-01 hit for edge endpoints.
- **Fix:** Added `remap_constraint_nodes` to rewrite every constraint payload `SemanticNodeId` through the node-densification remap before sorting/densifying constraints, and `constraint_referenced_nodes` for the validation pass. Verified by `normalized_constraints_are_shuffle_stable`, `from_output_builds_constraints_by_kind_index` (asserts the remapped callsite), and `from_output_rejects_dangling_constraint_node_ref`.
- **Files modified:** crates/polint/src/analysis/semantic_graph/store.rs
- **Committed in:** `97db0e25` (Task 2 commit)

**2. [Rule 1 - Bug / D-07 honesty] Type-mismatched Alloc projection corrected to honest deferral**
- **Found during:** Task 3 (`project_value_constraints`)
- **Issue:** The initial build attempted to project `Alloc` constraints from `ValueFact` allocations, but `NodeKind::AbstractObject` wraps `ObjectTokenId` while value allocations carry `AllocationTokenId` (a distinct identity family) — a compile error and, more importantly, a dishonest endpoint mapping.
- **Fix:** Dropped the `Alloc` projection and kept only the fully-honest `CopyEdge` projection (Place-to-Place from `PlaceRef`/`CallReturn`). Documented `Alloc`/`FieldLoad`/`FieldStore`/`TypeConstraint` as deferred honest-emptiness (no fabricated endpoints), consistent with the `ModelEdge` reservation and D-07.
- **Files modified:** crates/polint/src/analysis/semantic_graph/build.rs
- **Committed in:** `1523c247` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 missing critical correctness requirement, 1 bug/honesty correction).
**Impact on plan:** Both keep the graph honest and referentially sound for the Phase 47 solver. The targeted acceptance — closed 7-variant vocabulary, `ConstraintFact` mirroring shape, `build_semantic_graph` emitting >=1 Call edge, >=1 CallConstraint, exactly 0 ModelEdge — is met.

## Known Stubs

These constraint kinds are intentionally **zero-emission** in this minimal population pass. Each is documented honest emptiness, NOT a silent stub, and each is resolved by a named future plan. The vocabulary and node projection are in place so a later plan can add emission without re-keying.

| Kind | File | Reason | Resolved by |
|------|------|--------|-------------|
| `ModelEdge` | build.rs | No producer exists until the adaptation-model layer | Phase 49 (ADAPT-01) |
| `Alloc` | build.rs | `ValueFact` allocations carry `AllocationTokenId`; `NodeKind::AbstractObject` needs `ObjectTokenId` — no honest 1:1 bridge at this layer | later v1.3 plan wiring the object-token bridge |
| `FieldLoad` / `FieldStore` | build.rs | `AccessPathFact` expresses `base.field` but carries no distinct destination place identity; emitting would fabricate a destination node | later v1.3 field-flow plan |
| `TypeConstraint` | build.rs | Not exercised in this minimal pass; the variant + `TypeFactId` reference are defined and ready | later v1.3 type-bridge plan |

These stubs do not block GRAPH-02: the requirement is the closed vocabulary + a real-but-minimal graph proving the vocabulary end-to-end, which the Call edges/constraints and Copy constraints satisfy. Plan 03 asserts the full Go/TS snapshot fixtures.

## Threat Surface
- T-44-02-01 (Tampering / accidental points-to merge): mitigated — `ConstraintKind` documents the conceptual map to `PointsToConstraintKind` with no code coupling; `grep -c "use.*points_to::.*solver" constraints.rs` returns 0; only `PointsToStatus`/`PointsToPrecision` field types are imported.
- T-44-02-02 (Information Disclosure / SDK leak): mitigated — every new type (`ConstraintKind`, `ConstraintFact`, `SemanticConstraintId`, `build_semantic_graph`) is `pub(crate)`; `ALLOWED_PRELUDE` was not extended.
- T-44-02-04 (Spoofing / fabricated edges): mitigated — `ModelEdge` (and `Alloc`/`Field*`/`TypeConstraint`) emit zero; edges/constraints are emitted only where existing facts express them; the build test asserts exactly 0 ModelEdge and end-to-end referential validity.
- T-44-02-SC (package installs): n/a — no new package-manager installs.

## User Setup Required
None — no external service configuration required.

## Next Phase Readiness
- The constraint vocabulary, store carrying/indexing/validation, and `build_semantic_graph` are ready for Plan 03's provider/cache-key/validation wiring (incl. the Exact-precision ceiling rejection) and the Go/TS snapshot fixtures with real constraints to assert.
- No blockers.

---
*Phase: 44-semantic-graph-skeleton-constraint-vocabulary*
*Completed: 2026-05-30*

## Self-Check: PASSED

- FOUND: crates/polint/src/analysis/semantic_graph/constraints.rs
- FOUND: crates/polint/src/analysis/semantic_graph/build.rs
- FOUND: crates/polint/src/analysis/semantic_graph/store.rs (modified)
- FOUND: crates/polint/src/analysis/ids.rs (modified)
- FOUND: .planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-02-SUMMARY.md
- FOUND commit: d81451fb (Task 1)
- FOUND commit: 97db0e25 (Task 2)
- FOUND commit: 1523c247 (Task 3)
