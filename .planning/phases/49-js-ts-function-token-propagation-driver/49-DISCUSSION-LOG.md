# Phase 49: JS/TS Function-Token Propagation Driver - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-03
**Phase:** 49-JS/TS Function-Token Propagation Driver
**Mode:** `--auto`
**Areas discussed:** Solver integration seam, Token carrier and propagation scope, Budget and too-many-tokens behavior, Config and cache participation, Verification and benchmark proof

---

## Solver Integration Seam

| Option | Description | Selected |
|--------|-------------|----------|
| Use unified solver policy | Replace the `TsTokensPolicy` stub and route through `SolverEngine::run_to_solver_output`. | yes |
| Build separate TS provider | Create a dedicated provider outside the solver. | |
| Defer integration | Leave the policy stub and only prepare inputs. | |

**User's choice:** Auto-selected recommended default: Use unified solver policy.
**Notes:** Phase 47 reserved the seam and Phase 48 proved it with Go RTA. Phase 49 should follow the same shape.

---

## Token Carrier And Propagation Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Function-token set over existing identities | Propagate tokens keyed by TS inventory function identities / semantic function nodes. | yes |
| Synthetic callable symbols | Invent a separate callable identity family. | |
| Broad object/value tokens | Include object/property/prototype/value modeling in this phase. | |

**User's choice:** Auto-selected recommended default: Function-token set over existing identities.
**Notes:** Broad object/property/`this` behavior belongs to Phase 50. Phase 49 stays focused on JS-04 function-token flow.

---

## Budget And Too-Many-Tokens Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Sentinel plus BudgetExceeded | Collapse overflowing token sets to `"too-many-tokens"` and latch `BudgetExceeded`. | yes |
| Truncate silently | Keep first N tokens and drop the rest. | |
| Disable propagation for large inputs | Stop token propagation globally when a large input appears. | |

**User's choice:** Auto-selected recommended default: Sentinel plus BudgetExceeded.
**Notes:** This preserves the v1.3 honesty discipline: no silent precision loss and no fabricated edges.

---

## Config And Cache Participation

| Option | Description | Selected |
|--------|-------------|----------|
| Add `[solver.js]` budget knobs | Thread JS token caps through `SolverBudget` and solver digests. | yes |
| Reuse Go budget knobs | Overload `solver.go.*` or cross-domain caps for JS behavior. | |
| Hard-code defaults | No config surface for Phase 49. | |

**User's choice:** Auto-selected recommended default: Add `[solver.js]` budget knobs.
**Notes:** This mirrors Phase 48's `[solver.go]` pattern while keeping the public surface limited to `.polint.toml` config.

---

## Verification And Benchmark Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Native fixtures plus Jelly/polyglot gates | Add token-flow, token-explosion, Jelly delta, determinism, leak, and polyglot canary proof. | yes |
| Unit tests only | Prove the fixpoint in isolation but skip eval fixtures. | |
| Benchmark promotion only in Phase 54 | Defer all benchmark proof to the final gate. | |

**User's choice:** Auto-selected recommended default: Native fixtures plus Jelly/polyglot gates.
**Notes:** Phase 54 owns hard promotion gates, but Phase 49 must prove JS-04 produces real recall improvement without flooding precision.

---

## Agent's Discretion

- Internal `analysis::solver::ts_tokens` file layout.
- Exact token-set, token-variable, sentinel, and sub-budget type names.
- Natural plan slicing, provided the final plan covers inputs, policy/fixpoint, config/cache, and verification.

## Deferred Ideas

- JS/TS object/property/prototype/class/`this` modeling remains Phase 50.
- Adaptation model facts remain Phase 51.
- Refined call projection and unknown taxonomy remain Phase 52.
- Cache/budget consolidation remains Phase 53.
- Hard benchmark promotion gates remain Phase 54.
