# Quick Task 260513-jdo: Support Go 1.24 for the Go symbols sidecar

## Goal

Make the Go symbols sidecar build and run with Go 1.24 while remaining compatible with newer Go toolchains.

## Tasks

1. Pin the sidecar module to the Go 1.24-compatible dependency line.
   - Files: `tools/polint-go-symbols/go.mod`, `tools/polint-go-symbols/go.sum`, `crates/polint/go-sidecar/polint-go-symbols/go.mod`, `crates/polint/go-sidecar/polint-go-symbols/go.sum`
   - Verify: `go test ./tools/polint-go-symbols/...`

2. Keep Rust-side embedded sidecar tests and fixtures aligned with the Go 1.24 minimum.
   - Files: `crates/polint/src/symbol_graph/go.rs`
   - Verify: targeted `cargo test -p polint symbol_graph_go -- --nocapture`

3. Validate locally and through GitHub CI.
   - Verify: formatting, targeted tests, workspace tests/clippy as practical, push branch, check CI run.
