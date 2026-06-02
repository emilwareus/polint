---
phase: 47-unified-solver-core-derived-edge-provenance
verified: 2026-06-02T10:05:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: interrupted
  note: "Prior verifier run aborted by a transient API error before writing VERIFICATION.md. This is a fresh full verification; working tree clean, all 9 plan commits intact."
---

# Phase 47: Unified Solver Core & Derived-Edge Provenance Verification Report

**Phase Goal:** polint has a single private deterministic solver consuming the constraint vocabulary, with explicit budgets, per-language policy scaffolding, and full provenance on every derived edge — the heart of v1.3.
**Verified:** 2026-06-02T10:05:00Z
**Status:** passed
**Re-verification:** No — initial verification (prior run was interrupted before writing a report).

## Goal Achievement

### Observable Truths (the 5 ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
| - | ----- | ------ | -------- |
| 1 | Private `analysis::solver` with deterministic `VecDeque` worklist, explicit `SolverBudget`/`BudgetStatus`, per-language `SolverPolicy` trait scaffolding; points-to folded in as a sub-domain | ✓ VERIFIED | `solver/mod.rs` declares 9 `pub(crate)` sub-modules + D-04/D-11 docs. `engine.rs` `SolverEngine::run` drains a `VecDeque<usize>` worklist with monotonic `u64` step counter (lines 69-114). `budget.rs` `SolverBudget` (max_steps + max_outer_iterations + PointsToSubBudget) and `BudgetStatus` closed enum. `policy.rs` `SolverPolicy` trait + `PointsToPolicy` folding `solve_points_to` by composition (line 90). 54 `solver::` lib tests pass incl. `points_to_via_engine_equals_solve_points_to`. |
| 2 | Solver inherits the Phase 43 determinism gate — 10-shuffle byte-identical observed JSON WITH `polint.solver` enrolled | ✓ VERIFIED | `cargo test --lib eval::determinism_gate` → 6 passed: `go_reachable`/`ts_reachable` byte-identical under 10 seeded permutations; `shuffled_provider_count_equals_manifest_count` confirms `polint.solver` auto-enrolls via `provider_manifests()` (no harness edit). Dispatch block `analysis_kernel/mod.rs:588` runs the provider in production. |
| 3 | Every solver-derived edge carries `DerivedEdgeProvenance` (contributing fact IDs total-ordered by stable ID, constraint kind, solver step) consumable by `polint explain`; the deletion property test exists and passes | ✓ VERIFIED | `provenance.rs` `DerivedEdgeProvenance` has all 3 fields; `::new` sorts+dedups contributing facts by stable_key. `cli/mod.rs:1415` `explain_derived_edge_provenance` surfaces all 3 via `pub(crate)` view (test at 2812). D-09 test `deleting_any_contributing_fact_invalidates_the_derived_edge` PASSES (real load-bearing transitive-closure check, provenance.rs:228). |
| 4 | Dependency contract documented (closed input set, single-fixpoint-per-run, bounded outer iterations) AND a cycle-detection fixture proves no solver↔summary loop | ✓ VERIFIED | `solver/mod.rs:34-50` documents the D-11 contract. `validate.rs` `detect_solver_summary_cycle` (line 119) + 4 unit tests pass. Fixture `tests/eval-fixtures/provenance/cycle-detection/` (ping↔pong, 30s runtime budget) auto-discovered and PASSES via `eval_native_fixture_suite_covers_required_categories` (193s full-suite run, all fixtures green). |
| 5 | All solver types stay `pub(crate)` and the public-surface-leak gate passes; `ALLOWED_PRELUDE` unchanged | ✓ VERIFIED | `grep` for `^pub (struct\|enum\|fn\|trait\|const\|mod\|type)` in `solver/` → NONE (all `pub(crate)`). No solver type in `ALLOWED_PRELUDE`. `cargo test --test public_surface_leak` → 5 passed. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `solver/mod.rs` | Module root + naming-collision guard + D-11 contract | ✓ VERIFIED | Substantive doc (51 lines) + declares budget/cache_key/engine/facts/policy/provenance/provider/store/validate, all `pub(crate)`. |
| `solver/budget.rs` | SolverBudget + BudgetStatus closed enum | ✓ VERIFIED | SolverBudget (max_steps/max_outer_iterations/PointsToSubBudget), projection `points_to_budget()`, pinned-order + variant-count tests. |
| `solver/policy.rs` | SolverPolicy trait + 1 real impl + 2 honest stubs | ✓ VERIFIED | `PointsToPolicy` (composition fold), `GoRtaPolicy`/`TsTokensPolicy` return `PolicyOutcome::empty()`; stub-derives-nothing test passes. |
| `solver/engine.rs` | Deterministic VecDeque worklist + budget + step counter | ✓ VERIFIED | `SolverEngine::run` + `derive_edges` transitive closure with provenance; equivalence/determinism/exhaustion tests. |
| `solver/provenance.rs` | DerivedEdgeProvenance with 3 roadmap fields + D-09 test | ✓ VERIFIED | 3 fields; total-order/dedup constructor; deletion property test load-bearing and passing. |
| `solver/facts.rs` | Derived-edge fact family, serde-skip id, rejects Exact | ✓ VERIFIED | `DerivedEdgeFact` with `#[serde(skip)] id`, `derived_edge_precision_ceiling` never Exact. |
| `solver/store.rs` | Deterministic SolverOutput + SOLVER_PROVIDER_ID + dense-IDs-after-sort | ✓ VERIFIED | `normalized()` sort-then-dense-IDs; `SOLVER_PROVIDER_ID = "polint.solver"`; shuffle-stability tests. |
| `solver/cache_key.rs` | SOLVER_SCHEMA_LABEL + budget-digesting param digest | ✓ VERIFIED | `solver_provider_parameter_digest` folds budget knobs; budget-change test. |
| `solver/validate.rs` | Precision/dup/dangling/dense-ID + cycle detection | ✓ VERIFIED | `validate_derived_edges` + `detect_solver_summary_cycle`; 9 unit tests pass. |
| `solver/provider.rs` | polint.solver provider + output digest folding upstream + budget | ✓ VERIFIED | `derive_solver_with_cache_stats`, `solver_output_digest`, slot assertion test passes. |
| `tests/eval-fixtures/provenance/cycle-detection` | Fixture proving no solver↔summary loop | ✓ VERIFIED | `expected.polint-eval.toml` + `repo/{main.go,go.mod}`; passes via native fixture suite. |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `analysis/mod.rs` | `solver/mod.rs` | `pub(crate) mod solver;` | ✓ WIRED | Registered between slicing and stable_key. |
| `solver/policy.rs` | `points_to/solver.rs` | `solve_points_to` invoked in place (fold) | ✓ WIRED | `PointsToPolicy::solve` line 90. |
| `analysis_kernel/provider.rs` | `solver/provider.rs` | `polint.solver` manifest after semantic_graph, before refined_calls | ✓ WIRED | id at line 701, SOLVER_SCHEMA at 225; slot test passes. |
| `analysis_kernel/mod.rs` | `solver/provider.rs` | dispatch calls `derive_solver_with_cache_stats` with upstream digests + budget | ✓ WIRED | Dispatch block lines 588-604 (production, not test-only). |
| `cli/mod.rs` | `solver/provenance.rs` | explain private plumbing surfaces provenance | ✓ WIRED | `explain_derived_edge_provenance` line 1415, no public JSON field. |
| `solver/provenance.rs` | `semantic_graph/constraints.rs` | reuse `ConstraintKind::as_str()` | ✓ WIRED | provenance.rs:105. |

