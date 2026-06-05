# Phase 53: Cache & Solver Budgets Consolidation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-05
**Phase:** 53-cache-solver-budgets-consolidation
**Mode:** `/gsd-discuss-phase 53 --auto`
**Areas discussed:** Cache dependency ledger, Invalidation fixture strategy, Budget taxonomy and enforcement, Benchmark RSS reporting, Scope and public surface

---

## Cache Dependency Ledger

| Option | Description | Selected |
|--------|-------------|----------|
| Consolidated executable ledger | Create a single internal ledger or test matrix that audits all v1.3 cache inputs while reusing production digest helpers. | ✓ |
| Provider-by-provider comments only | Leave each provider's cache recipe documented locally and rely on local tests. | |
| New cache subsystem | Replace existing cache-key/layer-cache patterns with a new consolidation layer. | |

**User's choice:** `[auto] Selected consolidated executable ledger.`
**Notes:** This matches the phase goal without adding a second cache architecture. Existing helper recipes remain the implementation source of truth.

---

## Invalidation Fixture Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Positive and negative fixture pairs | Mutate one relevant input to require recompute, and mutate/reorder irrelevant inputs to preserve hits. | ✓ |
| Digest inequality only | Assert hashes change without observing cache behavior. | |
| Full external corpus proof | Use large benchmark corpora for all cache proof. | |

**User's choice:** `[auto] Selected positive and negative fixture pairs.`
**Notes:** Fixtures should observe hit/miss/recompute behavior where available, while staying native and small. External benchmark enforcement remains Phase 54.

---

## Budget Taxonomy And Enforcement

| Option | Description | Selected |
|--------|-------------|----------|
| Unified status with specific private reasons | Keep `BudgetStatus::BudgetExceeded` as the shared signal and attach stable reason strings for the sub-budget that tripped. | ✓ |
| Separate public budget enums | Expose a new enum per driver or fact family. | |
| Silent cap truncation | Drop work when caps are hit without making the precision loss visible. | |

**User's choice:** `[auto] Selected unified status with specific private reasons.`
**Notes:** This carries forward Phases 47-52: budget exhaustion is an honest fact/report signal, not a crash or hidden truncation.

---

## Benchmark RSS Reporting

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal internal RSS columns | Add deterministic cold/warm RSS threshold and observed columns to internal eval/benchmark reporting. | ✓ |
| Hard promotion enforcement now | Make RSS and precision floors final exit gates in Phase 53. | |
| Omit memory data | Leave memory/RSS out until Phase 54. | |

**User's choice:** `[auto] Selected minimal internal RSS columns.`
**Notes:** Phase 53 should create the report surface needed by Phase 54 without claiming final benchmark promotion.

---

## Scope And Public Surface

| Option | Description | Selected |
|--------|-------------|----------|
| Internal/test-only consolidation | Keep cache, budget, and ledger helpers `pub(crate)` or test-only; preserve public CLI/API boundaries. | ✓ |
| New public budget command | Add a user-facing command for budget inspection. | |
| SDK promotion | Expose solver/cache/budget types through the public SDK. | |

**User's choice:** `[auto] Selected internal/test-only consolidation.`
**Notes:** The only new v1.3 public CLI surface already landed in Phase 52: `polint inspect unknowns --format json`.

---

## the agent's Discretion

- Exact dependency-ledger implementation form.
- Exact stable budget-reason strings.
- Exact eval report structure for RSS fields.
- Final plan slicing.

## Deferred Ideas

- Phase 54 owns final benchmark promotion gates, hard precision floors, F-score beta=0.5, per-language deltas, and final recall claims.
- Public SDK/cache/solver/budget views remain out of v1.3.
