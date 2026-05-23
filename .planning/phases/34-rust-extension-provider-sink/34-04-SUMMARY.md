# Plan 34-04 Summary: Extension Provider Kernel Wiring

## Outcome

Wired extension provider output into the private analysis kernel:

- Added `polint.extensions` as a crate-private provider manifest and scheduled it once per kernel run.
- Added extension orchestration that discovers local providers, runs host handshake/provider commands, validates output, records accepted/rejected rows, and emits deterministic provider output digests.
- Extended metadata validation for known extension producer ids and extension exact-precision evidence checks.
- Added internal debug rows for extension activation, accepted counts, rejected counts, precision labels, and output evidence, while adding a public CLI no-leak guard.

## Files Changed

- `crates/polint/src/analysis/extensions/provider.rs`
- `crates/polint/src/analysis/extensions/store.rs`
- `crates/polint/src/analysis/extensions/validate.rs`
- `crates/polint/src/analysis_kernel/mod.rs`
- `crates/polint/src/analysis_kernel/provider.rs`
- `crates/polint/src/analysis_kernel/validation.rs`
- `crates/polint/src/analysis_kernel/debug.rs`
- `crates/polint/src/core/mod.rs`
- `crates/polint/tests/cli.rs`

## Verification

- `cargo test --lib -p polint -- extensions::provider metadata_validation provider_manifests_cover_existing_kernel_providers kernel_run_report_records_input_snapshot_and_provider_outputs extension_facts_are_sidecar_metadata` passed.
- `cargo test -p polint --test cli -- extension_no_leak` passed.
- `cargo clippy -p polint -- -D warnings` passed.

## Deviations

- None.

## Self-Check: PASSED
