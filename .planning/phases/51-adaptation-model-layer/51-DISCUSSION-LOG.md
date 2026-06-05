# Phase 51: Adaptation Model Layer - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-04
**Phase:** 51-Adaptation Model Layer
**Areas discussed:** Model Fact Schema And Scope, Validation And Anti-Oracle Guardrails, Benchmark Adapted Mode Reporting, Solver, Cache, And Budget Integration
**Mode:** `/gsd-discuss-phase 51 --auto`

---

## Model Fact Schema And Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Private TOML models with semantic-graph validation | Add private `analysis::adaptation` facts loaded from TOML, validate against semantic graph nodes, and emit `ModelEdge` only after acceptance. | ✓ |
| Ad hoc extension facts only | Reuse extension fixtures without adding a dedicated model schema. | |
| Public SDK surface now | Promote adaptation/model facts to public rule-author APIs during v1.3. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Scope is ADAPT-01/ADAPT-02 only. v1.3 public SDK promotion remains out of scope.

---

## Validation And Anti-Oracle Guardrails

| Option | Description | Selected |
|--------|-------------|----------|
| Strict validator with oracle-sanitizer fixtures | Reject non-resolving targets, wildcard/broad patterns, exact oracle-answer matches, and forbidden oracle-path access. | ✓ |
| Permissive models with report-only warnings | Let broad or uncertain models through and rely on reports to show precision loss. | |
| Agent judgment only | Trust the adaptation agent to avoid oracle leakage without executable checks. | |

**User's choice:** Auto-selected recommended default.
**Notes:** This preserves the project truthfulness rule: adaptation may improve recall only with source-evident model facts, not answer-key leakage or broad guessing.

---

## Benchmark Adapted Mode Reporting

| Option | Description | Selected |
|--------|-------------|----------|
| Full delta report reusing existing eval adaptation structures | Record prompt hash, changed files, accepted/rejected facts, unknown delta, precision/recall delta, runtime/cache delta, and held-out subset delta. | ✓ |
| Minimal before/after score | Record only headline precision/recall deltas. | |
| Markdown-only note | Keep adapted-mode evidence as human notes without deterministic JSON. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Existing `eval::adaptation` and `eval::delta` structures are the starting point, extended only where required by Phase 51.

---

## Solver, Cache, And Budget Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Digest-complete private integration | Accepted model files/facts affect semantic graph/solver digests, model expansion is budgeted, and rejected facts emit no constraints. | ✓ |
| Uncached experimental path | Run adapted mode outside normal cache/digest accounting. | |
| Public preview surface | Expose adaptation internals through public SDK/CLI while still experimental. | |

**User's choice:** Auto-selected recommended default.
**Notes:** Phase 53 owns the milestone-wide sweep, but Phase 51 must make its own model inputs cache-visible.

---

## Agent's Discretion

- Exact TOML path and field names.
- Exact model fact, ID, rejection reason, and budget type names.
- Whether adaptation is a standalone provider or a semantic-graph input snapshot, as long as ordering and digests are deterministic.
- Exact held-out subset partition strategy.
- Plan slicing.

## Deferred Ideas

- Native callable shim library for JS built-ins remains ADAPT-FUT-01.
- Reflection and dynamic-import auto-modeling remains ADAPT-FUT-02.
- Refined-call projection and unknown taxonomy remain Phase 52.
- Milestone-wide cache/budget consolidation remains Phase 53.
- Hard benchmark promotion gates remain Phase 54.
