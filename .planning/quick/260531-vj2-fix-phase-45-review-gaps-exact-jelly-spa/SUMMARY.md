---
quick_id: 260531-vj2
slug: fix-phase-45-review-gaps-exact-jelly-spa
status: complete
completed: 2026-05-31
---

# Summary: Fix Phase 45 Review Gaps

## Completed

- Added exact Jelly span oracle coverage for the TS inventory spans fixture.
- Strengthened TS direct-binding semantic graph fixture assertions so each
  claimed binding form is checked for direct-binding rows and graph constraints.
- Added a semantic graph digest regression that mutates the real TS path alias
  fixture and proves the provider digest changes.
- Fixed uncovered implementation gaps exposed by the stronger fixture:
  CommonJS exported function assignments are indexed as TS functions, and simple
  object-literal destructuring aliases can resolve as local direct bindings.

## Verification

- `cargo test -p polint ts_inventory_spans -- --nocapture`
- `cargo test -p polint ts_direct_bindings_fixture -- --nocapture`
- `cargo test -p polint semantic_graph_digest_changes_when_ts_path_alias_fixture_changes -- --nocapture`
- `cargo test -p polint extracts_commonjs_exported_function_expression_assignments -- --nocapture`
- `cargo test -p polint`
- `cargo fmt --all --check`
- `git diff --check`
- `cargo clippy -p polint --all-targets -- -D warnings`
