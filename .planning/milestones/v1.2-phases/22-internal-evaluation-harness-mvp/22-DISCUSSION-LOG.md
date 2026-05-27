# Phase 22: Internal Evaluation Harness MVP - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-05-17
**Phase:** 22-Internal Evaluation Harness MVP
**Mode:** `--auto`
**Areas discussed:** Harness surface, evaluation model, fixture strategy, matchers and metrics, determinism and hashing, integration boundaries

---

## Harness Surface

| Option | Description | Selected |
|--------|-------------|----------|
| Internal/test-facing harness | Keep evaluation crate-private and test-facing; no stable public CLI or SDK contract. | yes |
| Hidden CLI command | Add a hidden command if needed, still unsupported and undocumented. | |
| Public `polint eval` command | Promote eval as a user-facing CLI command now. | |

**Auto choice:** Internal/test-facing harness.
**Reason:** Phase 22 is foundation work and project conventions treat visible CLI commands as public contracts. Public eval promotion belongs later.

---

## Evaluation Model

| Option | Description | Selected |
|--------|-------------|----------|
| Full roadmap success-criteria model | Represent diagnostics, facts, graph edges, paths, invariants, and runtime budgets in expected/observed JSON. | yes |
| Diagnostics only | Start with only final diagnostics and expand later. | |
| External benchmark adapter schema first | Shape the model around OWASP or another external suite before native fixtures. | |

**Auto choice:** Full roadmap success-criteria model.
**Reason:** SAE-FND-03 explicitly requires the harness to cover all of these item kinds, and later analysis families need the shared model early.

---

## Fixture Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Native engine fixtures first | Add small native fixtures for kernel, provenance, cache, and extension invariants. | yes |
| OWASP adapter first | Start with an external benchmark adapter and scoring path. | |
| Large synthetic benchmark first | Build a broad polint-owned benchmark corpus. | |

**Auto choice:** Native engine fixtures first.
**Reason:** The roadmap says Phase 22 owns native invariants. External adapters are valuable but belong to the later benchmark adapter/promotion phase.

---

## Matchers And Metrics

| Option | Description | Selected |
|--------|-------------|----------|
| Generic matcher and metric substrate | Add matchers for diagnostics/facts/edges/paths/invariants/budgets and basic precision/recall/runtime metrics. | yes |
| Exact JSON equality only | Compare complete JSON blobs without tolerant or partial matching. | |
| Benchmark-specific scorecards only | Implement OWASP/RealVuln-style scoring before generic matching. | |

**Auto choice:** Generic matcher and metric substrate.
**Reason:** The harness needs reusable comparison semantics before external benchmark scorecards can be trustworthy.

---

## Determinism And Hashing

| Option | Description | Selected |
|--------|-------------|----------|
| Machine-stable deterministic output | Sort observable rows and exclude timestamps, absolute paths, temp roots, and transient runtime details from hashes. | yes |
| Human-readable output only | Prefer readable reports without byte-stable JSON/hash assertions. | |
| Include full runtime environment in hashes | Hash machine-specific paths and timing data into outputs. | |

**Auto choice:** Machine-stable deterministic output.
**Reason:** Deterministic expected/observed JSON and machine-stable output hashes are explicit Phase 22 success criteria.

---

## Integration Boundaries

| Option | Description | Selected |
|--------|-------------|----------|
| Fixture proofs over current internals | Reuse current kernel, metadata, diagnostics, and cache surfaces while modeling later extension/cache invariants without implementing those later phases. | yes |
| Implement future cache and extension substrates now | Fold Phase 23/24/34 work into the evaluation harness phase. | |
| Defer cache and extension invariant coverage | Avoid cache/extension fixture shapes until those later phases. | |

**Auto choice:** Fixture proofs over current internals.
**Reason:** Phase 22 must cover cache and extension invariants, but typed cache vocabulary, layer persistence, and real extension activation are explicitly later phases.

---

## the agent's Discretion

- Exact internal module and type names.
- Exact fixture manifest format and fixture directory placement.
- Exact split across model, matcher, metrics, runner, and native fixture implementation plans.
- Whether to use snapshot testing, direct JSON assertions, or both.

## Deferred Ideas

- External benchmark adapters and suite-specific scorecards.
- Stable public `polint eval` CLI and public eval JSON schemas.
- Benchmark tiers, baselines, and regression gates.
- Provider stats, typed cache keys, persistent layer cache, and extension activation.
