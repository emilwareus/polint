# Quick Task 260520-da2 Summary: Harden core trust boundaries and run secondary review

**Date:** 2026-05-20
**Status:** Complete

## Completed

- Added `repo_fs` helpers for canonical repo-relative reads, bounded input sizes, safe directory creation, and atomic writes.
- Wired config, baseline, AI-friendly output, runner output, source snippets, package labels, legacy analysis cache, and layer cache through bounded/safe filesystem paths.
- Kept default `.polint/cache` repo-confined while preserving externally configured cache paths as runner-controlled.
- Added Go readonly behavior for `go list` and Go sidecar package loading, plus opt-in `[languages.go] offline = true` environment policy.
- Added regression tests for symlink escapes, oversized inputs, cache/layer-cache parent symlink rejection, Go offline config, and sidecar readonly build flags.
- Ran a secondary security review scan over filesystem access, command execution, unsafe Rust, and trust-boundary surfaces.

## Verification

- `cargo fmt --all`
- `cargo test -p polint`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `go test ./...` in `tools/polint-go-symbols`
- `GOWORK=off go test ./...` in `crates/polint/go-sidecar/polint-go-symbols`
- `git diff --check`

`cargo audit` could not run because `cargo-audit` is not installed in this environment.

## Secondary Review Result

No new high/medium core issues found under the intended trust model. Repo-local Rust rules remain a trusted extension boundary with full code execution, which is consistent with product direction and should stay documented as intentional.

Residual lower-risk surfaces to keep in mind:

- Scaffolding commands such as `polint init`, `polint new-rule`, and `polint add-skill` still perform repo writes by design; they are not part of the CI `check` threat path.
- The embedded Go sidecar materializes source under the process temp directory. It is outside repo control and content-hash keyed, but still shares the normal limitations of user temp directories.
- Go semantic analysis may still use the module cache/network by default; `-mod=readonly` prevents manifest mutation, and `[languages.go] offline = true` is available for stricter CI.
