# Plan 34-03 Summary: Extension Sink Validation and Store

## Outcome

Implemented the first internal typed extension sink boundary:

- Added deterministic extension fact candidate payloads with family, stable key, precision, confidence/status, evidence, bindings, and payload labels.
- Added validation that separates accepted and rejected extension facts for undeclared outputs, missing bindings, invalid spans, missing precision/provenance, duplicate keys, and native conflicts.
- Added an `AnalysisDb` sidecar for accepted extension facts and rejected audit rows, with accepted facts receiving `polint.extension.<extension>.<provider>` metadata.

## Files Changed

- `crates/polint/src/analysis/extensions/mod.rs`
- `crates/polint/src/analysis/extensions/sinks.rs`
- `crates/polint/src/analysis/extensions/store.rs`
- `crates/polint/src/analysis/extensions/validate.rs`
- `crates/polint/src/analysis_kernel/metadata.rs`
- `crates/polint/src/core/mod.rs`

## Verification

- `cargo test --lib -p polint -- extensions::sinks extensions::validate extensions::store extension_facts_are_sidecar_metadata` passed.
- `cargo clippy -p polint -- -D warnings` passed.

## Deviations

- None.

## Self-Check: PASSED
