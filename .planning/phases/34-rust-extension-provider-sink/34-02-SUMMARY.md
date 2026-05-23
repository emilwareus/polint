# Plan 34-02 Summary: Extension Host and Protocol

## Outcome

Implemented the crate-private extension protocol and process host foundation:

- Added versioned handshake and provider-run protocol payloads with `deny_unknown_fields`.
- Added a bounded command host that invokes `cargo run --manifest-path <path> -- handshake` and `run-provider <provider>` through explicit `std::process::Command` args.
- Added deterministic host failure classification and controlled `polint/extension` diagnostics.

## Files Changed

- `crates/polint/src/analysis/extensions/mod.rs`
- `crates/polint/src/analysis/extensions/manifest.rs`
- `crates/polint/src/analysis/extensions/protocol.rs`
- `crates/polint/src/analysis/extensions/host.rs`
- `crates/polint/src/diagnostics/mod.rs`

## Verification

- `cargo test --lib -p polint -- extensions::protocol` passed.
- `cargo test --lib -p polint -- extensions::host` passed.
- `cargo test --lib -p polint -- extensions::manifest` passed.
- `cargo clippy -p polint -- -D warnings` passed.

## Deviations

- None.

## Self-Check: PASSED
