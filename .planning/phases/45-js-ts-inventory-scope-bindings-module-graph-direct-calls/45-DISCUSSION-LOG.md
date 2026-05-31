# Phase 45: JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-31
**Phase:** 45-JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls
**Mode:** `/gsd-discuss-phase 45 --auto`
**Areas discussed:** Inventory and span parity, Scope and binding facts, Module graph and import resolution, Semantic graph constraint emission

---

## Inventory And Span Parity

| Option | Description | Selected |
|--------|-------------|----------|
| Oxc inventory with strict Jelly parity | Use Oxc AST spans and existing span conversion; cover all roadmap function/callsite forms; feed Phase 42 Jelly renderer. | yes |
| Extend existing coarse FunctionFact only | Keep inventory inside current syntax facts even if it cannot represent every Jelly form. | |
| Benchmark adapter normalization | Patch Jelly adapter output instead of improving source identities. | |

**User's choice:** Auto-selected recommended option.
**Notes:** [auto] Inventory must improve source facts and stable identities, not duplicate renderer logic or apply benchmark-specific patches.

---

## Scope And Binding Facts

| Option | Description | Selected |
|--------|-------------|----------|
| Oxc semantic/scoping as source of truth | Add private scope/binding facts using Oxc semantic data, with AST fallback only where needed. | yes |
| Textual binding resolution | Resolve names through string parsing and local heuristics. | |
| Defer binding until token solver | Inventory callsites now and postpone direct binding entirely. | |

**User's choice:** Auto-selected recommended option.
**Notes:** [auto] Direct static binding is in Phase 45 scope; token propagation remains Phase 49.

---

## Module Graph And Import Resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse existing module_graph/oxc_resolver stack | Bridge existing ESM/CJS/tsconfig/package facts into binding resolution. | yes |
| Build resolver inside ts/scope | Keep scope and module resolution local to the new module. | |
| Only handle same-file calls | Avoid cross-module binding in this phase. | |

**User's choice:** Auto-selected recommended option.
**Notes:** [auto] Existing module graph facts are the authority; Phase 45 should not create a parallel resolver.

---

## Semantic Graph Constraint Emission

| Option | Description | Selected |
|--------|-------------|----------|
| Emit direct constraints only | Project static aliases and direct calls into `CopyEdge` and `CallConstraint`; no solver-derived edges. | yes |
| Emit full call edges now | Try to resolve token/property/object behavior in this phase. | |
| Store facts only, no constraints | Leave semantic graph integration to Phase 49. | |

**User's choice:** Auto-selected recommended option.
**Notes:** [auto] Phase 45 is a frontend producer for the semantic graph. It must not implement solver, token, property, prototype, or adaptation behavior.

---

## Agent's Discretion

- Exact internal fact names and module file layout.
- Whether inventory/scope facts are a new layer or a derived provider over existing TS syntax/semantic outputs.
- Whether to emit direct call target facts in addition to semantic graph constraints, provided existing contracts and visibility discipline hold.
- Exact plan slicing.

## Deferred Ideas

- JS/TS token propagation: Phase 49.
- JS/TS object/property/prototype/`this` model: Phase 50.
- Unified solver and derived provenance: Phase 47.
- Go sidecar: Phase 46.
- Adaptation models: Phase 51.
- Public SDK promotion: v1.4+.