### Behavioral Spot-Checks / Gate Execution (run by verifier)

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Leak gate | `cargo test -p polint --test public_surface_leak` | 5 passed; ALLOWED_PRELUDE unchanged | ✓ PASS |
| Solver core + provenance + cycle + provider | `cargo test -p polint --lib solver::` | 54 passed (incl. D-09 deletion test, cycle detection, slot + digest tests) | ✓ PASS |
| Points-to byte-identical (fold preserved) | `cargo test -p polint --lib analysis::points_to` | 10 passed (locked determinism + budget tests green) | ✓ PASS |
| Determinism gate (10-shuffle, solver enrolled) | `cargo test -p polint --lib eval::determinism_gate` | 6 passed; go/ts_reachable byte-identical; manifest-count enrollment asserted | ✓ PASS |
| Provider-order snapshots updated | `cargo test -p polint --lib provider_order` + slot assertion | 7 + 1 passed | ✓ PASS |
| Cycle-detection fixture runs & terminates within budget | `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories` | 1 passed (193s; auto-discovers + runs cycle-detection fixture) | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ----------- | ----------- | ------ | -------- |
| GRAPH-03 | 47-01, 47-03 | Private unified `analysis::solver` (VecDeque worklist, SolverBudget/BudgetStatus, SolverPolicy scaffolding, points-to fold) | ✓ SATISFIED | SC1/SC2/SC4/SC5 verified; REQUIREMENTS.md line 117 marked Complete. |
| GRAPH-04 | 47-02, 47-03 | Every derived edge carries DerivedEdgeProvenance consumable by `polint explain` | ✓ SATISFIED | SC3 verified; deletion property test passes; REQUIREMENTS.md line 118 Complete. |

