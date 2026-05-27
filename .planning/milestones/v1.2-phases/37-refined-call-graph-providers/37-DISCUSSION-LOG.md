# Phase 37: Refined Call Graph Providers - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-24
**Phase:** 37-refined-call-graph-providers
**Mode:** `$gsd-discuss-phase 37 --auto`
**Areas discussed:** Provider Shape and Scope, Algorithm Tiers and Budgets, Extension and Model Integration, Graph Materialization and Queries, Validation and Public Boundary

---

## Provider Shape and Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Add refined provider outputs over existing call facts | Preserves direct call provider and layers richer edges/statuses over it. | ✓ |
| Replace the calls provider | Higher churn and risks regressing Phase 30 behavior. | |
| Defer all refined behavior to data-flow | Would leave Phase 37 without its required refined call graph substrate. | |

**User's choice:** `[auto]` selected the recommended layered provider shape.
**Notes:** Existing direct call facts already contain the baseline vocabulary; Phase 37 should refine, not replace.

---

## Algorithm Tiers and Budgets

| Option | Description | Selected |
|--------|-------------|----------|
| Cheap opt-in tiers with explicit unresolved/budget statuses | Uses framework dispatch, type/value/function-token facts, summaries, extension/model facts, and bounded points-to where available. | ✓ |
| One exact whole-program graph | Conflicts with research and project truthfulness constraints. | |
| Only Go semantic call graph first | Too narrow for a multi-language phase and ignores the completed TS/JS and extension substrates. | |

**User's choice:** `[auto]` selected cheap opt-in tiers with explicit statuses.
**Notes:** Normal baseline checks should not pay for expensive whole-program points-to.

---

## Extension and Model Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Use Phase 34 extension facts and model provenance | Keeps repo-local model edges validated, provenance-rich, and quarantine-aware. | ✓ |
| Trust extension edges as native edges | Would hide extension precision and validation risk. | |
| Defer extension participation | Would miss a core product differentiator for repo-local modeling. | |

**User's choice:** `[auto]` selected extension/model integration through existing validated sinks.
**Notes:** Default-vs-extended eval deltas should make model impact visible.

---

## Graph Materialization and Queries

| Option | Description | Selected |
|--------|-------------|----------|
| Internal graph views only | Provides useful indexes and debug/eval proof without public API commitment. | ✓ |
| Public `CallGraph` SDK view now | Premature; Phase 41 owns public query promotion. | |
| No graph view, only raw edge rows | Makes downstream data-flow and evidence phases harder to consume. | |

**User's choice:** `[auto]` selected internal graph views only.
**Notes:** Graph views should be rebuildable from normalized edge facts.

---

## Validation and Public Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Eval/debug/no-leak proof | Proves direct-vs-refined behavior, determinism, statuses, provenance, and private boundary. | ✓ |
| Unit tests only | Too weak for a precision-sensitive analysis family. | |
| Benchmark adapters now | Belongs to Phase 40. | |

**User's choice:** `[auto]` selected eval/debug/no-leak proof.
**Notes:** Every edge needs precision/status/provenance and unresolved/budget statuses must stay explicit.

---

## The Agent's Discretion

- Exact module and fact names are left to planning.
- The planner may choose whether to add `polint.refined_calls` as a new provider manifest or to use an equivalent deterministic provider split.
- The planner may defer heavyweight Go RTA/VTA or broad points-to work if the phase still proves refined providers over current substrates with honest unknowns.

## Deferred Ideas

- Phase 38 owns local plus summary-projected data-flow.
- Phase 39 owns slicing, paths, and evidence bundles.
- Phase 40 owns external benchmark adapters and promotion gates.
- Phase 41 owns public SDK query views and agent ergonomics.
