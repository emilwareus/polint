# Quick Task 260520-fpj Summary: Fix remaining go.work repo-boundary issues

## Completed

- Rejected checked-in `go.work` reuse when any `use` target is absolute, parent-escaping, missing an in-repository `go.mod`, or symlink-escaping.
- Mirrored the hardening in both Go sidecar copies and kept embedded sidecar source parity intact.
- Added sidecar defenses for standalone invocation: package patterns starting with `-` are rejected and module roots must have in-repository `go.mod` files.
- Added regression coverage for outside `go.work` use entries, symlinked `go.work`, symlinked module roots, symlinked `go.work use` entries, and package-pattern flag injection.
- Re-ran the secondary security review against the changed Go lifecycle and sidecar trust-boundary paths.

## Review Result

No new blocking findings remain in the reviewed boundary. Checked-in Go workspace reuse now fails closed to a synthetic workspace unless every referenced module root stays inside the repository as a readable module.

Residual trust-model notes:

- Repo-local rules remain trusted user-authored code by product design.
- `languages.go.package_patterns` remains a powerful lifecycle input; flag injection is blocked, and `offline = true` is still the strict mode for CI environments that must avoid Go network/module-cache behavior.
- Concurrent local filesystem mutation during analysis is outside the untrusted-repo-content threat model.

## Verification

- `cargo test -p polint go_work --lib`
- `cargo test -p polint go_analysis_config_rejects_package_patterns_that_start_with_dash --lib`
- `GOWORK=off go test ./...` in `tools/polint-go-symbols`
- `GOWORK=off go test ./...` in `crates/polint/go-sidecar/polint-go-symbols`
- `cargo test -p polint embedded_go_sidecar_sources_match_workspace_sources --lib`
- `cargo clippy -p polint --lib -- -D warnings`
- `cargo test -p polint --lib`
