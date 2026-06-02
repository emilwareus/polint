---
phase: 47-unified-solver-core-derived-edge-provenance
plan: 02
subsystem: analysis
tags: [rust, solver, provenance, derived-edges, determinism, stable-key, pub-crate, graph-04]

# Dependency graph
requires:
  - phase: 47-01
    provides: "Unified solver core (engine + monotonic u64 step counter, SolverBudget/BudgetStatus, SolverPolicy scaffold + points-to fold)"
  - phase: 44-semantic-graph-skeleton-constraint-vocabulary
    provides: "ConstraintKind closed vocabulary + ConstraintFact shape + as_str() label reused by provenance"
  - phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
    provides: "Stable-key total-order dedup rule + public-surface-leak gate"
provides:
  - "DerivedEdgeProvenance (pub(crate)): contributing fact IDs total-ordered by stable ID, producing ConstraintKind label, monotonic u64 solver step"
  - "Derived-edge fact family: FactFamily::SolverDerivedEdge + DerivedEdgeFact (serde-skip dense id, reuses PointsToStatus/PointsToPrecision, rejects exact precision)"
  - "Deterministic SolverOutput/SolverStore (sort-then-dense-IDs, shuffle-stable, dup-key + precision-ceiling validation, SOLVER_PROVIDER_ID const)"
  - "engine::derive_edges: transitive CopyEdge closure emitting DerivedEdgeFacts with load-bearing provenance"
  - "polint explain private plumbing surfacing derived-edge provenance (no new public CLI/JSON surface, D-10)"
  - "GRAPH-04 deletion property test proving provenance is load-bearing (D-09)"
