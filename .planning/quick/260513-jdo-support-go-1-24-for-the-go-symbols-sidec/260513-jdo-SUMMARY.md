# Quick Task 260513-jdo: Support Go 1.24 for the Go symbols sidecar

## Completed

- Lowered the Go sidecar minimum from Go 1.25 to Go 1.24 in both the workspace sidecar and the embedded Rust copy.
- Pinned the sidecar dependency graph to the Go 1.24-compatible line:
  - `golang.org/x/tools v0.42.0`
  - `golang.org/x/mod v0.33.0`
  - `golang.org/x/sync v0.19.0`
- Lowered `go.work` to Go 1.24 so direct root-level Go commands also work with the supported minimum.
- Updated Go and Rust test fixtures from `go 1.25.0` to `go 1.24.0`.
- Added a Rust regression test that checks the embedded sidecar stays on the Go 1.24-compatible dependency line.

## Verification

- `go test ./...` in `tools/polint-go-symbols`
- `GOWORK=off go test ./...` in `crates/polint/go-sidecar/polint-go-symbols`
- `GOWORK=off GOTOOLCHAIN=go1.24.13 go test ./...` in `tools/polint-go-symbols`
- `GOTOOLCHAIN=go1.24.13 go test ./tools/polint-go-symbols/...`
- `cargo test -p polint symbol_graph_go -- --nocapture`
- `cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures -- --nocapture`
- `cargo test -p polint --test cli go_sensitive_writes_example_reports_write_and_readwrite_references -- --nocapture`
- `cargo test -p polint --test cli external_rule_consumes_go_symbols_and_references_through_public_sdk -- --nocapture`
- `PATH=/Users/emilwareus/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.24.13.darwin-arm64/bin:$PATH cargo test -p polint symbol_graph_go -- --nocapture`
- `PATH=/Users/emilwareus/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.24.13.darwin-arm64/bin:$PATH cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures -- --nocapture`
- `PATH=/Users/emilwareus/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.24.13.darwin-arm64/bin:$PATH cargo test -p polint --test cli go_sensitive_writes_example_reports_write_and_readwrite_references -- --nocapture`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `PATH=/Users/emilwareus/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.24.13.darwin-arm64/bin:$PATH cargo test --workspace --all-features --locked`
- `cargo fmt --all`
- `git diff --check`
- `cargo test -p polint embedded_go_sidecar_keeps_go_1_24_minimum -- --nocapture`

## Source Commit

- `f77450f` - `fix: support Go 1.24 for symbol sidecar`
- `a92a11c` - `fix: make Go sidecar version test line-ending agnostic`
