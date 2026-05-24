---
status: complete
created: 2026-05-24
completed: 2026-05-24
workflow: gsd-quick
---

# Fix Phase 36 Review Findings Summary

## Changes

- Restored Go MIR traversal inside composite and function literals while preserving literal value rows.
- Changed access-path call-return projections to carry `CallSiteId` rather than crossing into the `PlaceId` domain.
- Added `ValueKind::PlaceRef` for ordinary place copies and kept its precision inside the type/value alias provider ceiling.
- Remapped allocation-token references during value output normalization, including value subjects and object/array/composite value kinds.
- Preserved file/body/function/operation/place provenance for unsupported Go type facts.
- Updated provider-order eval expectations to include `polint.type_value_alias`.
- Added regression tests for every reviewed failure mode.

## Validation

- `cargo fmt --all --check`
- `cargo test -p polint --lib analysis::values::store --locked`
- `cargo test -p polint --lib analysis::types::go --locked`
- `cargo test -p polint --lib analysis::mir::lower_go --locked`
- `cargo test -p polint --lib provider_outputs_are_constructed_in_manifest_order --locked`
- `cargo test -p polint --lib eval_observed_kernel_collects_provider_order_invariants_from_real_kernel --locked`
- `cargo test -p polint --lib eval_native_fixture_runner_provider_order_fixture_passes --locked`
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked`
- `cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
