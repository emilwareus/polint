---
status: complete
quick_id: 260526-uq2
slug: fix-phase-41-review-findings-for-generat
date: 2026-05-26
---

# Summary

Fixed Phase 41 review findings:

- Generated Go rules now report a diagnostic for Go error branches without nearby test evidence.
- Generated Go negative fixtures now include a real `err != nil` branch and are verified through `polint test`.
- `facts list` now advertises whether a capability supports `unknowns`.
- `polint unknowns` returns an unsupported row with exit code 2 for stable fact views that do not support unknown inspection, such as metrics and module graph.
- Resolved-import unknown JSON now uses stable snake_case labels for precision and unresolved reasons.
- `facts sample` sorts all candidate rows before truncating to the requested bounded limit.

Verification:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test -p polint --test cli facts_list_json_is_stable_and_public_only --locked -- --exact`
- `cargo test -p polint --test cli facts_sample_requires_or_applies_bounded_limit --locked -- --exact`
- `cargo test -p polint --test cli unknowns_json_reports_public_setup_and_resolution_gaps --locked -- --exact`
- `cargo test -p polint --test cli new_rule_go_generates_fixture_that_test_can_run --locked -- --exact`
- `cargo test -p polint --test cli --locked -- --test-threads=1`
- `cargo doc -p polint --all-features --no-deps --locked`

