# Phase 44: Semantic Graph Skeleton & Constraint Vocabulary - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-30
**Phase:** 44-Semantic Graph Skeleton & Constraint Vocabulary
**Mode:** `/gsd:discuss-phase 44 --auto` (autonomous single-pass; Claude selected recommended option for every gray area)
**Areas discussed:** Module/node/edge model & identity, Constraint vocabulary design, Population strategy & snapshot fixtures, Indexes/validation/provider/cache-key, Determinism gate inheritance

---

## Semantic Graph Module, Node/Edge Model & Identity (GRAPH-01)

| Option | Description | Selected |
|--------|-------------|----------|
| New private `analysis::semantic_graph` with closed `NodeKind`/`EdgeKind` enums, nodes composing existing v1.2 IDs | Mirrors Phase 43 D-03 composition + Phase 42 closed-enum byte-stability discipline | ✓ |
| Extend an existing module (e.g. `analysis::calls` or `points_to`) with graph types | Couples the shared core to one sub-domain; rejected | |
| Invent parallel function/callsite/place identities for graph nodes | Duplicates identity, breaks determinism/cache discipline; rejected | |

**Auto-selected:** New `analysis::semantic_graph` module; `NodeKind` = {Function, Callsite, Scope, Place, AbstractObject, Module, Package}, `EdgeKind` = {Call, MemberOf, Alloc, Flow}, both closed `#[repr(u8)]` enums; nodes reference existing IDs (`FunctionId`, `CallSiteId`, `ScopeId`, `PlaceId`, `ObjectTokenId`, module/package IDs); new `SemanticNodeId`/`SemanticEdgeId` dense newtypes assigned after stable-key sort.
**Notes:** D-01..D-07. Directly applies Phase 42 `IdentityCategory` and Phase 43 `RootKind` precedents.

---

## Constraint Vocabulary Design (GRAPH-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Closed `ConstraintKind` enum (7 roadmap variants) as a unified frontend-facing vocabulary layered *above* existing `points_to::PointsToConstraintKind` | Preserves layered architecture; points-to folding deferred to Phase 47 | ✓ |
| Merge/rename `points_to::PointsToConstraintKind` into the new vocabulary now | Couples Phase 44 to Phase 47's solver-folding (GRAPH-03); rejected | |
| Open/non-exhaustive constraint enum | Breaks serde + `Ord` byte-stability; rejected | |

**Auto-selected:** Closed `ConstraintKind` = {CopyEdge, Alloc, FieldLoad, FieldStore, CallConstraint, ModelEdge, TypeConstraint}; payloads reference semantic-graph node / existing fact IDs; stored as a `ConstraintFact` family shaped like `PointsToConstraintFact`. Mandatory top-of-module doc comment distinguishing the unified vocabulary from the points-to sub-domain language.
**Notes:** D-08..D-10. Naming-collision guard mirrors Phase 43 D-02.

---

## Population Strategy & Snapshot Fixtures (GRAPH-01 + GRAPH-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Emit a real-but-minimal graph from already-available facts (calls, values, access paths); reserve `ModelEdge` empty; Go + TS/JS snapshot fixtures | Proves vocabulary end-to-end now; honest emptiness for unbuilt producers | ✓ |
| Type-only stub, defer all emission to Phases 45/46 | Fails GRAPH-02 "constraint emission verified by snapshot fixtures"; rejected | |
| Fake a `ModelEdge` producer to fill the variant | Dishonest; adaptation producer is Phase 49; rejected | |

**Auto-selected:** Emit nodes/`Call` edges/`CallConstraint` from `analysis::calls`; `Alloc`/`CopyEdge`/`FieldLoad`/`FieldStore` from `values`/`access_paths`; `member-of` from containment; zero `ModelEdge` (reserved). Snapshots under `tests/eval-fixtures/semantic-graph/`, normalized/total-ordered, byte-identical cross-platform. Graph derived/aggregated, never mutating sources.
**Notes:** D-11..D-13.

---

## Indexes, Validation, Provider Manifest & Cache Key (GRAPH-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Indexes (by NodeKind/EdgeKind/adjacency/ConstraintKind) + `validate()` pass + `polint.semantic_graph` provider after `type_value_alias`/before `refined_calls` + v1.2 digest-recipe cache key | Forward-compatible with Phase 47 GRAPH-05 refined-calls rework; matches existing provider discipline | ✓ |
| Slot the provider at the very end (after metrics) | Loses forward-compatibility with the refined-calls projection point; rejected as default | |
| Skip validation / indexes in the skeleton phase | Later solver consumption needs deterministic indexes + structural invariants; rejected | |

**Auto-selected:** Deterministic indexes built after dense-ID assignment; `validate()` asserting edge-endpoint/constraint-reference resolvability, no duplicate stable keys, contiguous sorted dense IDs, precision ceiling. Provider slots after `polint.type_value_alias`, before `polint.refined_calls`; cache key digests all consumed-provider output digests + source + provider/schema version.
**Notes:** D-14..D-17. Planner confirms slot against dependency DAG.

---

## Determinism Gate Inheritance (REACH-03 obligation)

| Option | Description | Selected |
|--------|-------------|----------|
| Inherit the Phase 43 `provider_manifests()`-driven determinism gate as a named acceptance criterion | Zero-maintenance auto-enrollment of the new provider; required by Phase 43 D-25 | ✓ |

**Auto-selected:** `polint.semantic_graph` auto-enrolls in the 10-shuffle byte-identical determinism gate; Phase 44 verification keeps it green.
**Notes:** D-18.

---

## Claude's Discretion

- Internal file layout of `analysis::semantic_graph/`.
- Exact payload field shapes of each `NodeKind`/`EdgeKind`/`ConstraintKind` variant (variant *sets* fixed by roadmap).
- Whether nodes or only edges/constraints carry status/precision/provenance.
- Precise index set and representation.
- Exact provider slot (after-`type_value_alias`/before-`refined_calls` is the default; planner confirms vs DAG).
- Plan slicing into (1) module + node/edge model, (2) constraint vocabulary + emission, (3) provider/cache/validation/fixtures/determinism/leak-gate.

## Deferred Ideas

- Unified solver core + `points_to::solver` folding — Phase 47 (GRAPH-03).
- `DerivedEdgeProvenance` for `polint explain` — Phase 47 (GRAPH-04).
- `refined_calls::provider` rework over solver output — Phase 47 (GRAPH-05).
- JS/TS inventory/scope/module/direct-calls as constraints — Phase 45 (JS-01/02/03).
- Go semantic frontend + sidecar lowering to constraints — Phase 46 (GO-01..04).
- Adaptation layer producing `ModelEdge` constraints — Phase 49 (ADAPT-01).
- Sidecar/toolchain/model/budget cache-key inputs + budget enforcement — Phases 50/CACHE-01/02.
- Public SDK promotion of any semantic-graph view — out of v1.3 (SDK-FUT-01).