affects: [47-03 (provider/cache-key/determinism-gate wiring reads SolverOutput + DerivedEdgeProvenance), 48 (Go RTA driver emits derived edges with provenance), 49 (TS tokens driver), 52 (GRAPH-05 refined_calls projection + inspect unknowns public surface)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provenance references EXISTING stable identities by stable_key (composition over duplication) — family folded into the length-prefixed key, no parallel identity space minted"
    - "Owned constraint-kind label (String from ConstraintKind::as_str) so provenance + derived-edge facts stay Serialize/Deserialize"
    - "Load-bearing provenance: transitive-closure derivation records the contributing-constraint set so deletion invalidation is a real property, not decorative metadata"
    - "Test-exercised private explain seam (D-10): pub(crate) internal accessor, cfg(test)-facing until the Plan 03 provider / Phase 52 surface consumes it; no public JSON field"

key-files:
  created:
    - crates/polint/src/analysis/solver/provenance.rs
    - crates/polint/src/analysis/solver/facts.rs
    - crates/polint/src/analysis/solver/store.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis/solver/engine.rs
    - crates/polint/src/analysis/solver/mod.rs
    - crates/polint/src/cli/mod.rs

key-decisions:
  - "DerivedEdgeProvenance carries the three roadmap-named fields (D-08): contributing facts total-ordered by stable ID (sorted + de-duplicated in ::new), ConstraintKind::as_str() label, and the engine's monotonic u64 step."
  - "ContributingFact stores only the stable_key (which embeds the FactFamily label via stable_key_from_parts) — no separate non-serializable FactFamily field, keeping the fact Deserialize."
  - "constraint_kind stored as owned String (not &'static str) so DerivedEdgeProvenance + DerivedEdgeFact derive Deserialize for the serde-skip round-trip."
  - "derive_edges computes the transitive CopyEdge closure with per-path contributing-constraint accumulation, making the deletion property (D-09) genuinely load-bearing."
  - "D-10 wired as a pub(crate) cfg(test)-facing explain seam (explain_derived_edge_provenance) — no new public ExplainReport/ExplainRuleRow field; ALLOWED_PRELUDE untouched."

patterns-established:
  - "Pattern 1: Provenance total-order by stable key + de-dup in the constructor — byte-stable and shuffle-insensitive by construction."
  - "Pattern 2: Precision ceiling as an enforced mapping (derived_edge_precision_ceiling) that no arm maps to FactPrecision::Exact, locked by an exhaustive unit test (D-06)."
  - "Pattern 3: Solver store mirrors semantic_graph store — SolverOutput::normalized (sort-then-dense-IDs), SolverStore::from_output with referential validation + by-constraint-kind index."

requirements-completed: [GRAPH-04]

# Metrics
duration: 21min
completed: 2026-06-02
---

# Phase 47 Plan 02: Derived-Edge Provenance Summary

**Every solver-derived edge now carries a `pub(crate) DerivedEdgeProvenance` (contributing fact IDs total-ordered by stable ID, the producing `ConstraintKind` label, and the engine's monotonic u64 solver step), emitted through a dedicated `SolverDerivedEdge` fact family into a shuffle-stable `SolverStore`, surfaced via `polint explain`'s existing private plumbing with no new public surface, and proven load-bearing by a deletion property test.**

## Performance

- **Duration:** ~21 min
- **Started:** 2026-06-02T06:24:13Z
- **Completed:** 2026-06-02T06:45:34Z
- **Tasks:** 3 (all TDD)
- **Files modified:** 8 (3 created, 5 modified)

## Accomplishments
- Added `DerivedEdgeProvenance` + `ContributingFact` (D-08): contributing facts referenced by EXISTING stable key (family folded in via `stable_key_from_parts`), sorted + de-duplicated in the constructor so provenance is byte-stable and shuffle-insensitive; constraint kind reuses `ConstraintKind::as_str()`; solver step is the Wave-1 engine's monotonic `u64`.
- Added the `FactFamily::SolverDerivedEdge` family + `DerivedEdgeFact` (serde-skip dense `id`, reuses the shared `PointsToStatus`/`PointsToPrecision` vocabulary) with an enforced precision ceiling that rejects `FactPrecision::Exact` (D-06), and a deterministic `SolverOutput`/`SolverStore` (sort-then-dense-IDs, shuffle-stable, duplicate-key + precision-ceiling validation, `SOLVER_PROVIDER_ID = "polint.solver"`).
- Wired `engine::derive_edges` to emit derived-edge facts with load-bearing provenance (transitive `CopyEdge` closure over a deterministic `BTreeMap`/`BTreeSet` worklist, accumulating the contributing-constraint set per derived edge).
- Wired `polint explain`'s existing private plumbing (`explain_derived_edge_provenance`) to surface a derived edge's contributing facts + constraint kind + solver step with NO new public JSON field (D-10); the public-surface-leak gate stays green and `ALLOWED_PRELUDE` is unchanged.
- Added the GRAPH-04 deletion property test (D-09): deleting ANY single contributing fact does not reproduce the transitive derived edge — provenance is sound and load-bearing, not decorative.

## Task Commits

Each task was committed atomically:

1. **Task 1: ID newtypes + DerivedEdgeProvenance (D-08)** - `b2588a41` (feat)
2. **Task 2: Derived-edge fact family + deterministic solver store** - `070e4c74` (feat)
3. **Task 3: explain provenance seam (D-10) + deletion property test (D-09)** - `dd7c9a38` (feat)

_All three are TDD tasks; behavior tests + implementation co-located in the new files landed in a single atomic commit each._

## Files Created/Modified
- `crates/polint/src/analysis/solver/provenance.rs` (created) - `DerivedEdgeProvenance` + `ContributingFact`; total-order/de-dup constructor; `stable_key_fragment`; shuffle-stability, dedup, label/step unit tests + the D-09 deletion property test.
- `crates/polint/src/analysis/solver/facts.rs` (created) - `DerivedEdgeFact` (serde-skip dense id, provenance-carrying) + `derived_edge_precision_ceiling` (never Exact, D-06) + precision/serialization tests.
- `crates/polint/src/analysis/solver/store.rs` (created) - `SolverOutput::normalized` (sort-then-dense-IDs), `SolverStore::from_output` (constraint-kind index + dup-key/precision-ceiling validation), `SOLVER_PROVIDER_ID`; shuffle-stability tests.
- `crates/polint/src/analysis/ids.rs` (modified) - `DerivedEdgeId` dense-ID newtype (Default-bearing for the serde-skip id) + registered in `assert_small_id_contract`.
- `crates/polint/src/analysis_kernel/metadata.rs` (modified) - `FactFamily::SolverDerivedEdge` variant + `label()` arm.
- `crates/polint/src/analysis/solver/engine.rs` (modified) - `derive_edges` transitive `CopyEdge` closure emitting `DerivedEdgeFact`s with provenance into `SolverOutput`; two engine tests.
- `crates/polint/src/analysis/solver/mod.rs` (modified) - declares `facts`, `provenance`, `store` modules.
- `crates/polint/src/cli/mod.rs` (modified) - `explain_derived_edge_provenance` private plumbing + `DerivedEdgeProvenanceView` (pub(crate), no public JSON field, D-10) + a unit test exercising the seam.

## Decisions Made
- **Stable-key-only contributing reference:** `ContributingFact` carries just the `stable_key` (which embeds the originating `FactFamily` via the length-prefixed `stable_key_from_parts` recipe), avoiding a non-serializable `FactFamily` field while keeping the byte-stable total order intact.
- **Owned constraint-kind label:** `constraint_kind: String` (from `ConstraintKind::as_str()`) rather than `&'static str`, so the provenance and the `DerivedEdgeFact` that carries it derive `Deserialize` for the serde-skip round-trip.
- **Load-bearing derivation:** the engine derives the transitive `CopyEdge` closure and records, per derived edge, the contributing-constraint set on its derivation path — which is what makes the D-09 deletion property a real invariant rather than a decorative annotation.
- **Test-exercised private explain seam (D-10):** added a `pub(crate)` `explain_derived_edge_provenance` accessor + an internal `DerivedEdgeProvenanceView`, both `cfg(test)`-live until the Plan 03 provider / Phase 52 surface consumes them in production; no public `ExplainReport`/`ExplainRuleRow` field, `ALLOWED_PRELUDE` untouched.

## Deviations from Plan

None - plan executed exactly as written. (Two within-task adjustments, both behavior-neutral: (1) `DerivedEdgeProvenance.constraint_kind` is owned `String` instead of `&'static str` because `&'static str` is not `Deserialize`, which the serde-skip derived-edge fact requires; (2) the `FactFamily::SolverDerivedEdge` variant was deferred from Task 1 to Task 2 — its only producer is the Task 2 `facts.rs`/`engine.rs` path, so adding it in Task 1 would have tripped the `-D warnings` dead-code gate.)

## Issues Encountered
- The pre-`make-lint` clippy gate (`-D warnings`) flagged `for (&start, _) in &map`, a `contains_key`-then-`insert`, and an unused `SolverOutput::empty()`; resolved by iterating `adjacency.keys()`, using the `BTreeMap` `Entry::Vacant` API, and removing the unused constructor.
- The D-10 explain seam (`explain_derived_edge_provenance` + `DerivedEdgeProvenanceView`) is only reachable from a `cfg(test)` caller, so `expect(dead_code)` reported unfulfilled under `--all-targets` (the items are live in test builds); switched to `#[cfg_attr(not(test), allow(dead_code))]` with a Phase-47-D-10 rationale comment.

## Known Stubs
- `FactFamily::SolverDerivedEdge` and the `SolverStore`/`SolverOutput` plumbing are produced/consumed by the Task 2 `derive_edges` path and unit tests, but are NOT yet wired into the kernel provider manifest — that is the intentional **Plan 03** boundary (provider registration + cache key + determinism-gate enrollment). `explain_derived_edge_provenance` is a `cfg(test)`-facing seam until the Plan 03 provider lands a production caller (D-10 explicitly sanctions this). These are reserved-for-Plan-03 stubs, not goal-blocking gaps: GRAPH-04 for this plan ships the provenance struct, fact family, store, explain seam, and the deletion property test.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- **Plan 03** can now register the `polint.solver` provider over `SolverOutput`/`DerivedEdgeProvenance` (the store's `normalized()`/`from_output` + `SOLVER_PROVIDER_ID` are in place), digest the budgets into the cache key (D-15), enroll in the Phase 43 determinism gate (D-14), and add the cycle-detection fixture (D-12) — plus the ~7 provider-order snapshot-site chore.
- The `polint explain` private plumbing exists and is test-locked; Plan 03's provider gives it a production caller, and Phase 52 (`inspect unknowns`) is the eventual public consumer.
- Determinism discipline (stable-key sort → dense IDs, BTree-ordered closure worklist, serde-skip dense id) matches the points-to/semantic-graph template; points-to fixtures stayed byte-identical (full lib suite, 1885 tests, green).

## Self-Check: PASSED

- Files: `solver/provenance.rs`, `solver/facts.rs`, `solver/store.rs` created; `ids.rs`, `metadata.rs`, `solver/engine.rs`, `solver/mod.rs`, `cli/mod.rs` modified — all present on disk.
- Commits: `b2588a41`, `070e4c74`, `dd7c9a38` present in `git log`.
- Tests: `solver::` (35) incl. the D-09 deletion property test, the explain-seam cli test, the precision-ceiling and shuffle-stability tests; `public_surface_leak` (5, `ALLOWED_PRELUDE` untouched); full `polint --lib` (1885) and `points_to` byte-identical — all green. `cargo build -p polint`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check` clean.

---
*Phase: 47-unified-solver-core-derived-edge-provenance*
*Completed: 2026-06-02*
