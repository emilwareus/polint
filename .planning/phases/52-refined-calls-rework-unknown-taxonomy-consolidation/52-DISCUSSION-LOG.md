# Phase 52: Refined-Calls Rework & Unknown Taxonomy Consolidation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-05
**Phase:** 52-Refined-Calls Rework & Unknown Taxonomy Consolidation
**Areas discussed:** Refined-call projection source, downstream compatibility, unknown taxonomy shape, public unknowns CLI, cache and validation proof
**Mode:** `--auto`

---

## Refined-Call Projection Source

| Option | Description | Selected |
|--------|-------------|----------|
| Solver-output projection | Make `polint.solver` derived edges canonical and project them into `RefinedCallEdgeFact`. | yes |
| Keep heuristic producers primary | Keep `framework`, `go`, `ts_js`, `summaries`, and `extensions` as independent primary refined-call producers. | |
| Hybrid without ownership | Let both solver and heuristic modules emit equivalent dynamic edges. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Solver-output projection matches GRAPH-05 and avoids duplicated semantic edges with inconsistent provenance.

---

## Downstream Compatibility

| Option | Description | Selected |
|--------|-------------|----------|
| Preserve `RefinedCallEdgeFact` | Keep the existing private fact shape so data-flow/evidence continue through `db.refined_call_edges()`. | yes |
| Rewrite downstream consumers | Make data-flow/evidence consume solver internals directly. | |
| Promote a public call graph | Add a public SDK/CLI graph surface now. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Preserving the contract is explicitly required by GRAPH-05 and keeps v1.3 public-surface discipline intact.

---

## Unknown Taxonomy Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Private aggregator module | Add `analysis::unknown_taxonomy` as one normalization boundary across providers. | yes |
| CLI-local mapping | Keep category mapping inside `cli/mod.rs`. | |
| Provider-local mapping only | Make each provider emit its own public unknown row shape. | |

**User's choice:** Auto-selected recommended default.
**Notes:** A private aggregator keeps provider facts honest while giving public JSON a stable, deterministic vocabulary.

---

## Public Unknowns CLI

| Option | Description | Selected |
|--------|-------------|----------|
| Canonical `inspect unknowns` plus compatibility alias | Add `polint inspect unknowns --format json` and preserve existing `polint unknowns --cap ...`. | yes |
| Replace old command outright | Remove `polint unknowns` and force all consumers to migrate immediately. | |
| Leave current command only | Keep `polint unknowns` and ignore the Phase 52 roadmap command. | |

**User's choice:** Auto-selected recommended default.
**Notes:** The roadmap requires `inspect unknowns`; existing tests/docs already treat `polint unknowns` as stable, so compatibility is the least disruptive path.

---

## Cache And Validation Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Digest solver output and test projection determinism | Include solver output digest in refined-call cache identity and add deterministic projection tests. | yes |
| Rely on existing upstream digests | Keep current refined-call cache inputs and trust indirect invalidation. | |
| Defer all cache work to Phase 53 | Skip Phase 52 cache participation changes. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Phase 53 owns the broad sweep, but Phase 52 must include the direct solver dependency it introduces.

---

## the agent's Discretion

- Exact `analysis::unknown_taxonomy` module layout and enum names.
- Exact shared renderer/args structure for `polint inspect unknowns` and the existing `polint unknowns` compatibility command.
- Natural plan slicing and fixture grouping.

## Deferred Ideas

- Phase 53 cache/budget consolidation.
- Phase 54 benchmark promotion gates.
- Public graph/query/eval/SDK surfaces for call graph, data-flow, evidence, solver, or semantic graph.
