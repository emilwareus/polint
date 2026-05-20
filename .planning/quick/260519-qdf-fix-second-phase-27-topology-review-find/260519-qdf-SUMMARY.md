# Quick Task 260519-qdf: Fix Second Phase 27 Topology Review Findings - Summary

**Date:** 2026-05-19
**Code Commit:** cbb635e
**Status:** Complete

## Changes

- Added workspace-member manifest discovery to module graph topology cache inputs, including source-less TS/JS workspace packages.
- Preserved semantic uncertainty for duplicate import rows by deriving `Dynamic`, `Ambiguous`, `SetupMissing`, or `Unsupported` before resolver fallback when duplicate semantic rows require it.
- Changed generic package-lock repository overlays to `ExactStatic`; lockfile-selected dependency rows still use `ExactLockfile`.
- Marked unsupported or malformed `package.json` manifests as unsupported package topology and emitted explicit unsupported overlays.
- Parsed both `bundleDependencies` and `bundledDependencies` as bundled npm dependency declarations.

## Verification

- `cargo test -p polint --locked module_graph_layer_key_topology_inputs_follow_workspace_member_manifests`
- `cargo test -p polint --locked module_graph_layer_cache_invalidates_when_source_less_workspace_member_manifest_changes`
- `cargo test -p polint --locked mixed_duplicate_semantic_import_paths_are_ambiguous_not_exact`
- `cargo test -p polint --locked duplicate_dynamic_import_paths_remain_dynamic_without_unique_semantic_link`
- `cargo test -p polint --locked collect_ts_topology_records_package_manager_and_lockfile_evidence`
- `cargo test -p polint --locked collect_ts_topology_marks_malformed_package_json_unsupported`
- `cargo test -p polint --locked parse_package_json_reads_package_workspace_exports_imports_and_dependency_sections`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `git diff --check`

## Deep Review

Second review found no remaining blocking findings in the changed feature area. The remaining constraints are intentional: workspace glob expansion still matches the existing `packages/*`-style collector behavior, and richer package/workspace topology remains crate-private.
