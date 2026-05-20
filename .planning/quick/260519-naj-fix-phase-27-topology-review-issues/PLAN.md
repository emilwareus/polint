---
quick_id: 260519-naj
status: completed
created: 2026-05-19T14:46:15.830Z
---

# Fix Phase 27 Topology Review Issues

## Scope

Fix seven review findings in the Phase 27 topology implementation:

- Preserve topology references when merging Go and TS topology outputs.
- Attach root JS workspace packages/source sets to the root workspace.
- Resolve nested workspace globs relative to the owning package manifest.
- Attribute nested package-lock entries correctly and avoid duplicate lock stable keys.
- Scope Go go.sum evidence to the module package that produced it.
- Include extended TS config files in lifecycle/cache identity.
- Avoid collapsing multiple semantic imports with the same file/path.

## Verification Plan

- Add targeted unit tests for each finding.
- Run focused module-graph, TS, Go, package-lock, and lifecycle tests.
- Run full workspace tests, fmt check, and clippy with warnings denied.

## Result

- Fixed topology merge reference remapping before normalization.
- Fixed TS workspace ownership for root workspaces and nested workspace manifests.
- Fixed package-lock nested package attribution and stable key uniqueness.
- Fixed Go go.sum evidence scoping to the originating module package.
- Fixed TS config lifecycle/cache identity for local extended config chains.
- Fixed ambiguous semantic import lookup for duplicate syntax import paths.

## Verification

- `cargo test -p polint merge_offsets_colliding_ids_before_final_normalization --locked`
- `cargo test -p polint collect_ts_topology_emits_js_workspace_and_member_packages --locked`
- `cargo test -p polint collect_ts_topology_expands_nested_workspace_globs_relative_to_manifest --locked`
- `cargo test -p polint collect_ts_topology_keeps_nested_package_lock_entries_distinct --locked`
- `cargo test -p polint parse_package_lock_infers_nested_node_modules_package_name --locked`
- `cargo test -p polint dependency_topology_scopes_go_sum_edges_to_module_package --locked`
- `cargo test -p polint module_graph_layer_key_topology_inputs_follow_tsconfig_extends --locked`
- `cargo test -p polint ts_js_lifecycle_records_manifests_config_resolver_options_and_sources --locked`
- `cargo test -p polint duplicate_syntax_import_paths_do_not_share_one_semantic_row --locked`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
