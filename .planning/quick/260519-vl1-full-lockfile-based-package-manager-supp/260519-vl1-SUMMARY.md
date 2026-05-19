# Quick Task 260519-vl1: Full lockfile-based package manager support - Summary

**Completed:** 2026-05-19
**Status:** Complete

## Changes

- Added crate-private JS lockfile parsing for npm package locks/shrinkwrap, pnpm, Yarn Classic, Yarn Berry, and text `bun.lock`.
- Added TS/JS package-manager selection that prefers valid `packageManager`, infers from one supported lockfile, reports multiple lockfile managers as ambiguous, and reports missing selected lockfiles on conflicts.
- Stopped treating `bun.lockb` as supported evidence and removed it from topology/cache lifecycle inputs.
- Added `npm-shrinkwrap.json` to topology input digests and selected it ahead of `package-lock.json` for npm.
- Kept Go package topology unchanged on `go.mod` and `go.sum`.

## Tests

- `cargo test -p polint js_lockfile --lib`
- `cargo test -p polint module_graph::ts::dependency_topology --lib`
- `cargo test -p polint module_graph::ts::topology --lib`
- `cargo test -p polint module_graph_layer_key_topology_inputs_change_on_manifest_lock_workspace_and_tsconfig --lib`
- `cargo test -p polint ts_js_lifecycle_records_manifests_config_resolver_options_and_sources --lib`
- `cargo test -p polint package_lock --lib`
- `cargo test -p polint module_graph::ts --lib`
- `cargo clippy -p polint --lib --tests -- -D warnings`
- `cargo test -p polint --lib`
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
