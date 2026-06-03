# Phase 48: Go RTA Driver - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-02
**Phase:** 48-Go RTA Driver
**Mode:** `--auto` (autonomous; recommended option auto-selected per area)
**Areas discussed:** Driver integration seam, RTA input signals / frontend-extension boundary, solver_config.go.* surface & budget channel, BudgetExceeded runaway-dispatch semantics, Verification fixtures

---

## Driver Integration Seam — how `go_rta` plugs into the Phase 47 solver core

| Option | Description | Selected |
|--------|-------------|----------|
| Route through the reserved `SolverEngine` | Make `GoRtaPolicy::solve()` real, extend `PolicyOutcome` to carry `DerivedEdgeFact`s, drive the `polint.solver` provider through `SolverEngine::run()` | ✓ |
| Parallel free function | Add `derive_go_rta_edges()` beside `derive_edges()`, called directly by the provider, skipping the engine | |

**Auto choice:** Route through `SolverEngine` (recommended).
**Notes:** Phase 47 `engine.rs` module docs (lines 18–28) explicitly reserve this seam: "when the Go RTA and TS token drivers register as policies, production will route through the engine so multiple sub-domains converge under one budget." Acceptance bar: points-to derived-edge output stays byte-identical. (CONTEXT D-01/D-02/D-03/D-04.)

---

## RTA Input Signals — frontend-extension boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Extend the Go frontend SSA emission | Emit address-taken funcs + `MakeInterface`/alloc instantiated concrete types + dynamic-callsite interface/method detail (crate-private, stable-keyed) and lower them | ✓ |
| Approximate from existing facts (≈ CHA) | Use only Phase 46 method-sets + all-types as a conservative instantiated set; no new frontend signals | |

**Auto choice:** Extend the frontend (recommended).
**Notes:** GO-05's named mechanisms ("address-taken function tracking", "runtime types through interfaces") cannot be implemented without these inputs, so emitting them is in-scope for the driver, not Phase 46 creep. De-risked by the scout: the sidecar already builds the SSA program (`emit.go:99-100`) and walks instructions (`emit.go:247-248`), so harvesting is additive. Approximating from existing facts would collapse RTA to CHA and miss the 70–90% recall ceiling. (CONTEXT D-05/D-06/D-07.)

---

## `solver_config.go.*` Surface & Budget Channel

| Option | Description | Selected |
|--------|-------------|----------|
| `[solver]`/`go` config table + `GoRtaSubBudget` | Add the config table beside `ReachabilityConfig`, thread knobs (address-taken threshold, RTA caps) into a new `GoRtaSubBudget` on `SolverBudget`, digest in the cache key | ✓ |
| Hardcoded defaults only | No config surface; Go RTA caps are compile-time constants | |

**Auto choice:** Config table + sub-budget (recommended).
**Notes:** Success criterion 3 explicitly requires "per-language `solver_config.go.*` knobs (e.g., address-taken threshold)". `PointsToSubBudget` and the Phase 43 `[reachability]` table are the precedents; defaults stay byte-identical so points-to fixtures don't change. (CONTEXT D-10/D-11/D-12.)

---

## BudgetExceeded — runaway-dispatch semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse unified `BudgetStatus::BudgetExceeded` | Latch the existing run-level honest signal when an RTA cap is hit; keep already-derived edges; no new enum | ✓ |
| New Go-specific budget status | Mint a separate Go dispatch-exhaustion status | |

**Auto choice:** Reuse `BudgetStatus::BudgetExceeded` (recommended).
**Notes:** D-06 honesty discipline + byte-stability; the Phase 52 unknown taxonomy and the reserved `budget_exceeded_reasons` JSON consume the existing signal. Iteration-cap fixture is the success-criterion-2 proof. (CONTEXT D-13/D-14.)

---

## Verification Fixtures

| Option | Description | Selected |
|--------|-------------|----------|
| Full coverage | Iteration-cap fixture + new polyglot Go+TS canary + native x/tools RTA edges, with determinism + leak gates green | ✓ |
| Minimal single fixture | One RTA fixture only | |

**Auto choice:** Full coverage (recommended).
**Notes:** All four success criteria name these artifacts; the polyglot Go+TS canary does not exist yet and must be created (promoted to a hard gate later in Phase 54). Reuse the `go-x-tools-rta-callgraph` `oracle-rta` suite + adapter. (CONTEXT D-15/D-16/D-17.)

---

## Claude's Discretion

- Internal `solver/go_rta/` file layout.
- Exact `PolicyOutcome` extension + `SolverEngine` derived-edge aggregation shape.
- Which SSA instruction families to harvest + exact new Go-frontend fact/constraint shapes.
- Exact `solver_config.go.*` knob names + `GoRtaSubBudget` fields.
- Plan slicing (3 suggested slices: frontend RTA-signals → `go_rta` policy + engine routing + config/budget → verification).

## Deferred Ideas

- TS token driver (Phase 49), JS object model (Phase 50), adaptation `ModelEdge` (Phase 51).
- `refined_calls` projection + unknown-taxonomy CLI (Phase 52).
- Go VTA (PREC-FUT-01, out of v1.3).
- Hard benchmark promotion gate + canary-as-hard-gate (Phase 54, BENCH-01).
- Cross-family cache/budget consolidation (Phase 53).
- Public SDK promotion of any solver view (out of v1.3).
