# Quick Task 260520-fpj: Fix remaining go.work repo-boundary issues and re-review

## Goal

Close the remaining Go workspace trust-boundary gaps found during the PR review and run another security review of the changed paths.

## Tasks

- Harden Rust Go lifecycle checked-in `go.work` reuse so outside or symlink-escaping `use` entries fall back to a synthetic workspace.
- Harden both Go sidecar source copies with safe repo-local file reads, package-pattern flag rejection, module-root `go.mod` validation, and checked-in `go.work` validation.
- Add regression tests for outside `go.work` use entries, symlinked `go.work`, symlinked module roots, and package-pattern flag rejection.
- Run focused Go/Rust tests, clippy, full Rust lib tests, and a secondary review pass.
