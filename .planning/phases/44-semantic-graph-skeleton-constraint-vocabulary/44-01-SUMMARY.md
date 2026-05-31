---
phase: 44-semantic-graph-skeleton-constraint-vocabulary
plan: 01
subsystem: api
tags: [semantic-graph, rust, byte-stability, closed-enum, adjacency-index, points-to]

# Dependency graph
requires:
  - phase: 43-reachability-and-root-semantics
    provides: reachability/ analog module (closed-enum byte-stability + normalized()/store indexes template)
  - phase: 36-p0-type-value-place-alias-substrate
    provides: points_to fact family + PointsToPrecision/PointsToConstraintKind shapes
provides:
  - Private analysis::semantic_graph module skeleton (all pub(crate))
  - Closed NodeKind (7 payload-carrying variants composing existing v1.2 IDs) and EdgeKind (4 Copy variants)
  - SemanticNodeFact / SemanticEdgeFact fact families with serde-skip dense id + precision + stable_key
  - Run-local dense SemanticNodeId / SemanticEdgeId newtypes
  - SemanticGraphOutput::normalized (stable-key sort then dense-ID assignment) + SemanticGraphStore with nodes-by-kind, edges-by-kind, outgoing + incoming adjacency indexes
  - SEMANTIC_GRAPH_PROVIDER_ID = "polint.semantic_graph"
affects: [44-02-constraint-vocabulary, 44-03-provider-validation-fixtures, 47-unified-call-graph-solver]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Payload-carrying closed enum: pinned declaration order + derived Ord + serde rename + as_str() label + exhaustive-variant/pinned-order lock tests, NO #[repr(u8)]"
    - "Dense IDs assigned only after stable-key sort; edge endpoints remapped during normalization to stay consistent with re-densified node numbering"
    - "Both adjacency directions (outgoing keyed by source, incoming keyed by target) built in one post-normalization edge pass"

key-files:
  created:
    - crates/polint/src/analysis/semantic_graph/mod.rs
    - crates/polint/src/analysis/semantic_graph/facts.rs
    - crates/polint/src/analysis/semantic_graph/store.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/mod.rs

key-decisions:
  - "Module node composes core::ModuleNodeId (PATTERNS V3 correction), not a non-existent ModuleId"
  - "Closed enums use pinned-order + serde-rename + as_str() + lock tests, never #[repr(u8)] (PATTERNS V2 correction)"
  - "Added a SemanticPrecision enum (mirroring PointsToPrecision shape, no Exact ceiling) as the node/edge precision field; Exact-precision rejection is deferred to Plan 03 validation per D-07"
  - "SemanticGraphOutput carries only nodes+edges; the constraints field is deferred to Plan 02 to avoid pinning that contract prematurely"
  - "normalized() remaps edge source/target handles to the post-sort dense node numbering so adjacency stays consistent after re-densification"

patterns-established:
  - "Pattern 1: Semantic graph node/edge fact shape — serde-skip dense id + composed-identity kind + precision + stable_key"
  - "Pattern 2: from_output referential validation rejects dangling edge endpoints with AnalysisError::InvalidFact, mirroring reachability/store.rs"

requirements-completed: [GRAPH-01]

# Metrics
duration: 18min
completed: 2026-05-30
---

# Phase 44 Plan 01: Semantic Graph Skeleton Summary

**Private byte-stable semantic-graph skeleton: closed 7-variant NodeKind / 4-variant EdgeKind taxonomies composing existing v1.2 identities, node/edge fact families, dense SemanticNodeId/SemanticEdgeId, and a SemanticGraphStore with stable-key-sorted dense IDs plus outgoing and incoming adjacency indexes.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-05-30T10:03:00Z
- **Completed:** 2026-05-30T10:21:00Z
- **Tasks:** 3
- **Files modified:** 5 (3 created, 2 modified)

## Accomplishments
- Registered the private `analysis::semantic_graph` module with the mandatory D-09 naming-collision guard distinguishing the unified `ConstraintKind` (Plan 02) from `points_to::PointsToConstraintKind` (no merge/rename until Phase 47).
- Closed `NodeKind` (7 payload-carrying variants composing `FunctionId`/`CallSiteId`/`ScopeId`/`PlaceId`/`ObjectTokenId`/`ModuleNodeId`/`PackageId`) and `EdgeKind` (4 `Copy` variants) with exhaustive-variant and pinned-declaration-order lock tests; no `#[repr(u8)]`.
- `SemanticGraphOutput::normalized()` sorts by `(stable_key, id)`, remaps edge endpoints, then assigns dense IDs by index (D-05); `SemanticGraphStore::from_output` builds nodes-by-kind, edges-by-kind, outgoing (source) and incoming (target) adjacency indexes (D-14) and rejects dangling edge endpoints.
- Dense `SemanticNodeId`/`SemanticEdgeId` newtypes added with the full derive set (incl. `Default` for the `#[serde(skip)]` digest discipline) and covered by the small-id contract test.

## Task Commits

Each task was committed atomically:

