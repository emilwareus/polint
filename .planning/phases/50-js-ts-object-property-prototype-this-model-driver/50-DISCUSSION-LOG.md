# Phase 50: JS/TS Object/Property/Prototype/`this` Model & Driver - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-04
**Phase:** 50-JS/TS Object/Property/Prototype/`this` Model & Driver
**Areas discussed:** Object identity and allocation sites, Property reads/writes/computed keys, `this` and receiver binding, Prototype/class lookup, Budgets/cache/unknowns/proof
**Mode:** `/gsd-discuss-phase 50 --auto`

---

## Object Identity And Allocation Sites

| Option | Description | Selected |
|--------|-------------|----------|
| Stable allocation-site tokens | Compose object identity from existing TS inventory, semantic graph, file/span, lexical parent, and object kind. | yes |
| String property names only | Skip object identity and resolve from names. | |
| TypeScript type identities | Treat declared TS types as runtime object identity. | |

**User's choice:** `[auto]` Selected stable allocation-site tokens.
**Notes:** This follows the Phase 42/45 identity discipline and avoids fabricating runtime objects from names or types.

---

## Property Reads, Writes, And Computed Keys

| Option | Description | Selected |
|--------|-------------|----------|
| Exact keys plus bounded computed buckets | Exact buckets for known keys; bounded computed/unknown buckets for dynamic keys; no name-only targets. | yes |
| Collapse all properties | Treat every property on an object as one bucket. | |
| Emit broad candidate edges | Improve recall by connecting unknown properties to many possible callees. | |

**User's choice:** `[auto]` Selected exact keys plus bounded computed buckets.
**Notes:** Precision-first behavior is required. Overflow becomes explicit budget/unknown evidence.

---

## `this`, Calls, Constructors, And Receiver Binding

| Option | Description | Selected |
|--------|-------------|----------|
| Roadmap-named receiver forms only | Model arrows, methods, constructors, bound functions, `call`, and `apply` when facts are known. | yes |
| All JS runtime receiver semantics | Try to model every receiver/native/callback behavior in one phase. | |
| Defer receiver modeling | Keep all `this` cases unresolved. | |

**User's choice:** `[auto]` Selected roadmap-named receiver forms only.
**Notes:** Broad native callbacks and framework behavior stay deferred; `bind`/`call`/`apply` are included because the Phase 50 success criteria name them.

---

## Prototype, Class, And Accessor Lookup

| Option | Description | Selected |
|--------|-------------|----------|
| Bounded prototype chain | Traverse stable prototype/class links with visited-set termination and depth/fanout caps. | yes |
| Unbounded lookup | Follow prototypes until fixpoint without a hard cap. | |
| Ignore prototypes | Resolve only direct object properties. | |

**User's choice:** `[auto]` Selected bounded prototype chain.
**Notes:** Prototype/class support is required for JS-05, but dynamic mutation stays unsupported unless stable facts justify it.

---

## Budgets, Cache, Unknowns, And Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit budget/unknown evidence | Add object-model budgets, digest participation, budget diagnostics, determinism/leak/Jelly proof. | yes |
| Best-effort truncation | Cap internally but do not surface exact budget evidence. | |
| Benchmark-only proof | Only show Jelly delta without native fixtures and cache proof. | |

**User's choice:** `[auto]` Selected explicit budget/unknown evidence.
**Notes:** This matches Phases 47-49 and prevents cache hits from serving truncated object-model runs as complete.

---

## Agent's Discretion

- Exact file slicing inside `ts/object_model/` and `analysis/solver/ts_object_model/`.
- Exact fact/newtype names for object tokens, property keys, receiver places, prototype links, budget evidence, and fixture helper APIs.
- Exact provider slot and whether object extraction is a standalone provider or a closed solver snapshot, provided digests and provider-order dependencies are correct.
- Exact object budget field names under `[solver.js]`, provided they are minimal, positive-clamped, documented, and cache-participating.

## Deferred Ideas

- Broad native/library callback models remain Phase 51/adaptation or future native-model work.
- Adaptation model facts and validated framework/native models remain Phase 51.
- Refined-call projection over solver output and unknown taxonomy consolidation remain Phase 52.
- Milestone-wide cache/budget consolidation remains Phase 53.
- Final benchmark promotion gates remain Phase 54.
