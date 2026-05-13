---
phase: 13-symbols-and-references
review: 13-REVIEW.md
fixed: 2026-05-13T08:05:00Z
status: fixed
findings_fixed:
  critical: 0
  warning: 3
  info: 0
commit: 5967507
---

# Phase 13 Code Review Fix Summary

Fixed all three warnings from `13-REVIEW.md`.

## Fixes Applied

### WR-01: Stable IDs include transient FileId values

- Removed `Span::file` from stable key span encoding so IDs do not depend on transient `AnalysisDb` insertion order.
- Added `stable_ids_do_not_include_transient_file_ids` to lock the behavior.

### WR-02: Unknown Go sidecar precision is upgraded to ExactSemantic

- Changed unknown Go sidecar precision strings to map to `SymbolPrecision::Unsupported`.
- Added `unknown_go_reference_precision_is_unsupported`.

### WR-03: Go write references are advertised but never emitted

- Added Go sidecar assignment classification for identifiers and selectors.
- `=` and `:=` uses classify as `write`; compound assignments and inc/dec classify as `read_write`.
- Added `TestEmitClassifiesAssignmentReferences`.

## Verification

- `gofmt -w tools/polint-go-symbols/internal/symbols/emit.go tools/polint-go-symbols/internal/symbols/emit_test.go`
- `cargo fmt --all -- --check`
- `go test ./tools/polint-go-symbols/...`
- `cargo test -p polint --lib stable_ids_do_not_include_transient_file_ids --locked`
- `cargo test -p polint --lib unknown_go_reference_precision_is_unsupported --locked`
