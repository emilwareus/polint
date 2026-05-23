# Plan 34-05 Summary: Extension Cache Identity and Quarantine

## Outcome

Implemented extension cache-key construction and verified quarantine behavior:

- Added an extension layer-key constructor using extension source, dependency, manifest, protocol, options, declared reads, input fact digests, and dependency layer digests.
- Reused existing Phase 33 quarantine behavior for real extension layer keys with non-absent `DigestKind::ExtensionCode` digests.
- Preserved native-only quarantine rejection for layers with absent or empty extension digests.

## Files Changed

- `crates/polint/src/analysis/extensions/mod.rs`
- `crates/polint/src/analysis/extensions/cache_key.rs`
- `crates/polint/src/analysis_kernel/provider.rs`

## Verification

- `cargo test --lib -p polint -- extensions::cache_key quarantine extension_no_cache` passed.
- `cargo clippy -p polint -- -D warnings` passed.

## Deviations

- Cache-disabled extension execution is already independent of analysis layer reads/writes because the extension provider runs directly in the kernel and records recompute stats; no layer-cache read/write path was added in this plan.

## Self-Check: PASSED
