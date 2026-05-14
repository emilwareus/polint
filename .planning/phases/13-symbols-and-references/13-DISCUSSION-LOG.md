# Phase 13: Symbols and References - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-12
**Phase:** 13-symbols-and-references
**Mode:** `--auto`
**Areas discussed:** Public SDK and capability boundary, fact model and precision, derivation pipeline, language semantics, lifecycle/future-analysis fit, cache/setup/diagnostics, external-consumer proof

---

## Public SDK And Capability Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Narrow typed SDK views | Add `Symbols<'_>` and `References<'_>` through the SDK/prelude and macro capability mapping. | yes |
| Broad `RuleCtx` accessors | Put symbol/reference access directly on `RuleCtx` as broad query methods. | |
| Expose engine/parser internals | Let rule authors inspect Oxc scopes, Go objects, sidecar JSON, or raw AST nodes. | |

**Auto choice:** Narrow typed SDK views.
**Rationale:** This carries forward Phase 11 and Phase 12 decisions that normal rule-authoring should use typed fact-view parameters and that internals stay private.

---

## Fact Model And Precision

| Option | Description | Selected |
|--------|-------------|----------|
| Separate symbols, definitions, and references with explicit precision/status | Preserve declarations and uses as different concepts, expose uncertainty honestly, and use stable semantic IDs. | yes |
| Collapse definitions into references | Simpler model but weaker for API surface, declaration, import/export, and ownership rules. | |
| Exact-only model | Hide unresolved/setup-missing/ambiguous facts until perfect resolution exists. | |

**Auto choice:** Separate facts with precision/status.
**Rationale:** This satisfies SYM-04 and keeps later call graph/dataflow work from rebuilding identity and uncertainty semantics.

---

## Derivation Pipeline

| Option | Description | Selected |
|--------|-------------|----------|
| Cross-file `symbol_graph` provider | Follow the Phase 12 module graph pattern: syntax first, module graph as needed, symbol graph before rules. | yes |
| Per-file adapter-only extraction | Keep all symbol extraction inside Go/TS adapters with no cross-file provider. | |
| Graph database first | Introduce a graph database or large query engine before facts stabilize. | |

**Auto choice:** Cross-file `symbol_graph` provider.
**Rationale:** Symbols/references need cross-file/module context but should remain behind polint-owned facts and SDK views.

---

## Language Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Oxc semantic for TS/JS and Go sidecar with `go/packages`/`go/types` | Use the strongest current language-native sources without overclaiming max precision. | yes |
| Syntax-only for both languages | Faster and simpler but fails SYM-02/SYM-03 intent for typed package information and Oxc semantic facts. | |
| Full compiler/type-checker sidecars for both languages immediately | More powerful but too broad for Phase 13 and would pull in future type-aware/dataflow scope. | |

**Auto choice:** Oxc semantic for TS/JS and Go typed sidecar.
**Rationale:** This matches the research recommendation and keeps TypeScript compiler sidecar and Go SSA/call graph work deferred.

---

## Lifecycle And Future Analysis Fit

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal Phase 13 lifecycle hooks only if needed | Avoid blocking future lifecycle architecture while keeping the public API small now. | yes |
| Ship broad lifecycle/plugin API in Phase 13 | Expose `ScanLifecycle`, command declarations, and fact providers immediately. | |
| Ignore lifecycle architecture entirely | Implement symbols in a way that may need rework for call graph/dataflow/lifecycle later. | |

**Auto choice:** Minimal hooks only if needed.
**Rationale:** The lifecycle research is important, but Phase 13 should implement symbols/references without committing to a broad public extension API prematurely.

---

## Cache, Setup, And Diagnostics

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic setup-aware cache and diagnostics | Include behavior-affecting inputs in digests and emit actionable setup diagnostics. | yes |
| Best-effort cache with parser-style errors | Simpler but risks stale or misleading facts. | |
| Disable caching for symbol facts | Correct but likely too slow and inconsistent with the product's performance goals. | |

**Auto choice:** Deterministic setup-aware cache and diagnostics.
**Rationale:** This carries forward Phase 7, Phase 11, and Phase 12 cache/support decisions.

---

## External-Consumer Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Temp-repo tests through public SDK imports only | Prove rule authors can request and consume facts as external users. | yes |
| Internal unit tests only | Faster but misses the rule-author platform contract. | |
| Example-only proof | Useful documentation, but not enough regression coverage. | |

**Auto choice:** Temp-repo tests through public SDK imports only.
**Rationale:** AGENTS.md requires external-consumer proof when adding rule-authoring features.

---

## the agent's Discretion

- Exact internal module names, enum variant names, query method names, and plan split are left to the planner/executor within the constraints captured in `13-CONTEXT.md`.

## Deferred Ideas

- Broad lifecycle API, TypeScript compiler sidecar, Go SSA/call graph, CFG/dataflow/coverage/test metrics, and project-level symbol graph caching are deferred beyond Phase 13 unless strictly required for SYM-01 through SYM-04.
