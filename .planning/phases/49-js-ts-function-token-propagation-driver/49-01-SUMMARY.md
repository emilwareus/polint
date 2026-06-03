---
phase: 49-js-ts-function-token-propagation-driver
plan: 01
subsystem: api
tags: [js, ts, solver, budget, cache-key, config, semantic-graph]

requires:
  - phase: 47-unified-solver-core-derived-edge-provenance
    provides: "SolverBudget/BudgetStatus, polint.solver provider slot, SolverEngine, TsTokensPolicy placeholder"
  - phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
    provides: "TS inventory/scope/direct-binding facts and TokenFlowRequired unresolved reason"
  - phase: 48-go-rta-driver
    provides: "per-language solver sub-budget/config/cache-key pattern via GoRtaSubBudget and [solver.go]"
provides:
  - "JsTokensSubBudget on SolverBudget with finite strictly-positive token caps"
  - "[solver.js] TOML config mapped through SolverConfig::to_js_sub_budget with zero-cap fallback"
  - "ts_tokens_fixpoint_v1 plus every budget.js.* knob in solver provider parameter/output digests"
  - "crate-private TsDirectBindingReason::is_function_token_handoff classifier selecting only TokenFlowRequired"
affects: [JS-04, phase-49-plan-02-ts-token-driver, solver, semantic_graph]

tech-stack:
  added: []
  patterns:
    - "JS token budget mirrors the Go RTA sub-budget pattern: crate-private caps, finite defaults, zero-config fallback, and explicit cache participation"
    - "Token-flow handoff is an unresolved-reason classifier, not a semantic-graph behavior change; property/prototype/receiver reasons remain unresolved"

key-files:
  created: []
  modified:
    - "crates/polint/src/analysis/solver/budget.rs"
    - "crates/polint/src/config/mod.rs"
    - "crates/polint/src/analysis_kernel/mod.rs"
    - "crates/polint/src/eval/go_rta.rs"
    - "crates/polint/src/eval/determinism_gate.rs"
    - "crates/polint/src/analysis/solver/cache_key.rs"
    - "crates/polint/src/analysis/solver/provider.rs"
    - "crates/polint/src/ts/binding/facts.rs"

requirements-progress: [JS-04]

duration: 55min
completed: 2026-06-03T17:02:33Z
---

# Phase 49 Plan 01: JS/TS Token Substrate Summary

Phase 49 Plan 01 prepared the JS/TS function-token driver substrate without implementing token propagation. `SolverBudget` now carries a crate-private `js: JsTokensSubBudget`, `.polint.toml` supports `[solver.js]` caps with zero fallback, solver digests include `ts_tokens_fixpoint_v1` and all JS token caps, and the TS unresolved-reason handoff now has a narrow classifier for `TokenFlowRequired` only.

## Accomplishments

- Added `JsTokensSubBudget { max_tokens_per_var, max_candidates_per_callsite, max_token_worklist_steps, max_closure_depth }` with finite strictly-positive defaults; existing cross-domain, points-to, and Go defaults remain pinned.
- Added `[solver.js]` config via `SolverJsConfig` and `SolverConfig::to_js_sub_budget()`, threaded through the kernel and eval budget builders so fixture config can drive Plan 02.
- Added `ts_tokens_fixpoint_v1` and every `budget.js.*` field to solver provider parameter digests and output digests; tests prove JS token budget changes invalidate both.
- Added `TsDirectBindingReason::is_function_token_handoff()` and tests proving only `TokenFlowRequired` is eligible; `PropertyFlowRequired`, `PrototypeModelRequired`, and `ThisModelRequired` remain unchanged and unresolved.

## Task Commits

1. Task 1: JS token budget/config substrate - `03e6c07a`
2. Task 2: JS token cache/output digest participation - `b137c2d7`
3. Task 3: TS token unresolved-reason handoff classifier - `492b12e5`

## Verification

- `cargo test -p polint analysis::solver::budget` - passed
- `cargo test -p polint analysis::solver::cache_key` - passed
- `cargo test -p polint config::tests::solver` - passed
- `cargo test -p polint ts::binding` - passed
- `cargo test -p polint analysis::semantic_graph` - passed
- Pre-commit `make lint` ran on every task commit and passed (`cargo fmt --all -- --check` plus clippy workspace/all-targets/all-features with `-D warnings`).

## Deviations

- The plan's combined Cargo filters used invalid syntax (`cargo test` accepts one test-name filter). I ran the same coverage as separate valid filters.
- While adding `[solver.js]`, I also threaded JS budget config through the existing kernel/eval `SolverBudget` builders so Plan 02 fixtures can use `.polint.toml` without a follow-up plumbing patch.

## Next Phase Readiness

Plan 49-02 can now replace the `TsTokensPolicy` stub with a real private `analysis::solver::ts_tokens` driver using the configured JS token budget, cache-safe algorithm versioning, and a narrow `TokenFlowRequired` handoff. Semantic graph behavior for unresolved property/prototype/receiver cases is unchanged.

## Self-Check: PASSED

- Summary artifact: `.planning/phases/49-js-ts-function-token-propagation-driver/49-01-SUMMARY.md`
- Required commits: `03e6c07a`, `b137c2d7`, `492b12e5`
- Key invariant: no public SDK/runner/CLI surface promoted; all new solver/token controls remain crate-private.
