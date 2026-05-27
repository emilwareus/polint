# Phase 29: Local CFG and Control Dependence - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-20
**Phase:** 29-local-cfg-and-control-dependence
**Areas discussed:** Fact shape and provider boundary, CFG views and edge semantics, Derived analyses, Validation/cache/evaluation
**Mode:** `--auto`

---

## Fact Shape and Provider Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Private CFG facts over existing semantic MIR | Add crate-private CFG fact families and provider wiring after `polint.semantic_mir`, preserving public API discipline. | yes |
| Public SDK CFG view now | Promote `Cfg<'_>` and supported `cfg` capability in this phase. | |
| Placeholder-only graph command | Keep CFG as a diagnostic/export placeholder without real internal fact families. | |

**User's choice:** Auto-selected recommended default: private CFG facts over existing semantic MIR.
**Notes:** This follows Phase 28, the v1.2 substrate-first roadmap, and the public API visibility contract.

---

## CFG Views and Edge Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Typed edge kinds with named internal views | Model normal, abrupt, exceptional/cleanup, unknown, and synthetic transfers explicitly, with view/precision labels. | yes |
| One untyped adjacency graph | Store only basic predecessor/successor links. | |
| Exact language parity upfront | Attempt full Go SSA and full JS async/finally precision before exposing any rows. | |

**User's choice:** Auto-selected recommended default: typed edge kinds with named internal views.
**Notes:** This preserves truthfulness while giving later data-flow and evidence phases enough structure.

---

## Derived Analyses

| Option | Description | Selected |
|--------|-------------|----------|
| Derived reachability/dominance/postdominance/control dependence | Build and validate CFG first, then compute deterministic derived facts over graph views. | yes |
| Language builders emit all derived facts directly | Push dominance/control-dependence logic into Go and TS/JS lowering. | |
| Defer derived facts to later phases | Emit only nodes/edges in Phase 29. | |

**User's choice:** Auto-selected recommended default: derived reachability/dominance/postdominance/control dependence.
**Notes:** Phase 29's requirement explicitly includes dominance, postdominance, and control dependence.

---

## Validation, Cache, and Evaluation

| Option | Description | Selected |
|--------|-------------|----------|
| Full internal validation and deterministic eval snapshots | Add metadata, invariant validation, cache-key/output-digest participation, Go/TS fixture snapshots, and public no-leak proof. | yes |
| Minimal tests only | Add unit tests for graph algorithms but defer eval/cache/no-leak checks. | |
| Public benchmark gate now | Add external benchmark adapters and promotion gates in this phase. | |

**User's choice:** Auto-selected recommended default: full internal validation and deterministic eval snapshots.
**Notes:** This matches prior phase patterns and keeps public promotion deferred until evidence exists.

---

## the agent's Discretion

- Exact Rust module layout for CFG internals.
- Number and boundaries of implementation plans.
- Whether CFG storage lives directly in `AnalysisDb` sidecars or behind a private store/session wrapper.
- Exact algorithm implementation for first-pass dominators/postdominators, provided deterministic behavior and correctness tests are included.

## Deferred Ideas

- Public `Cfg<'_>` SDK view and supported `cfg` capability.
- Direct call facts and call graph behavior.
- Abstract-domain transfer functions over CFG.
- Summary/effects control summaries.
- Repo-local extension overlay sink.
