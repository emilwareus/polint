---
phase: 47-unified-solver-core-derived-edge-provenance
plan: 03
subsystem: analysis-kernel
tags: [rust, solver, provider, cache-key, cycle-detection, determinism, leak-gate, pub-crate, graph-03, graph-04]

# Dependency graph
requires:
  - phase: 47-01
    provides: "Unified solver core (SolverEngine + SolverBudget/BudgetStatus + SolverPolicy fold + monotonic step counter)"
  - phase: 47-02
    provides: "SolverOutput/SolverStore, SOLVER_PROVIDER_ID, DerivedEdgeFact + DerivedEdgeProvenance, derive_edges transitive closure"
  - phase: 44-semantic-graph-skeleton-constraint-vocabulary
    provides: "ConstraintKind vocabulary + reserved provider slot (after semantic_graph, before refined_calls)"
  - phase: 43-reachability-roots-per-suite-scoring-mode
    provides: "Phase 43 determinism gate (provider_manifests-driven auto-enrollment) + provider digest recipe"
  - phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
    provides: "public-surface-leak gate (ALLOWED_PRELUDE)"
provides:
  - "polint.solver private kernel provider registered in the reserved slot (after polint.semantic_graph, before polint.refined_calls, D-13)"
  - "SOLVER_SCHEMA_LABEL + solver_provider_parameter_digest digesting the SolverBudget (D-15)"
  - "solver/provider.rs: derive_solver_with_cache_stats — drives the engine over the closed constraint snapshot, output digest folds semantic_graph + type_value_alias upstream digests + SolverBudget (D-15)"
  - "solver/validate.rs: precision-ceiling/dup-key/dangling/dense-ID checks + detect_solver_summary_cycle (D-12 bounded cycle detection)"
  - "AnalysisDb.replace_solver_facts / solver_derived_edges storage (solver_derived_edges fact family)"
  - "tests/eval-fixtures/provenance/cycle-detection: native-tree proof that a solver->summary->solver cycle is bounded, not divergent (D-11/D-12)"
