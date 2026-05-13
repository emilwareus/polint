# Quick Task: Fix Symbol Reference PR Review Findings

## Goal

Fix the blocking issues found in the Phase 13 symbols/references PR review and rerun focused verification plus a second review.

## Scope

- Make the Go symbol sidecar usable outside a source checkout path assumption.
- Correct Go package-qualified calls and external symbol ownership metadata.
- Avoid blocking TS-only symbol/reference rules when only Go setup is missing.
- Add regression tests for the fixed cases.

## Verification

- `go test ./tools/polint-go-symbols/...`
- focused `cargo test -p polint` symbol graph tests
- `cargo fmt --all`
- targeted second review of the resulting diff
