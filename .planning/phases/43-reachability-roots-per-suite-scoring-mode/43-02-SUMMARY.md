---
phase: 43-reachability-roots-per-suite-scoring-mode
plan: 02
subsystem: api
tags: [reachability, marking, scoring-mode, call-graph, eval, polint]

# Dependency graph
requires:
  - phase: 43-reachability-roots-per-suite-scoring-mode
    provides: analysis::reachability module + ReachabilityRootFact + CallReachabilityFact shape + polint.reachability provider (Plan 01)
  - phase: 30-direct-call-facts
    provides: direct-call edge set (CallTargetFact resolved-target edges) the BFS walks
provides:
  - "ScoringMode closed enum (oracle-rta/oracle-jelly/whole-repo) + required SuiteManifest.scoring_mode field with two-layer gate-fails-if-missing"
  - "All four committed suite manifests declare scoring_mode (go-x-tools=oracle-rta, jelly=oracle-jelly, gosec/secbench=whole-repo)"
  - "traverse.rs reachable-set BFS over resolved direct-call edges + CallReachabilityFact marking populated through the provider"
  - "mode-aware scoring filter (filter_scored_edges_by_scoring_mode) joining the reachable-graph marking by call-site stable key"
affects: [44-marking-traversal, 45-per-suite-scoring-mode, 46-determinism-gate, reachability, scoring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Closed-enum kebab wire strings via PER-VARIANT serde rename (not rename_all) — D-14 correction"
    - "Sorted-frontier BFS (BTreeMap/BTreeSet adjacency) for insertion-order-independent marking"
    - "Composition-by-call-site-stable-key marking family; analysis::calls never mutated"

key-files:
  created:
    - crates/polint/src/analysis/reachability/traverse.rs
  modified:
    - crates/polint/src/eval/suite.rs
    - crates/polint/src/eval/runner.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/tiers.rs
    - crates/polint/src/eval/markdown.rs
    - crates/polint/src/eval/adapter.rs
    - crates/polint/src/eval/external/secbench_js.rs
    - crates/polint/src/eval/external/go_x_tools_callgraph.rs
    - crates/polint/src/eval/external/jelly_callgraph.rs
    - crates/polint/src/eval/external/gosec.rs
    - crates/polint/src/analysis/reachability/facts.rs
    - crates/polint/src/analysis/reachability/provider.rs
    - crates/polint/src/analysis/reachability/store.rs
    - crates/polint/src/analysis/reachability/mod.rs
    - research/evaluation-harness/suites/go-x-tools-rta-callgraph.toml
    - research/evaluation-harness/suites/jelly-callgraph-micro.toml
    - research/evaluation-harness/suites/gosec-samples.toml
    - research/evaluation-harness/suites/secbench-js-smoke.toml

key-decisions:
  - "ScoringMode uses per-variant #[serde(rename = ...)] for kebab wire strings (oracle-rta/oracle-jelly/whole-repo), NOT rename_all=snake_case which would emit oracle_rta (43-PATTERNS D-14)"
  - "Reachable-set BFS extends the frontier ONLY on Resolved (not Ambiguous) direct-call targets — the pre-solver edge set marks only edges it is confident about"
  - "An edge with no marking fails closed under oracle-rta (excluded from scoring) — never silently included (threat T-43-02-02)"
  - "The mode-aware scoring helpers + the eval external-suite scoring path are #[cfg(test)] because the eval harness is internal/test-facing with no public CLI/SDK surface"

patterns-established:
  - "Pattern: marking traversal supplies roots explicitly so it never depends on db.reachability_roots() being populated yet; provider seeds it with real-function roots before storing"
  - "Pattern: scoring filter is generic over edge type via a call_site_key_of closure so it is unit-testable without the full eval pipeline"

requirements-completed: [REACH-02]

# Metrics
duration: 75min
completed: 2026-05-29
---

# Phase 43 Plan 02: Per-Suite Scoring Mode & Reachable-Graph Marking Summary

**Required `ScoringMode` field on every suite manifest (kebab wire strings, two-layer gate-fails-if-missing), a reachable-set BFS over the v1.2 direct-call edge set that emits a separate `CallReachabilityFact` marking family without mutating `analysis::calls`, and a mode-aware scoring filter that filters scored edges to the reachable set only under `oracle-rta`.**

## Performance

- **Duration:** ~75 min
- **Tasks:** 3
- **Files modified/created:** 1 created (traverse.rs) + 17 modified

## Accomplishments

- **Task 1 — ScoringMode + required field + gate:** Added the closed `ScoringMode` enum with per-variant kebab serde renames (`oracle-rta`/`oracle-jelly`/`whole-repo`), a required non-`Option` `scoring_mode` field on `SuiteManifest`, and a two-layer gate (structural `deny_unknown_fields` + non-`Option`, plus an explicit `validate()` guard). Byte-for-byte wire-string tests, a structural missing-field negative test, an invalid-value negative test, and a per-TOML round-trip test all pass. All four committed suite TOMLs declare the correct mode.
- **Task 2 — reachable-set marking:** Added `traverse.rs` with `compute_reachable_set` (BFS from root `target_function`s over RESOLVED direct-call edges only) and `mark_call_reachability` (one `CallReachabilityFact` per call site, keyed by the call-site stable key). Sorted-frontier `BTreeMap`/`BTreeSet` adjacency + a mark sort make the output byte-identical regardless of insertion order. The provider wires `marks` into the output, the output digest, and the stored facts; the store gained a `by_call_site_stable_key` read index. `analysis::calls` is never mutated (proven by `marking_does_not_mutate_the_call_store`).
- **Task 3 — mode-aware scoring filter:** Added `filter_scored_edges_by_scoring_mode` (oracle-rta filters to the reachable-from-roots edges; oracle-jelly/whole-repo score the full set) joining the marking by call-site stable key via `reachable_graph_lookup`. Threaded `manifest.scoring_mode` into the eval external-suite scoring path (`scored_call_graph_edges_for_db` + a recorded scored-edge-count invariant). The backwards-mode footgun is guarded by a strict-subset regression test (oracle-rta ⊆ oracle-jelly), and unmarked edges fail closed under oracle-rta.

## Task Commits

1. **Task 1: required ScoringMode field + kebab wire strings + four manifest updates** — `95384ac` (feat)
2. **Task 2: reachable-set BFS marking + CallReachabilityFact family** — `4c880a5` (feat)
3. **Task 3: mode-aware scoring filter in the eval scoring path** — `a91782c` (feat)

_TDD note: each task landed as a single `feat` commit with its co-located tests rather than separate RED/GREEN commits — the typed contracts (the required field, the marking shape, the filter signature) must exist for the byte-stability/subset tests to compile, matching the Plan 01 convention._

## Decisions Made

- **Per-variant serde rename, not `rename_all`.** Followed 43-PATTERNS.md D-14: `rename_all = "snake_case"` would emit `oracle_rta`; the kebab wire contract requires explicit `#[serde(rename = "oracle-rta")]` per variant. Asserted byte-for-byte.
- **Resolved-only frontier.** The BFS extends only on `CallTargetStatus::Resolved` targets; `Ambiguous` is conservatively excluded. The pre-solver edge set marks only edges it is confident about; Phases 47/48 widen this behind the same marking contract (documented at the top of `traverse.rs`).
- **Fail-closed unmarked edges.** Under `oracle-rta`, an edge with no marking cannot be proven reachable, so it is excluded from scoring (never silently included) — mitigates threat T-43-02-02.
- **`#[cfg(test)]` scoring helpers.** `reachable_graph_lookup`, `filter_scored_edges_by_scoring_mode`, and `scored_call_graph_edges_for_db` are test-gated because the entire eval external-suite scoring path is `#[cfg(test)]` (the eval harness is internal/test-facing per the v1.2 decisions — no public CLI/SDK surface). The marking itself is fully wired into the production provider/store/digest.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated all in-repo SuiteManifest construction/parse sites for the new required field**
- **Found during:** Task 1 (and one more surfaced during Task 2's full-suite run)
- **Issue:** Adding the required non-`Option` `scoring_mode` field broke every `SuiteManifest { .. }` literal and every inline manifest-TOML fixture that omitted it — exactly the structural gate firing.
- **Fix:** Added `scoring_mode` to the 8 `SuiteManifest` test/helper literals (`suite.rs`, `runner.rs` ×2, `tiers.rs`, `markdown.rs`, `external/{secbench_js,go_x_tools_callgraph,jelly_callgraph,gosec}.rs`) and to the inline manifest-TOML fixture in `eval/adapter.rs`. The four committed suite TOMLs were updated as the planned Task 1 deliverable.
- **Files modified:** the files listed above.
- **Verification:** `cargo test -p polint eval::suite eval::adapter eval::runner eval::metrics` green; full `--lib` suite 1693/1693 green.
- **Committed in:** `95384ac` (helpers + TOMLs), `4c880a5` (adapter.rs fixture surfaced during Task 2).

**2. [Rule 3 - Blocking] Refactored mark_call_reachability to take roots explicitly**
- **Found during:** Task 2 (provider wiring)
- **Issue:** The provider must compute marks before the roots are stored in the db, so reading `db.reachability_roots()` inside the traversal would see an empty/stale set.
- **Fix:** `compute_reachable_set` / `mark_call_reachability` accept `roots: &[ReachabilityRootFact]`; the provider passes the discovered real-function roots directly.
- **Files modified:** `traverse.rs`, `provider.rs`.
- **Verification:** `cargo test -p polint reachability::traverse reachability::provider` green.
- **Committed in:** `4c880a5`.

**Total deviations:** 2 auto-fixed (both Rule 3 blocking). No scope creep — the manifest-helper updates and the explicit-roots refactor were both required to make the planned contracts compile and wire cleanly.

## Issues Encountered

- `push_function`/`replace_reachability_facts` reassign dense IDs on insert, so a metrics test initially asserted a pre-store `FunctionId(1)`; fixed by reading the id returned from `push_function` (test-expectation bug, not implementation).
- `toml::from_str` cannot deserialize a bare top-level scalar; the kebab-string deserialize test uses `serde_json` for the scalar form, while the full TOML manifest path is covered by the per-TOML round-trip test.
- The pre-commit `make lint` gate runs `cargo fmt --check` and `cargo clippy -D warnings`; resolved formatting and gated the test-only scoring helpers/imports with `#[cfg(test)]` to avoid non-test dead-code warnings.

## User Setup Required

None — no external service configuration. `scoring_mode` is a required suite-manifest field already populated on all four committed suites.

## Next Phase Readiness

- The `CallReachabilityFact` marking is now populated through the production `polint.reachability` provider (store/digest/debug all carry marks), so Plan 03's determinism gate can assert byte-identical marks under permuted insertion order — the `permuted_insertion_order_produces_byte_identical_marks` test is the precursor.
- The mode-aware scoring filter contract (`filter_scored_edges_by_scoring_mode` + `reachable_graph_lookup`) is in place for the per-suite scoring work; later phases that add solver-derived edges plug into the SAME `CallReachabilityFact` marking contract (documented in `traverse.rs`).

## Threat Flags

None — no new network endpoints, auth paths, or trust-boundary surface beyond the planned `scoring_mode` parse path (mitigated by T-43-02-01) and the marking join (mitigated by T-43-02-02/03/04).

---
*Phase: 43-reachability-roots-per-suite-scoring-mode*
*Completed: 2026-05-29*

## Self-Check: PASSED

`traverse.rs` and `43-02-SUMMARY.md` exist on disk; all three task commits (95384ac, 4c880a5, a91782c) are present in git history.
