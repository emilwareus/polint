# Phase 55: SDK Query Vocabulary and Preview Contract - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-20
**Phase:** 55-SDK Query Vocabulary and Preview Contract
**Areas discussed:** Public vocabulary boundary, Capability support semantics, Module organization, Pattern and query syntax, Validation gates

---

## Public Vocabulary Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Policy-level preview views | Promote `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`, and `DataFlow<'_>` as policy query views. | ✓ |
| Raw graph/CFG/callgraph traversal | Expose lower-level graph structures directly. | |
| Delay all public names | Keep every new SDK name private until behavior phases land. | |

**User's choice:** Auto-selected recommended default.
**Notes:** This matches the milestone's one-good-way API goal and preserves raw internals.

---

## Capability Support Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Fail closed until implemented | Recognize preview capabilities but do not run rules until each family has real facts. | ✓ |
| Run with placeholder empty facts | Let rules execute against empty or placeholder views. | |
| Bypass capability diagnostics | Treat preview requests as ordinary setup-independent capabilities. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Avoids false negatives and keeps capability names honest.

---

## Module Organization

| Option | Description | Selected |
|--------|-------------|----------|
| `sdk::facts` + `sdk::policy` split | Keep fact views in `sdk::facts`; put query/pattern/result vocabulary in `sdk::policy`. | ✓ |
| Everything in `sdk::facts` | Add all public query and pattern types to the existing facts module. | |
| New public crate/module root | Create a broader new public graph/query API area. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Preserves macro compatibility while avoiding a bloated facts module.

---

## Pattern and Query Syntax

| Option | Description | Selected |
|--------|-------------|----------|
| Plain query structs and typed pattern constructors | Use `Query::new(required...)`, explicit option fields, and typed pattern constructors. | ✓ |
| Fluent builder DSL | Add chained builder methods for every query option. | |
| String query language | Add a mini-language for policy queries. | |
| Closure filters | Let rules pass closures into query evaluation. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Keeps the public API simple, analyzable, and consistent with the product direction.

---

## Validation Gates

| Option | Description | Selected |
|--------|-------------|----------|
| Compile/manifest/capability contract only | Phase 55 proves SDK names, macros, manifests, diagnostics, docs, and leak gates. | ✓ |
| Full policy query behavior | Implement event/call/control/data-flow behavior in this phase. | |
| Docs-only planning | Add documentation without code-level capability proof. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Keeps Phase 55 small and lets Phases 56-59 own runtime query behavior.

---

## the agent's Discretion

- The planner may choose exact enum/struct field names when the captured API intent is preserved.
- The planner may split SDK, macro/capability, CLI metadata, docs, and tests into separate implementation plans.

## Deferred Ideas

- Runtime query behavior is deferred to Phases 56-59.
- Generated templates are deferred to Phase 60.
- Broad docs and final promotion gates are deferred to Phases 61-62.
