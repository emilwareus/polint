# Fix Follow-Up Cache Review Findings

## Goal

Resolve follow-up review findings in the Phase 24 layer-cache fixes.

## Scope

- Make dependency-index invalidation run on same-layer stale manifests when an exact manifest key misses.
- Bump the layer manifest schema for the new required dependency-index field.
- Ensure normal manifest writes reject metadata that the read path would reject.

## Verification

- `cargo test -p polint --lib layer_cache --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
