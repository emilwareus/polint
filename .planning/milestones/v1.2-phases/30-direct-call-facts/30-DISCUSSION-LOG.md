# Phase 30: Direct Call Facts - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-21
**Phase:** 30-direct-call-facts
**Mode:** `--auto`
**Areas discussed:** Internal boundary and provider placement, Call fact shape and identity, Direct resolution semantics, Validation/cache/debug/evaluation, Public capability contract

---

## Internal Boundary And Provider Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Internal `analysis::calls` layer | Follow MIR/places/CFG patterns, keep facts crate-private, and wire through a manifest-owned provider. | yes |
| Expand legacy `FunctionFact.calls` | Reuse the existing string call hints as the main call graph substrate. | |
| Public provider trait first | Introduce a generic call provider registry before native fact contracts are validated. | |

**Auto choice:** Internal `analysis::calls` layer.
**Notes:** This matches the bootstrap research and preserves the public API boundary. Existing `FunctionFact.calls` can remain compatibility data but should not become the semantic substrate.

---

## Call Fact Shape And Identity

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit call-site, target, and unresolved facts | Model direct calls as normalized internal rows with stable keys, metadata, statuses, and indexes. | yes |
| Single graph edge list | Store only caller-to-callee edges and infer unresolved calls from missing targets. | |
| AST/string-based calls | Keep call facts anchored to parser text and raw callee strings. | |

**Auto choice:** Explicit call-site, target, and unresolved facts.
**Notes:** MIR call operations, `PlaceId`, semantic references, and stable metadata give downstream phases a better substrate than raw parser text or a flat edge list.

---

## Direct Resolution Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Direct/binding/static only | Resolve only precise direct references, imports, constructors, and static/member bindings available from existing semantic facts. | yes |
| Include refined providers now | Add Go CHA/RTA/VTA and TS/JS function-token flow in Phase 30. | |
| Resolve dynamic forms heuristically | Emit best-effort targets for function values, dynamic members, framework dispatch, and reflection. | |

**Auto choice:** Direct/binding/static only.
**Notes:** Dynamic and setup-sensitive calls should become unresolved or unsupported facts with reasons. Refined providers need later type/value/summary/entrypoint substrate and evaluation gates.

---

## Validation, Cache, Debug, And Evaluation

| Option | Description | Selected |
|--------|-------------|----------|
| Full internal proof path | Add metadata, validation, future-fit digest keys, debug counters/snapshots, eval fixtures, and public no-leak tests. | yes |
| Minimal in-memory rows only | Add direct call rows without provider metadata, cache identity, or eval/debug proof. | |
| Public debug output by default | Expose call snapshots through stable CLI or SDK surfaces immediately. | |

**Auto choice:** Full internal proof path.
**Notes:** Phase 30 should follow the Phase 28/29 pattern: internal rows are useful only if deterministic, validated, cache-aware, and guarded from accidental public promotion.

---

## Public Capability Contract

| Option | Description | Selected |
|--------|-------------|----------|
| Keep `call_graph` unsupported | Preserve public capability diagnostics and delay `CallGraph<'_>` behavior until promotion gates. | yes |
| Promote `CallGraph<'_>` now | Expose a supported public call graph SDK view during Phase 30. | |
| Add public docs only | Document internal call facts without a supported SDK view. | |

**Auto choice:** Keep `call_graph` unsupported.
**Notes:** This is required by Phase 30 success criteria and the rule-authoring platform contract. Internal facts can exist while public capabilities remain unsupported and honest.

---

## the agent's Discretion

- Exact module/file split inside `analysis::calls`.
- Whether unresolved calls are represented as separate facts, unresolved target rows, or both.
- Whether call-site extraction and direct target resolution are one provider pass or internal subpasses.
- Whether full persistent warm-run restore lands in Phase 30 or only future-fit output digests/cache keys land first.

## Deferred Ideas

- Go CHA/RTA/VTA and interface dispatch.
- TS/JS function-token/value-flow target resolution.
- Framework dispatch, repo-local call models, synthetic entrypoints, extension sinks, and trust-boundary edges.
- Public `Calls<'_>`, public `CallGraph<'_>`, stable call docs, and public query builders.
