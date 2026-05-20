# Quick Task 260520-a6t Summary

**Date:** 2026-05-20
**Status:** Complete

## Changes

- Added root `pnpm-workspace.yaml` membership to TS/JS package manifest collection so pnpm workspaces work without `package.json#workspaces`.
- Made workspace-root `packageManager` authoritative for members before considering member-local stale lockfiles.
- Added unsupported topology evidence when a selected JS lockfile has package requirements but produces no parseable exact selected entries.

## Tests

- `cargo fmt --check`
- `cargo test -p polint --lib module_graph::ts::dependency_topology`
- `cargo test -p polint --lib module_graph::formats::js_lockfile::tests`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
