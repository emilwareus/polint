# Phase 38: Local Plus Summary-Projected Data Flow - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-25
**Phase:** 38-local-plus-summary-projected-data-flow
**Areas discussed:** Data-flow fact shape, local graph semantics, interprocedural projection, model facts, budgets and unknowns, query-scoped path search, validation/eval/public boundary

---

## Data-Flow Fact Shape

| Option | Description | Selected |
|--------|-------------|----------|
| General value-flow facts | Build private generic data-flow nodes/edges with source/sink/taint as query/model layers over them. | ✓ |
| Taint-first facts | Build source-to-sink taint as the primary representation and generalize later. | |
| Public SDK view now | Promote `DataFlow<'_>` while the internal fact model is still new. | |

**User's choice:** `[auto]` selected general value-flow facts.
**Notes:** This follows the research recommendation and keeps Phase 38 private until promotion gates.

---

## Local Graph Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| MIR/place/CFG anchored graph | Reuse semantic MIR, places, CFG nodes, access paths, and existing stable keys. | ✓ |
| Parallel data-flow IR | Create a separate IR for data flow and bridge later. | |
| AST-level graph | Build flow directly from parser ASTs. | |

**User's choice:** `[auto]` selected MIR/place/CFG anchored graph.
**Notes:** This avoids duplicate identities and keeps parser objects out of downstream facts.

---

## Interprocedural Projection

| Option | Description | Selected |
|--------|-------------|----------|
| Direct/refined call plus summary projection | Project argument/parameter/return and summary TITO/effects through accepted call and summary facts. | ✓ |
| Whole-program path expansion | Eagerly expand all callees and all paths into one graph. | |
| Local only | Defer all interprocedural behavior. | |

**User's choice:** `[auto]` selected direct/refined call plus summary projection.
**Notes:** Missing summaries become unknown/havoc rows rather than no-flow.

---

## Model Facts

| Option | Description | Selected |
|--------|-------------|----------|
| Validated additive models | Represent sources, sinks, sanitizers, barriers, and additional steps as provenance-rich validated facts. | ✓ |
| Hard-coded rule semantics | Encode source/sink behavior directly in rule-specific logic. | |
| Model facts can override native facts | Let repo-local models delete or replace native flow. | |

**User's choice:** `[auto]` selected validated additive models.
**Notes:** Extension/model contributions remain precision-ceiling gated and quarantine-aware.

---

## Budgets and Unknowns

| Option | Description | Selected |
|--------|-------------|----------|
| Visible deterministic budget facts | Emit unknown/havoc/budget rows for unsupported or truncated flow. | ✓ |
| Silent truncation | Stop traversal when budgets are hit without leaving evidence. | |
| Exact-or-nothing | Emit only resolved facts and drop uncertain cases. | |

**User's choice:** `[auto]` selected visible deterministic budget facts.
**Notes:** Truncation must not look like clean no-flow.

---

## Query-Scoped Path Search

| Option | Description | Selected |
|--------|-------------|----------|
| Bounded internal query path search | Assemble paths on demand from compact facts with explicit limits and truncation markers. | ✓ |
| Eager all-pairs paths | Persist all possible source-to-sink paths during provider execution. | |
| Defer all path search | Only emit graph facts and leave all path behavior to Phase 39. | |

**User's choice:** `[auto]` selected bounded internal query path search.
**Notes:** Phase 38 proves bounded reachability for fixtures; Phase 39 owns rich evidence rendering and ranking.

---

## Validation, Eval, and Public Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Private validated provider with no-leak proof | Add validation/debug/eval/cache/no-leak coverage and keep public SDK/CLI unchanged. | ✓ |
| Promote public `DataFlow<'_>` now | Make the SDK placeholder real during this phase. | |
| Skip no-leak proof | Rely on private module visibility only. | |

**User's choice:** `[auto]` selected private validated provider with no-leak proof.
**Notes:** Public promotion remains Phase 41 work.

---

## The Agent's Discretion

- Exact Rust module layout and enum names.
- Whether model facts live in one module or native/extension submodules.
- Exact plan split across fact contracts, provider/cache, local graph, interprocedural projection, model facts, query search, validation/debug/eval/no-leak proof.
- Minimal shape of Phase 38 path search, as long as bounded query-scoped reachability is proven and Phase 39 can consume it.

## Deferred Ideas

- Rich slicing/evidence bundles and ranked path rendering: Phase 39.
- External benchmark adapters and promotion gates: Phase 40.
- Public `DataFlow<'_>` and stable agent ergonomics: Phase 41.
- Full IFDS/IDE, all-pairs paths, and broader language parity: future work.
