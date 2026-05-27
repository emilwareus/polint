# Plan 34-06 Summary: Real Extension Eval Fixture

## Outcome

Added a real extension eval fixture and completed the internal observation coverage for extension sink results:

- Added `tests/eval-fixtures/extension/real-sink`, including a repo-local `.polint/extensions/demo` Rust binary that implements the Phase 34 handshake and provider-run protocol commands.
- Extended native eval observation to emit accepted/rejected extension facts with producer, provenance, precision, and rejection evidence.
- Added extension delta invariants for changed facts, rejected facts, real sink activation, input snapshot extension discovery, and extension provider output presence.
- Updated provider-order expectations now that `polint.extensions` is part of the kernel provider sequence.

## Files Changed

- `crates/polint/src/analysis_kernel/provider.rs`
- `crates/polint/src/analysis_kernel/incremental/run_report.rs`
- `crates/polint/src/eval/observed.rs`
- `crates/polint/src/eval/fixtures.rs`
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml`
- `tests/eval-fixtures/extension/real-sink/expected.polint-eval.toml`
- `tests/eval-fixtures/extension/real-sink/repo/.polint.toml`
- `tests/eval-fixtures/extension/real-sink/repo/.polint/extensions/demo/Cargo.toml`
- `tests/eval-fixtures/extension/real-sink/repo/.polint/extensions/demo/src/main.rs`
- `tests/eval-fixtures/extension/real-sink/repo/src/app.ts`

## Verification

- `cargo test --lib -p polint -- eval_extension` passed.
- `cargo test -p polint --test cli -- extension_no_leak` passed.
- `cargo test --lib -p polint -- eval_native_fixture_suite_covers_required_categories` passed.
- `cargo clippy -p polint -- -D warnings` passed.

## Full Workspace Attempt

- `cargo test --workspace` passed the full `polint` library suite, then failed in CLI integration tests due to `No space left on device (os error 28)` while compiling temporary rule crates and writing temp cache files. The failing output showed linker/write failures from disk exhaustion, not assertion failures from Phase 34 behavior.

## Deviations

- No `eval/model.rs` schema change was needed; the existing fact and invariant item shapes already represented extension accepted/rejected rows.
- No new public docs or SDK/runner exports were added; the existing `extension_no_leak` CLI test already guards the internal protocol and sink markers relevant to this plan.

## Self-Check: PASSED
