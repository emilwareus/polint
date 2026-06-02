# Phase 47: Unified Solver Core & Derived-Edge Provenance - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-02
**Phase:** 47-Unified Solver Core & Derived-Edge Provenance
**Mode:** `/gsd:discuss-phase 47 --auto` (autonomous — recommended option auto-selected per question)
**Areas discussed:** Solver core & points_to folding, Budget/Policy generalization, Derived-edge provenance, Dependency contract & cycle detection, Provider wiring & gates

---

## Solver Core & Folding `points_to::solver` In

| Option | Description | Selected |
|--------|-------------|----------|
| New `analysis::solver` core; fold points-to as first sub-domain (composition) | Unified core owns worklist/budget/policy; existing points-to fixpoint becomes the first `SolverPolicy` impl, fixtures byte-identical | ✓ |
| Keep `points_to::solver` authoritative; thin unified wrapper delegates | Less refactor but the core doesn't truly own the abstraction the roadmap requires | |
| Rewrite a fresh generic solver, retire points-to engine | Highest risk of points-to precision/determinism regression | |

**Auto-selected:** New `analysis::solver` core; fold points-to in as the first sub-domain by composition (recommended default).
**Notes:** Roadmap says "folds v1.2's `points_to::solver` in as a sub-domain." Acceptance bar = points-to snapshot/determinism fixtures stay byte-identical. Naming-collision guard doc comment mandatory (mirrors Phase 44 D-09).

---

## Budget Model & `SolverPolicy` Scaffolding

| Option | Description | Selected |
|--------|-------------|----------|
| Unified `SolverBudget`/`BudgetStatus`; `PointsToBudget` becomes sub-domain projection; policy trait + 1 real impl, Go/TS stubs | Generalizes proven points-to knobs; honest emptiness for not-yet-built drivers | ✓ |
| Per-sub-domain budgets only, no unified budget type | Fails GRAPH-03's "explicit `SolverBudget`/`BudgetStatus`" criterion | |
| Build Go/TS policy impls now | Out of scope — that's Phases 48/49 | |

**Auto-selected:** Unified budget generalizing points-to knobs; `SolverPolicy` trait with exactly one real impl (points-to), Go/TS as honest stubs (recommended default).
**Notes:** Budget exhaustion surfaces as `BudgetExceeded` facts, never silent drops (Phase 44 honest-precision discipline). Precision ceiling rejects `Exact` on derived edges.

---

## Derived-Edge Provenance (GRAPH-04)

| Option | Description | Selected |
|--------|-------------|----------|
| `DerivedEdgeProvenance { contributing_facts (total-ordered by stable ID), constraint_kind, solver_step }` on every derived edge; deletion property test; consumed by existing `polint explain` | Matches the three roadmap-named fields; byte-stable; load-bearing | ✓ |
| Store provenance in a side table keyed by edge | Weaker coupling; harder to prove deletion invalidation | |
| Add a new public CLI surface for provenance | Violates v1.3 single-new-public-surface rule (that's `inspect unknowns`, Phase 52) | |

**Auto-selected:** Struct carried on every derived edge, contributing facts total-ordered by stable ID, consumed by the existing `polint explain` surface (recommended default).
**Notes:** Deletion property test proves provenance is sound, not decorative. Total-ordering reuses the Phase 42 dedup total-order rule.

---

## Dependency Contract & Cycle Detection

| Option | Description | Selected |
|--------|-------------|----------|
| Doc-contract (closed input set / single fixpoint / bounded outer iterations via `SolverBudget`) + cycle-detection fixture proving no solver↔summary loop | Concrete, testable mechanism behind GRAPH-03 SC4 | ✓ |
| Document the contract only, no fixture | Fails the "cycle-detection fixture proves no loop" criterion | |
| Allow bounded solver↔summary feedback | Admits a loop the roadmap forbids | |

**Auto-selected:** Doc-contract + structural guard + cycle-detection fixture (recommended default).
**Notes:** Summaries are an input, never re-fed mid-fixpoint. Bounded outer-iteration cap enforced via `SolverBudget`, surfaced as `BudgetStatus`/`BudgetExceeded`.

---

## Provider Wiring, Determinism Gate & Leak Gate

| Option | Description | Selected |
|--------|-------------|----------|
| Register private `polint.solver` provider after `semantic_graph`, before `refined_calls`; auto-enrolls in Phase 43 determinism gate; all types `pub(crate)` | Uses Phase 44's pre-reserved slot; forward-compatible with Phase 52 GRAPH-05 | ✓ |
| Emit solver output inside the `semantic_graph` provider | Conflates graph skeleton with solving; muddies the Phase 52 read point | |
| Solver as a pure library with no provider | No determinism-gate enrollment; no cache participation | |

**Auto-selected:** Register `polint.solver` in the pre-reserved slot; cache key digests budgets + upstream digests; leak gate stays green, no `ALLOWED_PRELUDE` extension (recommended default).
**Notes:** Adding a provider touches ~7 provider-order snapshot assertions — run full `cargo test -p polint` (memory: `polint-kernel-provider-snapshot-sites`).

---

## Claude's Discretion

- Internal file layout of `analysis::solver/`.
- Physically relocate the points-to fixpoint engine vs. invoke it in place as a registered sub-domain.
- Exact field shapes/newtypes of `SolverBudget`, `BudgetStatus`, `SolverPolicy`, `DerivedEdgeProvenance`.
- Alias vs. wrap `PointsToBudget`/`PointsToBudgetStatus` as the sub-domain projection.
- Exact `polint.solver` provider slot (default: after `semantic_graph`, before `refined_calls`).
- Plan slicing (3-plan default proposed in CONTEXT.md "Claude's Discretion").

## Deferred Ideas

- `refined_calls::provider` rework over solver output → GRAPH-05, Phase 52.
- Go RTA driver (`solver::go_rta`) → GO-05, Phase 48.
- JS/TS function-token propagation driver (`solver::ts_tokens`) → JS-04, Phase 49.
- JS/TS object/property/prototype/`this` model & driver → JS-05, Phase 50.
- Adaptation model layer / `ModelEdge` producer → ADAPT-01/02, Phase 51.
- Unknown-taxonomy consolidation + `polint inspect unknowns` CLI → TAX-01, Phase 52.
- Cross-family cache-key + budget consolidation → CACHE-01/02, Phase 53.
- Public SDK promotion of solver views → out of v1.3 (SDK-FUT-01).
