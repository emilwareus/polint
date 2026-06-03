---
phase: 49
phase_name: js-ts-function-token-propagation-driver
status: issues_found
depth: standard
files_reviewed: 31
reviewed_at: 2026-06-03T20:42:16Z
reviewer: codex-inline
findings:
  critical: 0
  warning: 1
  info: 1
  total: 2
---

# Code Review: Phase 49

## Summary

Reviewed the Phase 49 source/test diff for the JS/TS function-token propagation driver. The private solver implementation and production wiring are structurally sound: JS solver budgets are threaded through kernel config, the `TsTokensPolicy` uses a closed snapshot, sentinel states do not dispatch as callable targets, and public SDK/CLI surface area is not widened.

The main issue is verification scope: the end-to-end fixture name/source covers parameter, return, and closure cases, but the executable gate only asserts current direct-alias producer behavior. This is documented in the summary, but it leaves Phase 49's "assignment, parameter, return, closure" completion claim weaker than the code path actually proves.

## Findings

### WR-01: Native TS token fixture does not assert parameter, return, or closure edges

**Severity:** Warning  
**File:** `crates/polint/src/eval/ts_tokens.rs:52`  
**Related fixture:** `tests/eval-fixtures/ts-tokens/alias-parameter-return/repo/src/app.ts:46`

`alias_parameter_return_fixture_proves_represented_token_edges` asserts only `entry -> aliasTarget` and `entry -> assignedTarget` (`crates/polint/src/eval/ts_tokens.rs:58-59`). The fixture source includes parameter callback, returned function, and closure calls (`app.ts:46-50`), but the test never asserts edges to `parameterTarget`, `returnTarget`, or `closureTarget`.

That means a regression or missing implementation for those source-level flows still passes the Phase 49 native fixture. The Plan 03 summary honestly notes the current frontend only emits direct alias `CopyEdge`s, but `.planning/STATE.md` marks JS-04 complete and the test name implies broader native proof than exists.

**Recommendation:** Either keep JS-04 explicitly partial until TS frontend producers emit `CopyEdge`s for parameter/return/closure flows, or add frontend producer support plus assertions for `entry -> parameterTarget`, `entry -> returnTarget`, and `entry -> closureTarget`. If the current scope is intentional, rename the test/fixture or add a separate pending/gap artifact so the completion state does not overstate coverage.

### IN-01: `max_closure_depth` is accepted and cache-keyed but unused by the token solver

**Severity:** Info  
**File:** `crates/polint/src/analysis/solver/budget.rs:101`  
**Related files:** `crates/polint/src/config/mod.rs:96`, `crates/polint/src/analysis/solver/cache_key.rs:108`

`JsTokensSubBudget` exposes `max_closure_depth`, config accepts `[solver.js] max_closure_depth`, and the value participates in parameter/output digests. The Phase 49 solver path does not read it, so changing this knob can invalidate caches without changing solver behavior.

**Recommendation:** Remove/defer the knob until closure-depth limiting exists, or wire it into the closure/flow producer when that behavior lands. If keeping it as a reserved Phase 50/53 hook, add a comment at the config field and a small test/documentation note that it is currently cache-participating but behavior-reserved.

## Reviewed Files

- `crates/polint/src/analysis/solver/budget.rs`
- `crates/polint/src/analysis/solver/cache_key.rs`
- `crates/polint/src/analysis/solver/engine.rs`
- `crates/polint/src/analysis/solver/mod.rs`
- `crates/polint/src/analysis/solver/policy.rs`
- `crates/polint/src/analysis/solver/provider.rs`
- `crates/polint/src/analysis/solver/ts_tokens/dispatch.rs`
- `crates/polint/src/analysis/solver/ts_tokens/fixpoint.rs`
- `crates/polint/src/analysis/solver/ts_tokens/inputs.rs`
- `crates/polint/src/analysis/solver/ts_tokens/mod.rs`
- `crates/polint/src/analysis_kernel/mod.rs`
- `crates/polint/src/config/mod.rs`
- `crates/polint/src/eval/determinism_gate.rs`
- `crates/polint/src/eval/go_rta.rs`
- `crates/polint/src/eval/mod.rs`
- `crates/polint/src/eval/ts_tokens.rs`
- `crates/polint/src/ts/binding/facts.rs`
- `tests/eval-fixtures/determinism/ts_tokens/*`
- `tests/eval-fixtures/polyglot-canary/go-ts/*`
- `tests/eval-fixtures/ts-tokens/alias-parameter-return/*`
- `tests/eval-fixtures/ts-tokens/token-explosion/*`

## Verification

No new tests were run for this review. Prior Phase 49 verification recorded passing `cargo test -p polint`, `cargo clippy -p polint --all-targets`, `cargo test -p polint --test public_surface_leak`, and commit-hook `make lint`.
