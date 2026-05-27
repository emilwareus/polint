# Phase 28: Private Semantic MIR and Place Identity - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-20
**Phase:** 28-private-semantic-mir-and-place-identity
**Mode:** auto-selected recommended defaults
**Areas discussed:** Internal boundary and provider placement, MIR shape and lowering subset, Place identity model, Unsupported semantics and uncertainty, Validation/cache/public boundary

---

## Internal Boundary And Provider Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Private analysis module integrated with kernel provider metadata | Add crate-private `analysis::{mir, places}` and wire it through internal provider/run-report/validation paths without public exposure. | Yes |
| Extend existing core AnalysisDb directly | Put MIR/place facts directly into `core::AnalysisDb` and grow that module as the semantic engine. | |
| Public SDK-first MIR view | Build a user-facing MIR view immediately and make internal shape follow the public contract. | |

**User's choice:** Auto-selected recommended default: private analysis module integrated with kernel provider metadata.
**Notes:** This follows the research recommendation and prior public API discipline. `AnalysisDb` can own or reference semantic artifacts only through a narrow internal store/session boundary.

---

## MIR Shape And Lowering Subset

| Option | Description | Selected |
|--------|-------------|----------|
| Small vertical slice for both Go and TS/JS | Lower known function bodies into a small deterministic owned subset covering binds, assignments, reads/writes, literals, identifiers, member/index access, branches, returns, and call-shaped operations. | Yes |
| One language only | Prove the MIR shape in just Go or just TS/JS before adding the second language. | |
| Broad CFG/call/dataflow-ready MIR in one pass | Attempt full CFG, call target, domain, and summary readiness in Phase 28. | |

**User's choice:** Auto-selected recommended default: small vertical slice for both Go and TS/JS.
**Notes:** Full CFG and direct call target facts are explicitly later phases. Phase 28 should produce enough shape for later phases without overclaiming exact semantics.

---

## Place Identity Model

| Option | Description | Selected |
|--------|-------------|----------|
| Access-path place keys without alias precision | Use stable roots and projections for locals, parameters, globals, temporaries, fields/properties, indexes, call returns, and unknowns while leaving alias/points-to precision to later facts. | Yes |
| Parser-node IDs as place identity | Reuse parser-native node IDs or spans as place identity. | |
| Points-to-rich place identity now | Encode heap abstraction, alias, and refined receiver precision directly in `PlaceKey`. | |

**User's choice:** Auto-selected recommended default: access-path place keys without alias precision.
**Notes:** This keeps Phase 28 stable and lets Phase 36 refine relationships between places later without invalidating identity.

---

## Unsupported Semantics And Uncertainty

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit unsupported facts/diagnostics | Emit unsupported or unknown rows with source evidence, affected places where known, conservative action, precision/status downgrade, and metadata. | Yes |
| Skip unsupported constructs | Omit constructs the first lowering pass cannot model. | |
| Fail the whole analysis on unsupported syntax | Treat unsupported semantic constructs as fatal. | |

**User's choice:** Auto-selected recommended default: explicit unsupported facts/diagnostics.
**Notes:** This preserves truthfulness and gives later CFG/domain/data-flow phases a visible precision boundary.

---

## Validation, Cache, And Public Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Private deterministic proof with no public promotion | Add deterministic MIR/place snapshots, metadata/stable-key validation, future-fit cache identity, internal eval fixtures, and public no-leak tests. | Yes |
| Public preview command for MIR | Add a public or preview CLI command to dump MIR. | |
| Implementation-only without fixture snapshots | Build internals first and add fixture snapshots later. | |

**User's choice:** Auto-selected recommended default: private deterministic proof with no public promotion.
**Notes:** This matches Phase 28's private-first scope and keeps public promotion gated by later fixture, docs, cache, temp-repo, and benchmark evidence.

---

## the agent's Discretion

- Exact module and file naming under `crates/polint/src/analysis/`.
- Whether semantic artifacts live in `AnalysisDb`, `SemanticStore`, or `AnalysisSession`.
- How to split the phase into plans.
- Which advanced constructs are conservatively unsupported in Phase 28 as long as explicit unsupported rows and deterministic snapshots exist.

## Deferred Ideas

- Full CFG/control-dependence modeling remains Phase 29.
- Direct call target facts and public call graph behavior remain Phase 30 or later.
- Abstract domains, summaries, demand queries, data flow, slicing, extension activation, benchmark gates, and public SDK query views remain later phases.
