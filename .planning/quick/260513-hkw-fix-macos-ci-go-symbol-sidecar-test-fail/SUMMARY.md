# Summary

Fixed the macOS CI merge blocker after Go symbol tests produced empty sidecar facts when the Go sidecar setup was unavailable.

## Changes

- Made Go sidecar-backed Rust unit tests skip only when polint reports `setup_missing` for Go symbol/reference capabilities.
- Made the external Go symbol SDK CLI fixture use the same setup-missing skip behavior.
- Forced source-based Go sidecar runs to use `GOTOOLCHAIN=local`, so GitHub runners do not hang on automatic Go toolchain downloads.
- Strengthened the Go symbol Rust fixture helper to fail with capability support rows and diagnostics for unsupported non-setup failures, instead of panicking later on empty symbols.

## Verification

- `go test ./tools/polint-go-symbols/...`
- `cargo test -p polint symbol_graph_go -- --nocapture`
- `cargo test -p polint --test cli external_rule_consumes_go_symbols_and_references_through_public_sdk -- --nocapture`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