1. **Task 1: SemanticNodeId/SemanticEdgeId newtypes + module registration** - `5f30142c` (feat)
2. **Task 2: NodeKind/EdgeKind taxonomies + node/edge fact families + D-09 guard** - `1baa9fb9` (feat)
3. **Task 3: SemanticGraphOutput::normalized + SemanticGraphStore indexes** - `30f3a803` (feat)

_Note: TDD-style tasks were implemented as single feat commits because the three files share a compilation unit (the module declaration in Task 1 requires the submodules to exist); tests and implementation were authored and verified together per task._

## Files Created/Modified
- `crates/polint/src/analysis/semantic_graph/mod.rs` - Module declaration + D-09 naming-collision guard doc comment; declares `facts` and `store` submodules.
- `crates/polint/src/analysis/semantic_graph/facts.rs` - `NodeKind`, `EdgeKind`, `SemanticPrecision`, `SemanticNodeFact`, `SemanticEdgeFact` with as_str labels and lock tests.
- `crates/polint/src/analysis/semantic_graph/store.rs` - `SemanticGraphOutput::normalized`, `SemanticGraphStore` with four BTreeMap index sidecars, referential validation, `SEMANTIC_GRAPH_PROVIDER_ID`.
- `crates/polint/src/analysis/ids.rs` - Added `SemanticNodeId`/`SemanticEdgeId` newtypes + contract-test registration.
- `crates/polint/src/analysis/mod.rs` - Registered `pub(crate) mod semantic_graph;`.

## Decisions Made
- Composed `core::ModuleNodeId` for the `Module` node variant (PATTERNS V3); there is no `ModuleId` type.
- Followed the established closed-enum byte-stability convention (pinned order + serde rename + `as_str()` + lock tests); added no `#[repr(u8)]` (PATTERNS V2).
- Introduced a `SemanticPrecision` enum (same shape family as `PointsToPrecision`, no `Exact` variant) as the node/edge precision field; the Exact-precision ceiling is enforced in Plan 03 validation per D-07.
- `SemanticGraphOutput` carries only `nodes` + `edges`; the `constraints` field is deferred to Plan 02 (documented in a comment) so this plan does not pin Plan 02's contract.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Edge-endpoint remap during normalization**
- **Found during:** Task 3 (SemanticGraphOutput::normalized)
- **Issue:** The plan specified sorting nodes/edges and reassigning dense node IDs by index, but did not state that edge `source`/`target` handles (which point at the pre-sort dense node IDs) must be rewritten to the post-sort node numbering. Without this, edges would reference stale node IDs after re-densification, silently corrupting adjacency and breaking referential validation.
- **Fix:** Built a `BTreeMap<SemanticNodeId, SemanticNodeId>` remap during node densification and rewrote every edge's `source`/`target` to the new node IDs before sorting/densifying edges. Verified by `incoming_adjacency_is_built_and_consistent_with_outgoing` and `normalized_is_shuffle_stable`.
- **Files modified:** crates/polint/src/analysis/semantic_graph/store.rs
- **Verification:** All 16 semantic_graph tests pass; shuffle-stability test confirms byte-identical output and identical dense IDs under row shuffle.
- **Committed in:** `30f3a803` (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical correctness requirement)
**Impact on plan:** The remap is required for correctness of the adjacency indexes the Phase 47 solver will traverse. No scope creep — still nodes/edges only, no constraints field added.

## Issues Encountered
- A test-only assertion error in `normalized_assigns_dense_ids_after_stable_key_sort` (I initially asserted the wrong sort order: `"node|callsite|z"` sorts before `"node|function|a"` because `'c' < 'f'`). Corrected the expected ordering; this was a test-expectation fix, not a production-code change.

## Threat Surface
- T-44-01-01 (Information Disclosure via SDK leak): mitigated — every new type is `pub(crate)`; `ALLOWED_PRELUDE` was NOT extended; the `public_surface_leak` gate passes (5/5 tests).
- T-44-01-03 (Tampering via non-deterministic dense IDs): mitigated — dense IDs assigned only after stable-key sort; `normalized_is_shuffle_stable` enforces byte-identical output under shuffle.
- No new package-manager installs (T-44-01-SC): no new dependencies added.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The node/edge skeleton, dense IDs, and store indexes are ready for Plan 02 to add the `ConstraintKind` vocabulary + `SemanticConstraintId` and the `constraints` field on `SemanticGraphOutput`.
- Plan 03 will add the provider/cache-key/validation wiring (incl. the Exact-precision ceiling rejection that D-07 reserves) and eval fixtures.
- No blockers.

---
*Phase: 44-semantic-graph-skeleton-constraint-vocabulary*
*Completed: 2026-05-30*

## Self-Check: PASSED

- FOUND: crates/polint/src/analysis/semantic_graph/mod.rs
- FOUND: crates/polint/src/analysis/semantic_graph/facts.rs
- FOUND: crates/polint/src/analysis/semantic_graph/store.rs
- FOUND commit: 5f30142c (Task 1)
- FOUND commit: 1baa9fb9 (Task 2)
- FOUND commit: 30f3a803 (Task 3)
