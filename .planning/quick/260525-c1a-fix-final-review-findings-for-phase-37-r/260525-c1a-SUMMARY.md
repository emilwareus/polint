---
quick_id: 260525-c1a
slug: fix-final-review-findings-for-phase-37-r
status: complete
completed: 2026-05-25
---

# Quick Task 260525-c1a Summary

## Outcome

Fixed the Phase 37 final-review findings:

- `refined_calls.edge` extension candidates now validate that call-site, native function, and native symbol references resolve before merge.
- Extension refined-call payloads support `site=file_span:<relative-path>:<start-byte>` so fixtures can bind to source-stable call sites instead of run-local numeric ids.
- The extension-model eval fixture now uses the source-stable `file_span` binding.
- The direct-vs-refined eval fixture now proves Go refined-call output through a Go language invariant.
- Go refined-call unresolved rows remain scoped to dispatch/function-value style unresolved calls, with a regression preventing unrelated missing semantic references from being recast as dispatch.

## Verification

- `cargo test -p polint --lib analysis::extensions::validate --locked`
- `cargo test -p polint --lib analysis::refined_calls::go --locked`
- `cargo test -p polint --lib eval_native_fixture_runner_refined_calls_fixture_passes --locked`
- `cargo test -p polint --lib refined_call --locked`
- `cargo test -p polint --test cli --locked -- checked_in_examples_are_runnable_cli_fixtures`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
