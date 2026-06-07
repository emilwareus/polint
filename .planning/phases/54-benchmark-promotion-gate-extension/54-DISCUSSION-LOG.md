# Phase 54: Benchmark Promotion Gate Extension - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-05
**Phase:** 54-Benchmark Promotion Gate Extension
**Mode:** `/gsd-discuss-phase 54 --auto`
**Areas discussed:** promotion strictness, flooding protection, F0.5 and deltas, polyglot canary, public API leak gate, benchmark evidence, gate configuration, milestone closeout

---

## Promotion Strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Hard per-suite floors | Fail the gate when any configured suite/language/scoring-mode precision floor is missed. | ✓ |
| Aggregate score | Allow strong suites to compensate for weak suites through an overall average. | |
| Advisory-only floors | Report precision floors but do not block promotion. | |

**User's choice:** Auto-selected hard per-suite floors.
**Notes:** Go floor is fixed at >=60%; Jelly floor is configurable per suite/gate config.

---

## Flooding Protection

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit flooding failure | Reject low-precision synthetic or benchmark rows even when recall improves. | ✓ |
| Recall-first promotion | Allow recall improvement to pass when precision falls. | |
| Manual review only | Surface flooding risk but require human review to block promotion. | |

**User's choice:** Auto-selected explicit flooding failure.
**Notes:** Recall claims must not bypass precision floors.

---

## F0.5 And Deltas

| Option | Description | Selected |
|--------|-------------|----------|
| Add F0.5 and per-language deltas | Track beta=0.5 and enforce/report deltas by language, suite, scoring mode, and precision tier. | ✓ |
| F1 only | Continue reporting only existing F1/F2/F3 metrics. | |
| Milestone-wide deltas only | Report one aggregate delta for all languages and suites. | |

**User's choice:** Auto-selected F0.5 with per-language deltas.
**Notes:** Existing F1 stays visible for continuity.

---

## Polyglot Canary

| Option | Description | Selected |
|--------|-------------|----------|
| Promote existing canary | Use `tests/eval-fixtures/polyglot-canary/go-ts/` and existing Go/TS canary tests in the gate path. | ✓ |
| Build a new canary | Create a separate mixed-language fixture for promotion. | |
| Defer canary to manual regression | Keep canary tests outside promotion gating. | |

**User's choice:** Auto-selected promote existing canary.
**Notes:** The canary must prove Go RTA, TS token, TS object-model, and no cross-language edge behavior.

---

## Public API Leak Gate

| Option | Description | Selected |
|--------|-------------|----------|
| Use existing leak gate | Keep `public_surface_leak.rs` and CI leak-gate as the canonical guard. | ✓ |
| Expand prelude allowlist | Add v1.3 internals to `ALLOWED_PRELUDE`. | |
| Documentation-only guard | Rely on visibility comments and manual review. | |

**User's choice:** Auto-selected existing leak gate.
**Notes:** If the leak gate fails, fix visibility rather than relaxing the allowlist.

---

## Benchmark Evidence

| Option | Description | Selected |
|--------|-------------|----------|
| Measured final audit | Record actual Go/Jelly precision and recall with commands and limitations. | ✓ |
| Claim target from roadmap | State the <3% to >25-30% recall target as achieved without fresh evidence. | |
| Native-only proof | Ignore external Go/Jelly adapters for final audit. | |

**User's choice:** Auto-selected measured final audit.
**Notes:** Unavailable external suites must be marked skipped or limited.

---

## Gate Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| Internal eval config | Extend internal suite/gate structs and tests. | ✓ |
| Public CLI config | Add user-facing promotion-gate flags or JSON contract. | |
| Parallel gate framework | Add a separate benchmark gate implementation. | |

**User's choice:** Auto-selected internal eval config.
**Notes:** `PromotionGateThresholds` and `SuiteGateConfig` are the likely extension points.

---

## Milestone Closeout

| Option | Description | Selected |
|--------|-------------|----------|
| Reconcile state and audit | Update roadmap/requirements/state and final audit after verification. | ✓ |
| Code-only finish | Leave planning state untouched. | |
| Broad roadmap rewrite | Rework prior phase scopes while closing v1.3. | |

**User's choice:** Auto-selected reconcile state and audit.
**Notes:** Stale status rows may be corrected as documentation hygiene only.

## the agent's Discretion

- Exact F0.5 storage location, provided report compatibility and schema tests stay honest.
- Exact deterministic gate row shape.
- Exact conservative Jelly default floor, if no prior value exists.
- Exact implementation plan slicing.

## Deferred Ideas

- Public SDK exposure for benchmark/solver/gate internals.
- New external benchmark corpora.
- Public benchmark dashboards.
