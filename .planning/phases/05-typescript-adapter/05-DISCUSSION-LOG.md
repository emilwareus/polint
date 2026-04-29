# Phase 5: TypeScript Adapter - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-04-29T16:09:00Z
**Phase:** 05-typescript-adapter
**Mode:** `--auto`
**Areas discussed:** Parser diagnostics and extraction source, TS/JS fact breadth, Component JSX and raw color heuristics, Complexity and import graph facts, Fixtures and CLI proof, Execution policy

---

## Parser diagnostics and extraction source

| Option | Description | Selected |
|--------|-------------|----------|
| Oxc AST as source of truth | Replace the current line-oriented extraction contract with Oxc parsing and AST traversal. Recommended because Phase 5 explicitly requires Oxc-backed TS/JS parsing. | x |
| Keep string scanning | Preserve current extraction and add tests around it. Lower churn but does not satisfy the phase goal honestly. | |
| TypeScript compiler services | Use richer semantic APIs immediately. More precise but out of scope for v1 syntax facts. | |

**User's choice:** Auto-selected Oxc AST as source of truth.
**Notes:** Parser errors should become stable `parser/ts` diagnostics. Best-effort extraction is acceptable only when parse diagnostics remain visible.

---

## TS/JS fact breadth

| Option | Description | Selected |
|--------|-------------|----------|
| Full Phase 5 syntax facts | Extract imports, exports, functions, arrows, methods where practical, classes, components, JSX attributes, string literals, calls, and complexity. Recommended because it maps directly to `TS-02` and `TS-03`. | x |
| Minimal parser-only hardening | Only fix parser diagnostics and keep existing extraction breadth. Too narrow for the roadmap success criteria. | |
| Semantic breadth | Add type-aware facts and module resolution. More precise but belongs to later semantic work. | |

**User's choice:** Auto-selected full Phase 5 syntax facts.
**Notes:** If classes need a dedicated model, add a narrow core fact/accessor rather than overloading unrelated facts.

---

## Component JSX and raw color heuristics

| Option | Description | Selected |
|--------|-------------|----------|
| Honest syntax heuristics | Treat PascalCase declarations, JSX-returning functions, JSX attributes, and string/template literals as syntax facts with explicit heuristic boundaries. Recommended because it supports example rules without overclaiming React semantics. | x |
| Full React semantics | Resolve component imports, hooks, JSX factories, and framework-specific conventions. Too broad for Phase 5. | |
| Raw color only | Focus only on color literals and skip component facts. Too narrow for `TS-02`. | |

**User's choice:** Auto-selected honest syntax heuristics.
**Notes:** Dynamic template expressions should not be presented as exact string values.

---

## Complexity and import graph facts

| Option | Description | Selected |
|--------|-------------|----------|
| Practical parser-backed facts | Compute basic complexity from AST control-flow constructs and feed module specifiers into `ImportFact`/graph helpers. Recommended because it satisfies `TS-03` without pulling Phase 8 graph command work forward. | x |
| Exact CFG/call graph | Build deeper graph infrastructure now. Too broad for Phase 5. | |
| Defer graph proof | Skip import graph facts until Phase 8. Too weak for `TS-03`. | |

**User's choice:** Auto-selected practical parser-backed facts.
**Notes:** Production graph commands, DOT output hardening, and full resolver behavior remain deferred.

---

## Fixtures and CLI proof

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse and expand current fixtures | Extend `tests/fixtures/ts/`, `tests/fixtures/mixed/view.ts`, and `examples/ts-design-tokens/Button.tsx`; add focused CLI JSON assertions. Recommended because it matches Phase 4 verification style. | x |
| Create a new fixture layout | More isolated but duplicates existing test surface. | |
| Unit tests only | Faster but would not prove the user-facing CLI path required by `TEST-02`. | |

**User's choice:** Auto-selected reuse and expand current fixtures.
**Notes:** Clean TS/JS fixtures should produce no `parser/ts` diagnostics; failing fixtures should trigger useful TS rule diagnostics.

---

## Execution policy

| Option | Description | Selected |
|--------|-------------|----------|
| Main branch, no worktrees | Continue the user-requested policy established in prior phases. Recommended because repository and GSD planning live together on `main`. | x |
| Temporary worktrees | Allows parallel isolation but directly conflicts with the user's instruction. | |
| Large refactor first | Broad cleanup before implementation. Too risky for a phase-scoped adapter hardening pass. | |

**User's choice:** Auto-selected main branch, no worktrees.
**Notes:** Keep plans narrow and test-driven. Use Oxc AST helpers over ad hoc line scanning where possible.

---

## the agent's Discretion

- The agent may choose exact Oxc traversal helper structure.
- The agent may decide whether to add a narrow class fact to core if needed.
- The agent may decide how much static template literal text to collect while avoiding false exactness.
- The agent may split implementation into independently verifiable parser, fact, complexity, and CLI fixture plans.

## Deferred Ideas

- Full TypeScript type checking and symbol resolution.
- Production import resolution and graph command/DOT hardening.
- Exact CFG/call graph construction.
- Full SDK/rule authoring completeness and final documentation.
