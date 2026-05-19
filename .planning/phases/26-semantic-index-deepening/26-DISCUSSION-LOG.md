# Phase 26: Semantic Index Deepening - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-19
**Phase:** 26-semantic-index-deepening
**Mode:** `--auto`
**Areas discussed:** Semantic provider boundary, fact model deepening, resolution and unknown handling, cache/validation/public surface

---

## Semantic Provider Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Deepen current providers first | Build on `module_graph`, `symbol_graph`, provider manifests, metadata, and layer cache; introduce private `semantic` modules only where they reduce coupling. | yes |
| Create a large new public semantic API | Promote a generic semantic graph/query surface now. | |
| Replace current symbol graph wholesale | Discard current symbol/reference provider and rebuild as a separate stack. | |

**User's choice:** Auto-selected recommended default: deepen current internal providers first.
**Notes:** This matches prior phase decisions keeping analysis internals private and avoids duplicating the existing stable-key/cache/metadata substrate.

## Fact Model Deepening

| Option | Description | Selected |
|--------|-------------|----------|
| Required semantic-index families | Add/deepen scopes, richer imports/exports, aliases, generated symbols, resolution facts, unknowns, and stable export identities. | yes |
| Only polish current symbols/references | Limit the phase to minor field tweaks on current public facts. | |
| Jump directly to call graph/MIR | Start later semantic/interprocedural phases before name/import identity is stable. | |

**User's choice:** Auto-selected recommended default: implement the Phase 26 semantic-index fact families privately first.
**Notes:** The selected scope follows `SAE-SEM-01` and the semantic-index research validation taxonomy.

## Resolution And Unknown Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Visible unknowns | Represent unresolved, ambiguous, dynamic, unsupported, setup-missing, external, and generated states as facts/status rows with provenance and precision. | yes |
| Drop unresolved facts | Omit rows that cannot be resolved exactly. | |
| Pretend resolution is exact | Collapse uncertainty into exact-looking facts. | |

**User's choice:** Auto-selected recommended default: unknowns are first-class data.
**Notes:** This carries forward the project truthfulness constraint and prior Phase 12/13/21 decisions.

## Cache, Validation, And Public Surface

| Option | Description | Selected |
|--------|-------------|----------|
| Internal proof first | Keep new semantic internals private, prove with eval fixtures, cache restore, metadata validation, and public no-leak tests. | yes |
| Promote Scopes/Imports now | Add public SDK views for every new fact family immediately. | |
| Add public semantic export now | Ship SCIP/Kythe-style or generic semantic export as part of Phase 26. | |

**User's choice:** Auto-selected recommended default: internal proof first.
**Notes:** Public docs should change only for existing supported public facts whose behavior changes.

## the agent's Discretion

- Exact internal module layout and type names.
- Whether semantic work remains under `symbol_graph` or is factored into a private `semantic` namespace.
- Exact plan split and fixture grouping.
- Conservative deferral of public SDK expansion where success criteria can be met honestly without promotion.

## Deferred Ideas

- Public `Scopes<'_>`, richer public `Imports<'_>`, semantic export, extension activation, xref search, MIR, CFG, call graph, data-flow, and broad query builders.