No orphaned requirements: REQUIREMENTS.md maps Phase 47 = GRAPH-03, GRAPH-04 only (line 148); both claimed by plans and verified. GRAPH-05 correctly mapped to Phase 52 (Pending), not pulled into scope.

### Scope Fences (all held)

| Fence | Status | Evidence |
| ----- | ------ | -------- |
| NO `refined_calls::provider` rework (GRAPH-05/Phase 52) | ✓ HELD | `git diff ef2f5b1b^..8fc8c6b5 -- refined_calls/` → no files modified. |
| Go RTA (Phase 48) honest stub only | ✓ HELD | `GoRtaPolicy::solve` returns `PolicyOutcome::empty()`; doc names Phase 48 GO-05. |
| TS tokens (Phase 49) honest stub only | ✓ HELD | `TsTokensPolicy::solve` returns `PolicyOutcome::empty()`; doc names Phase 49 JS-04. |
| No new public CLI/SDK surface | ✓ HELD | explain seam is `pub(crate)`; leak gate green; `CacheCategoryArg::Derived` is pre-existing. |
| Points-to fixtures byte-identical | ✓ HELD | `analysis::points_to` 10 tests green incl. locked determinism/budget; budget defaults 10_000/64/512 unchanged via projection. |

### Anti-Patterns Found

None. No `TODO`/`FIXME`/`XXX`/`HACK` debt markers in solver source. The `#[cfg_attr(not(test), allow(dead_code))]` on `explain_derived_edge_provenance` and `solver_derived_edges` is an explicitly-sanctioned (D-10/D-13) test-exercised seam whose production consumer lands in Phase 52 — facts are stored unconditionally on every run so the determinism gate and cache key observe them. Honest stubs (Go/TS) producing `empty()` are the D-07 reserved-but-stubbed contract, not stubs in the goal-blocking sense.

### Human Verification Required

None. All success criteria are programmatically verifiable and were verified by running the load-bearing gate commands directly.

### Gaps Summary

No gaps. All 5 ROADMAP Success Criteria are TRUE in the code, both requirement IDs (GRAPH-03, GRAPH-04) are satisfied, every artifact exists, is substantive, is wired into production dispatch, and data flows (the provider runs in the kernel and emits derived edges + provenance that the determinism gate observes). All scope fences held. The interrupted prior run left the tree clean with all 9 atomic commits intact; this fresh verification confirms goal achievement.

---

_Verified: 2026-06-02T10:05:00Z_
_Verifier: Claude (gsd-verifier)_
