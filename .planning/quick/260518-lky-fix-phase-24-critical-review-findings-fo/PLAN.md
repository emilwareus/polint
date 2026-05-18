# Fix Phase 24 Critical Review Findings

## Goal

Address the critical review findings for Phase 24 layer-cache implementation without changing the public SDK surface.

## Scope

- Make dependency-index and change-set invalidation part of live layer-cache manifest validation.
- Validate syntax-layer `output_digest` against the cached payload on reads.
- Surface metrics layer cache write failures as `internal/cache` diagnostics.
- Use unique temporary paths for atomic layer-cache writes.

## Verification

- `cargo test -p polint --lib layer_cache --locked`
- `cargo test -p polint --lib metrics_layer_cache_write_failure_is_reported --locked`
- `cargo test -p polint --lib kernel_surfaces_metrics_layer_cache_write_diagnostics --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
