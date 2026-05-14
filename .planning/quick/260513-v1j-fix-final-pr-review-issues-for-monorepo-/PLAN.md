# Plan

Fix the final PR review issues for monorepo Go lifecycle support.

## Tasks

1. Use a checked-in root `go.work` only when it covers every selected Go module
   root; otherwise create the temporary internal workspace.
2. Add coverage for partial root `go.work` files in both the Rust lifecycle and
   Go sidecar.
3. Make the public monorepo CLI tests fail on setup-missing diagnostics instead
   of silently skipping.
4. Add a public CLI fixture for multiple Go microservice module roots with one
   `.polint/` and one `.polint.toml`.
5. Clean whitespace issues reported by `git diff --check origin/main...HEAD`.
6. Run targeted tests, full workspace tests, Go sidecar tests, clippy, and a
   second review pass.
