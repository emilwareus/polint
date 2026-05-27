# Phase 32: Summary Kernel and Direct Summaries - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-21
**Phase:** 32-summary-kernel-and-direct-summaries
**Areas discussed:** Summary domain scope, Summary kernel placement, TITO precision level, Memory-touch granularity, Summary-to-solver relationship, Store architecture
**Mode:** --auto (all decisions auto-selected)

---

## Summary Domain Scope

| Option | Description | Selected |
|--------|-------------|----------|
| All four core domains | ControlEffects, CallEffects, MemoryEffects, DataFlowTito at direct/local level | ✓ |
| Two domains only | ControlEffects and CallEffects, defer memory and TITO | |
| Single domain | ControlEffects only as proof of concept | |

**Auto-selected:** All four core domains (recommended by research FINAL-REPORT.md)
**Notes:** Research converges on all four being needed for downstream phases. Direct/local computation is cheap enough to do all four.

---

## Summary Kernel Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Separate analysis::summaries | New module with callable-level identity, distinct from domain solver | ✓ |
| Extend analysis::domains | Add summary computation inside existing domain infrastructure | |

**Auto-selected:** Separate analysis::summaries module
**Notes:** Summaries have different identity (callable-level vs. block/operation-level) and different cache/invalidation semantics than domain results. Following the established pattern of separate modules per fact family.

---

## TITO Precision Level

| Option | Description | Selected |
|--------|-------------|----------|
| Simple param-to-return | Parameter-to-return, receiver mutation, argument mutation by direct observation | ✓ |
| Access-path TITO | Field-level flow tracking through containers and objects | |

**Auto-selected:** Simple param-to-return (recommended default)
**Notes:** Access-path precision is complex and deferred to Phase 38 data flow. Simple TITO is sufficient for Phase 33 SCC closure and Phase 37 refined call graph.

---

## Memory-Touch Granularity

| Option | Description | Selected |
|--------|-------------|----------|
| Core resources + coarse external | Receiver, Param, Return, Local, Global, Module + MayHaveExternalEffects flag | ✓ |
| Full resource hierarchy | Per-resource tracking for FileSystem, Network, Database, Env, Process, Time | |
| Minimal | Receiver and Param only | |

**Auto-selected:** Core resources + coarse external flag
**Notes:** Full resource hierarchy requires type/value substrate (Phase 36) for meaningful precision. Coarse external flag is honest about what direct analysis can determine.

---

## Summary-To-Solver Relationship

| Option | Description | Selected |
|--------|-------------|----------|
| Hybrid: lift + dedicated pass | Lift control effects from solver, dedicated pass for TITO/memory | ✓ |
| Lift only | Derive all summaries from domain solver results | |
| Separate pass only | Independent summary builder ignoring domain results | |

**Auto-selected:** Hybrid approach
**Notes:** Control effects can be cheaply approximated from domain results (reachability, nilness). TITO and memory effects need MIR operation-level flow tracking that the solver doesn't provide.

---

## Store Architecture

| Option | Description | Selected |
|--------|-------------|----------|
| Separate SummaryStore | Callable-level identity, domain-typed payloads, own validation/cache | ✓ |
| Extend DomainOutput | Add summary facts alongside domain observation/event facts | |

**Auto-selected:** Separate SummaryStore
**Notes:** Summary keys use callable_stable_key (already defined in keys.rs), distinct from domain results which are per-body/block/operation. Separate store enables Phase 33 to query summaries independently.

---

## Claude's Discretion

- Exact Rust module layout within analysis::summaries
- Whether four domains are separate types or SummaryPayload enum variants
- Whether summary provider runs as one pass or per-domain sub-passes
- Whether to add LayerKind::DirectSummaries or extend existing vocabulary
- Deferral of any domain slot if existing facts cannot support it honestly

## Deferred Ideas

- Interprocedural SCC closure and callee summary application (Phase 33)
- Extension-authored summary providers (Phase 34)
- Framework entrypoints and trust boundaries (Phase 35)
- Access-path TITO and heap/alias tracking (Phase 36, 38)
- Public SDK summary views (Phase 41)
