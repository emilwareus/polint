# Quick Task: Fix macOS CI Go Symbol Sidecar Test Failure

## Goal

Make PR CI merge-ready after macOS failed Go symbol tests with empty sidecar output.

## Scope

- Make Go sidecar-backed tests skip only when polint reports Go symbol/reference setup is missing.
- Prevent automatic Go toolchain downloads from hanging sidecar-backed tests on CI.
- Improve Go symbol test helper failure output so unsupported support rows are visible.
- Re-run focused local verification and push the branch.

## Verification

- `cargo fmt --all`
- focused `cargo test -p polint symbol_graph_go -- --nocapture`
- `cargo clippy --workspace --all-targets -- -D warnings`
