---
phase: 55-sdk-query-vocabulary-and-preview-contract
plan: 02
subsystem: capability-planning
tags: [sdk, capabilities, manifests, facts-list]
key-files:
  created: []
  modified:
    - crates/polint-macros/src/lib.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/cli/mod.rs
requirements-completed: [API-01, API-05, API-06]
duration: 24 min
completed: 2026-06-20
---

# Phase 55 Plan 02: Capability Derivation Manifest And Fail-Closed Support Summary

Preview policy views now derive distinct capability names and report honest preview/unsupported status through analysis plans and `polint facts list`.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 7f09143a | Added `events`, `calls`, `control_flow`, and preview `dataflow` capability wiring, support rows, macro mapping, and CLI facts metadata. |

## Verification

- `cargo fmt --all --check` PASS
- `cargo test -p polint-macros capability_for_type_maps_supported_fact_views --locked` PASS
- `cargo test -p polint --lib preview_policy_capabilities_remain_fail_closed --locked` PASS
- `cargo test -p polint --lib policy_preview_capabilities_have_distinct_names --locked` PASS
- `cargo test -p polint --lib facts_list_reports_phase55_preview_capabilities --locked` PASS
- `cargo check -p polint --locked` PASS
- `cargo run -p polint --locked -- facts list --format json` PASS; JSON includes `calls`, `control_flow`, `dataflow`, and `events` as `preview`, while `cfg` and `call_graph` remain `reserved`.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` PASS

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- GSD commit wrapper again returned a truncated hook failure while clippy was still checking. Running clippy directly passed, and retrying the GSD commit succeeded.

## Next Phase Readiness

Plan 55-03 can add external temp-repo proof, public-surface leak-gate updates, and docs. The implementation already exposes the preview capability rows that docs/tests need to assert against.

## Self-Check: PASSED

