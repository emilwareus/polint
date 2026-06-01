---
quick_id: 260531-vj2
slug: fix-phase-45-review-gaps-exact-jelly-spa
status: complete
created: 2026-05-31
---

# Fix Phase 45 Review Gaps

Task: Fix Phase 45 review gaps from the critical review:

- Add exact Jelly span oracle data and compare rendered inventory spans against it.
- Strengthen the TS direct-binding semantic graph fixture so each claimed binding form is asserted.
- Add an end-to-end cache/digest regression that mutates real fixture inputs and proves semantic graph digest changes.

Verification:

- `cargo test -p polint ts_inventory_spans`
- `cargo test -p polint ts_direct_bindings_fixture`
- `cargo test -p polint semantic_graph_digest_changes_when_ts_path_alias_fixture_changes`
- `cargo test -p polint`
- `cargo clippy -p polint --all-targets -- -D warnings`
