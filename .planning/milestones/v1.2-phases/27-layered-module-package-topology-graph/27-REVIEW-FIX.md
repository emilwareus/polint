# Phase 27 Review Fixes

## Status

All warnings from `27-REVIEW.md` were fixed.

## Fixes

- `WR-01`: Go import-to-package derivation now matches Go package nodes by import-path label when topology package rows do not carry `module_node`, and Go external imports use full module-path prefix matching against `go.mod` requirements. Requirements attached to the Go module package apply to local Go package source sets under the same workspace root.
- `WR-02`: `package-lock.json` parsing now treats malformed JSON, v1 lockfiles, and unknown lockfile versions as unsupported lockfile evidence. TS topology emits `Unsupported` resolved-dependency rows for that evidence instead of silently suppressing missing-lockfile diagnostics with no row.
- `WR-03`: `derive_module_topology_with_cache_stats` now clears stale import-to-package facts before returning the empty-imports cache digest.

## Verification

- `cargo test -p polint module_graph::import_to_package --locked`
- `cargo test -p polint module_graph::ts::dependency_topology --locked`
- `cargo test -p polint module_graph::formats::package_lock --locked`
- `cargo test -p polint module_graph::module_topology_layer_cache --locked`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
