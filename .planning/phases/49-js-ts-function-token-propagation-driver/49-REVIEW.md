---
phase: 49
phase_name: js-ts-function-token-propagation-driver
status: passed
depth: deep
files_reviewed: 9
reviewed_at: 2026-06-03T21:27:10Z
reviewer: codex-inline
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
---

# Code Review: Phase 49 Deep Follow-up

## Summary

Deep follow-up review after fixing the previous Phase 49 findings. The prior native-fixture coverage gap is closed: semantic graph now emits private TS token `CopyEdge` constraints for simple parameter callback, returned-function, and returned-closure source flows, and the native eval gate asserts the resulting solver edges. The unused `max_closure_depth` knob has been removed from the still-unmerged budget/config/cache surface.

No new blocking issues found.

## Previous Findings

- **WR-01 fixed:** `alias_parameter_return_fixture_proves_source_flow_token_edges` now asserts direct alias, assignment, parameter callback, returned function, and returned closure target edges. The source-flow producer resolves only through existing semantic function/callsite nodes and uses uniqueness guards for same-name functions before emitting `CopyEdge`s.
- **IN-01 fixed:** `max_closure_depth` was removed from `JsTokensSubBudget`, `[solver.js]` config mapping, solver parameter digest parts, and solver output digest parts.

## Deep Review Notes

- The new source-flow projection is private to `semantic_graph::build` and emits only `ConstraintKind::CopyEdge`; it does not widen the SDK, runner, CLI, or public rule-authoring surface.
- Ambiguous function names are not guessed: the source-flow index collapses duplicate display names to `None`, so parameter/return flows only emit when the callee or function token can be resolved uniquely.
- Missing semantic endpoints are skipped rather than fabricated. Returned nested closures are summarized only to an existing captured callable target, preserving the existing semantic-node contract.
- Property/prototype/this cases remain outside the token driver boundary and are still asserted as non-targets in the eval gate.
- Semantic graph provider parameters now include `ts_token_source_flow_projection_v1`, so cached semantic graph outputs invalidate for the new projection behavior.

## Reviewed Files

- `crates/polint/src/analysis/semantic_graph/build.rs`
- `crates/polint/src/analysis/semantic_graph/cache_key.rs`
- `crates/polint/src/analysis/solver/budget.rs`
- `crates/polint/src/analysis/solver/cache_key.rs`
- `crates/polint/src/analysis/solver/provider.rs`
- `crates/polint/src/config/mod.rs`
- `crates/polint/src/eval/ts_tokens.rs`
- `tests/eval-fixtures/ts-tokens/alias-parameter-return/expected.polint-eval.toml`
- `.planning/phases/49-js-ts-function-token-propagation-driver/49-01-SUMMARY.md`

## Verification

- `cargo test -p polint eval::ts_tokens -- --nocapture`
- `cargo test -p polint cache_key -- --nocapture`
- `cargo test -p polint solver_js -- --nocapture`
- `cargo test -p polint solver_budget_default_js_sub_budget_matches_js_defaults -- --nocapture`
- `cargo clippy -p polint --all-targets -- -D warnings`
- `cargo test -p polint -q`
