# Summary

Fixed the Windows CI failure in the Phase 13 symbols/references PR.

## Cause

The Go symbol sidecar JSON contract used array fields, but the Go producer could
serialize nil slices as `null`. Windows hit that shape in the monorepo public-SDK
tests, and the Rust reader treated `null` as invalid for sequence fields.

## Changes

- Initialized sidecar output slices so `packages`, `symbols`, `definitions`, and
  `references` serialize as `[]` instead of `null`.
- Applied the same producer fix to the embedded Go sidecar source used by
  `polint`.
- Made Rust sidecar deserialization accept `null` sequence fields as empty
  vectors for tolerance with older/external sidecar binaries.
- Added Go regression coverage for empty-array JSON output.
- Added Rust regression coverage for parsing null sequence fields.

## Validation

- `GOTOOLCHAIN=go1.24.13 go test ./tools/polint-go-symbols/...`
- `GOWORK=off GOTOOLCHAIN=go1.24.13 go test ./...` in
  `crates/polint/go-sidecar/polint-go-symbols`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test -p polint sidecar_null_sequence_fields_parse_as_empty_vectors --locked -- --nocapture`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test -p polint external_rule_consumes_go_symbols --locked -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `env PATH="$(GOTOOLCHAIN=go1.24.13 go env GOROOT)/bin:$PATH" cargo test --workspace --all-features --locked`
- `cargo test -p polint --test cargo_install_smoke --locked -- --ignored`
- `git diff --check`
- `git diff --check origin/main`
