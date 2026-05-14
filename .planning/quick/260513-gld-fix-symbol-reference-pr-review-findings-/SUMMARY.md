# Summary

Fixed the Phase 13 symbols/references review findings.

## Changes

- Replaced the Go sidecar development-checkout path assumption with sidecar discovery:
  - `POLINT_GO_SYMBOLS` may point to a sidecar binary or source directory.
  - a `polint-go-symbols` binary next to the `polint` executable is used when present.
  - otherwise, an embedded source copy is materialized to a temp cache and run with `GOWORK=off`.
- Corrected Go package-qualified calls such as `fmt.Println()` so the selector is emitted as a `call` reference, not a `read`.
- Corrected external Go symbol metadata so external symbols do not claim the local use-site file as their owner.
- Preserved local Go package-level symbol file ownership so `Symbols::for_file` still works for local Go functions.
- Scoped Go setup-missing support rows to rules whose configured file globs can actually match Go files.
- Added regressions for:
  - embedded sidecar source drift,
  - TS-only symbol rules not being blocked by missing Go setup,
  - package-qualified external Go calls,
  - sidecar JSON metadata for external calls.

## Verification

- `go test ./tools/polint-go-symbols/...`
- `cargo test -p polint symbol_graph_go -- --nocapture`
- `cargo test -p polint`
- `cargo test -p polint-macros`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Second review completed after tests. No remaining blocking findings found.
