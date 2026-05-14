# Summary

Fixed the final monorepo lifecycle review issues for the Phase 13 symbols and
references PR.

## Changes

- Root `go.work` files are now used only when they cover every selected Go module
  root.
- Partial or unrelated root `go.work` files now fall back to an internal
  temporary workspace for Go package loading.
- Rust lifecycle coverage now tests both covered and partial root `go.work`
  behavior.
- The Go symbols sidecar now checks root `go.work` coverage through
  `golang.org/x/mod/modfile`.
- Sidecar coverage now proves a partial root `go.work` does not block symbol and
  reference extraction across multiple configured module roots.
- External public-SDK CLI tests now fail on Go symbol setup regressions instead
  of silently skipping.
- Added a one-`.polint/`, one-`.polint.toml` monorepo fixture with multiple Go
  module roots.
- Updated setup docs and agent contract text to describe the root `go.work`
  coverage behavior.
- Cleaned trailing whitespace/newline issues reported by diff checks.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test -p polint go::lifecycle --locked -- --nocapture`
- `GOTOOLCHAIN=go1.24.13 go test ./tools/polint-go-symbols/...`
- `GOWORK=off GOTOOLCHAIN=go1.24.13 go test ./...` in
  `crates/polint/go-sidecar/polint-go-symbols`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test -p polint external_rule_consumes_go_symbols --locked -- --nocapture`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test -p polint module_graph_go --locked -- --nocapture`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test -p polint symbol_graph_go --locked -- --nocapture --test-threads=1`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test --workspace --all-features --locked`
- `git diff --check origin/main`
- `git diff --check`

## Second Review

No blocking findings found. The PR now has automated coverage for the hard
monorepo case that mattered most: one repo-local polint setup, multiple Go module
roots, and a root `go.work` that exists but does not cover those roots.