affects: [48 (Go RTA driver emits derived edges into this provider), 49 (TS tokens driver), 52 (GRAPH-05 refined_calls projection reads solver_derived_edges; inspect unknowns), 53 (CACHE-01/02 budget cache-key sweep)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provider digest folds upstream output digests + SolverBudget (D-15): semantic_graph + type_value_alias points-to source families + explicit budget knobs"
    - "Cycle detection as a bounded reachability check over BTree-ordered value-flow adjacency: a CallConstraint (summary) node reachable from itself is flagged and bounded, never diverged"
    - "Provider-order snapshot chore: adding a provider touched 11 ordered-vec/snapshot sites (memory floor of ~7 confirmed conservative)"

key-files:
  created:
    - crates/polint/src/analysis/solver/cache_key.rs
    - crates/polint/src/analysis/solver/validate.rs
    - crates/polint/src/analysis/solver/provider.rs
    - tests/eval-fixtures/provenance/cycle-detection/expected.polint-eval.toml
    - tests/eval-fixtures/provenance/cycle-detection/repo/main.go
    - tests/eval-fixtures/provenance/cycle-detection/repo/go.mod
    - tests/eval-fixtures/provenance/cycle-detection/repo/.polint.toml
  modified:
    - crates/polint/src/analysis/solver/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis/semantic_graph/provider.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "polint.solver consumes the stored semantic-graph constraints as its closed input snapshot (D-11); the dispatch threads SolverBudget::default() so the budget is in the cache key (D-15)."
  - "Output digest folds the SolverBudget BOTH via the parameter digest AND explicitly in solver_output_digest (belt-and-suspenders) so a budget change is unmissable (D-15)."
  - "D-12 cycle detection: a CallConstraint (summary) node that is reachable from itself through the CopyEdge value-flow graph is flagged + bounded (visited-set BFS terminates); a pure value-flow cycle with no summary node is intentionally NOT flagged (the contract is about summaries specifically)."
  - "The fixture proves bounding via a tight 30s runtime budget on mutually-recursive ping<->pong Go code — a divergent solver would blow it (D-11 bounded outer iterations)."
  - "solver_derived_edges accessor carries #[cfg_attr(not(test), allow(dead_code))] until Phase 52's refined_calls rework reads it (no production consumer yet; facts are stored unconditionally so the determinism gate observes them)."

patterns-established:
  - "Pattern 1: A new kernel provider requires updating EVERY ordered-vec/snapshot site — 11 here (3 provider.rs vecs + 1 ProviderOrderRow + neighbor assertion + run_report + observed + 2 mod.rs vecs + 2 fixture tomls + fixtures.rs vec). The memory floor of ~7 is conservative; run the full suite and fix every failure."
  - "Pattern 2: Budget participation in a cache key is a named D-15 contract; the parameter digest and the output digest both fold it, locked by a budget-change trip-wire test."

requirements-completed: [GRAPH-03, GRAPH-04]

# Metrics
duration: 36min
completed: 2026-06-02
---

# Phase 47 Plan 03: Solver Provider, Cache Key, Validation & Cycle-Detection Summary

**The unified solver is now a live, deterministic, cached, validated kernel provider: `polint.solver` registers in the reserved slot (after `polint.semantic_graph`, before `polint.refined_calls`), digests its upstream output digests + the `SolverBudget` into a cache key (D-15), validates its derived edges (precision ceiling, dup keys, dangling endpoints) and runs a bounded D-12 solver↔summary cycle-detection check, and ships a native cycle-detection fixture — all 11 provider-order snapshot sites updated, the Phase 43 determinism gate (10-shuffle byte-identical) and Phase 42 leak gate (`ALLOWED_PRELUDE` unchanged) staying green, points-to fixtures byte-identical.**

## Performance

- **Duration:** ~36 min
- **Started:** 2026-06-02T06:50:15Z
- **Completed:** 2026-06-02T07:26:33Z
- **Tasks:** 3
- **Files modified:** 16 (7 created, 9 modified)

## Accomplishments
- **Task 1 (cache key + validate, D-12/D-15):** `solver/cache_key.rs` defines `SOLVER_SCHEMA_LABEL` and `solver_provider_parameter_digest` folding the `SolverBudget` knobs, with the locked parts-list / algorithm-bump / budget-change trip-wire tests. `solver/validate.rs` emits evidence-bearing diagnostics for Exact-precision derived edges, duplicate stable keys, dangling endpoints, and non-contiguous dense IDs, plus `detect_solver_summary_cycle` — a bounded reachability check proving a `solver → summary → solver` constraint set is detected/bounded, not divergent.
- **Task 2 (provider + wiring + snapshots, D-13/D-15):** `solver/provider.rs` drives the engine over the closed semantic-graph constraint snapshot, validates, persists via the new `AnalysisDb::replace_solver_facts`, and computes an output digest folding `polint.semantic_graph` + `polint.type_value_alias` (points-to source families) digests + the `SolverBudget`. Registered the `polint.solver` `ProviderManifest` in the reserved slot + `SOLVER_SCHEMA` const + the kernel dispatch block threading the budget. Updated all 11 provider-order/snapshot sites.
- **Task 3 (cycle fixture + gates, D-12/D-14/D-16):** Added `tests/eval-fixtures/provenance/cycle-detection` (mutually-recursive Go `ping`<->`pong`) proving the solver terminates within a tight runtime budget rather than diverging. Verified the Phase 43 determinism gate stays green (`polint.solver` auto-enrolled, `go/ts_reachable` byte-identical under 10 seeded permutations) and the Phase 42 leak gate stays green with `ALLOWED_PRELUDE` untouched.

## Task Commits

Each task was committed atomically:

1. **Task 1: solver cache_key + validate (D-12, D-15, precision ceiling)** - `e00aefe2` (feat)
2. **Task 2: polint.solver provider + kernel manifest/dispatch + 11 snapshot sites (D-13, D-15)** - `f8c8f257` (feat)
3. **Task 3: cycle-detection fixture + determinism/leak gate proof (D-12, D-14, D-16)** - `8fc8c6b5` (test)

_Tasks 1 and 2 are TDD tasks; their behavior tests + implementation co-located in the new files landed in a single atomic commit each._

## Files Created/Modified
- `crates/polint/src/analysis/solver/cache_key.rs` (created) - `SOLVER_SCHEMA_LABEL` + `solver_provider_parameter_digest` (budget-folding) + 4 locked tests.
- `crates/polint/src/analysis/solver/validate.rs` (created) - `validate_derived_edges` (precision ceiling/dup/dangling/dense-ID) + `detect_solver_summary_cycle` (D-12) + 9 unit tests.
- `crates/polint/src/analysis/solver/provider.rs` (created) - `derive_solver_with_cache_stats` + `solver_output_digest` (D-15) + slot-assertion + 6 tests.
- `crates/polint/src/analysis/solver/mod.rs` (modified) - declares `cache_key`, `validate`, `provider`.
- `crates/polint/src/analysis_kernel/provider.rs` (modified) - `polint.solver` manifest + `SOLVER_SCHEMA` const + 4 provider-order/snapshot sites.
- `crates/polint/src/analysis_kernel/mod.rs` (modified) - solver dispatch block (threads SolverBudget) + 2 ordered-vec snapshots.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` (modified) - provider-output-order snapshot.
- `crates/polint/src/analysis/semantic_graph/provider.rs` (modified) - neighbor slot assertion (tva+2 == solver, tva+3 == refined_calls).
- `crates/polint/src/core/mod.rs` (modified) - `solver_derived_edges` field + `replace_solver_facts` + accessor.
- `crates/polint/src/eval/observed.rs` + `eval/fixtures.rs` (modified) - provider-order invariant snapshots.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` (modified) - inserted `polint.solver` invariant + renumbered.
- `tests/eval-fixtures/provenance/cycle-detection/*` (created) - the D-12 native cycle-detection fixture.

## Decisions Made
- **Closed input snapshot:** the provider reads `db.semantic_constraints()` (already stored by `polint.semantic_graph`) as its fixed input — never re-reading mutated state mid-run (D-11). The dispatch threads `SolverBudget::default()`.
- **Budget in the cache key twice (D-15):** the parameter digest folds the budget AND `solver_output_digest` folds it explicitly, so a budget change is unmissable from either angle; a dedicated trip-wire test locks both.
- **Cycle detection is summary-specific (D-12):** a `CallConstraint` summary node reachable from itself through the value-flow graph is flagged + bounded; a pure value-flow cycle with no summary node is intentionally not flagged (the closed-input contract is about summaries, not arbitrary copy cycles).
- **Fixture proves bounding via runtime budget:** a tight 30s budget on `ping`<->`pong` mutual recursion — a divergent solver blows it, so staying inside is the bounded-iteration proof.

## Deviations from Plan

None — plan executed exactly as written. The plan estimated "~7" provider-order snapshot sites (treated as a floor); the full `cargo test -p polint` surfaced **11** sites in total (3 ordered vecs in `provider.rs`, 1 `ProviderOrderRow`, the `semantic_graph` neighbor assertion, `run_report`, `observed`, 2 ordered vecs in `analysis_kernel/mod.rs`, the `fixtures.rs` assertion vec, and 2 fixture tomls). All were updated and pass — exactly the "run the full suite, fix every failing ordered vec" procedure the plan and the `polint-kernel-provider-snapshot-sites` memory prescribe. (One behavior-neutral adjustment: the `solver_derived_edges` accessor carries `#[cfg_attr(not(test), allow(dead_code))]` until Phase 52 lands a production reader, mirroring the Plan-02 explain-seam pattern; the facts are still stored unconditionally so the determinism gate observes them.)

## Issues Encountered
- The initial full-suite run had 4 failures — all additional provider-order snapshot sites beyond the four `provider.rs` sites the plan listed: `run_report::provider_outputs_are_constructed_in_manifest_order`, `eval::observed` provider-order invariants, and two `eval::fixtures` tests (one reading the `provider-order` fixture toml, one suite-coverage). Each was a snapshot update (insert `polint.solver` + renumber), not a logic bug. The `eval_native_fixture_runner_manifest_asserts_all_provider_order_invariants` test reads the fixture toml, so updating the toml required updating its sibling hardcoded vec in lockstep.
- The plain (non-test) `cargo build` flagged the `solver_derived_edges` accessor as dead code (it is only read from `#[cfg(test)]` provider tests today); resolved with the `#[cfg_attr(not(test), allow(dead_code))]` pattern + a Phase-52 rationale comment.

## Known Stubs
- `AnalysisDb::solver_derived_edges` has no production reader yet — Phase 52's GRAPH-05 refined_calls rework is the intended consumer. This is the intentional Phase-47/Phase-52 boundary (Phase 47 *emits* into the slot; Phase 52 *reads* it), not a goal-blocking gap. The facts are stored unconditionally on every run so the determinism gate observes them and the cache key certifies them.
- `eval::report::SolverMetricSection` (`solver_step_count` / `budget_exceeded_reasons`) remains the Phase-43-reserved zero/empty shape — wiring the live solver step counts into the observed report is downstream (Phase 52 unknown taxonomy). The solver provider is live; the metric projection is intentionally deferred.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- **Phase 48 (Go RTA)** and **Phase 49 (TS tokens)** can now attach their real `SolverPolicy` drivers; the `polint.solver` provider, cache key, validation, and determinism enrollment are live and the slot is wired.
- **Phase 52 (GRAPH-05)** can rework `refined_calls::provider` to project over `db.solver_derived_edges()`; the storage, provenance, and explain seam are all in place, and the provider slot is forward-compatible.
- Determinism (BTree accumulation + dense-IDs-after-sort), the budget-in-cache-key contract, and the bounded cycle-detection contract are established and locked by trip-wire tests.

## Self-Check: PASSED

- Files: all 7 created files (`solver/cache_key.rs`, `solver/validate.rs`, `solver/provider.rs`, and the 4 cycle-detection fixture files) and the 9 modified files exist on disk.
- Commits: `e00aefe2`, `f8c8f257`, `8fc8c6b5` present in `git log`.
- Tests: solver `cache_key` (4), `validate` (9, incl. D-12 cycle detection), `provider` (6, incl. the slot assertion + 3 digest-invalidation tests); the 11 provider-order/snapshot sites; the determinism gate (`go/ts_reachable` byte-identical under 10 permutations); the leak gate (5, `ALLOWED_PRELUDE` unchanged); `points_to` (26, byte-identical); full `cargo test -p polint` green; `cargo clippy -p polint --all-targets` clean; `make lint` pre-commit hook green on all three commits.

---
*Phase: 47-unified-solver-core-derived-edge-provenance*
*Completed: 2026-06-02*
