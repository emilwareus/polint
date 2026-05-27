# Phase 41 Verification

Status: PASS
Date: 2026-05-26

## Goal

Promote bounded public SDK query views and agent ergonomics without leaking internal analysis surfaces, while keeping unsupported future capabilities honest and documented.

## Result

Phase 41 is implemented.

- Public SDK helpers were added for metrics, resolved imports, module graph relationships, symbols, and references.
- Public agent commands were added: `polint facts list`, `polint facts sample`, `polint unknowns`, and `polint explain`.
- JSON schema files were added for facts, unknowns, and explain output.
- `polint new-rule` now generates positive and negative fixture cases.
- Generated skill text, project skills, README, and fact docs were updated to describe the promoted surfaces and reserved capabilities.
- Public no-leak and temp-repo external-rule tests cover the promoted SDK and CLI surfaces.

## Verification Commands

Passed:

- `cargo check -p polint --locked`
- `cargo fmt --all --check`
- `cargo test -p polint --lib sdk::facts::tests --locked`
- `cargo test -p polint --lib analysis_plan::tests::reserved_capabilities_remain_unsupported --locked`
- `cargo test -p polint-macros --locked`
- `cargo test -p polint --test cli facts_list_json_is_stable_and_public_only --locked`
- `cargo test -p polint --test cli facts_sample_requires_or_applies_bounded_limit --locked`
- `cargo test -p polint --test cli unknowns_json_reports_public_setup_and_resolution_gaps --locked`
- `cargo test -p polint --test cli explain_json_reports_rule_capability_plan --locked`
- `cargo test -p polint --test cli new_rule_generates_fixture_that_inspect_and_test_can_run --locked`
- `cargo test -p polint --test cli phase41_public_promotion_baseline_no_leak --locked`
- `cargo test -p polint --test cli generated_skills_describe_phase41_public_surface --locked`
- `cargo test -p polint --test cli phase41_public_json_contracts_are_stable --locked`
- `cargo test -p polint --test cli phase41_metric_query_helpers_external_rule --locked`
- `cargo test -p polint --test cli phase41_relationship_query_helpers_external_rule --locked`
- `cargo test -p polint --test cli phase41_symbol_reference_query_helpers_external_rule --locked`
- `cargo test -p polint --test cli inspect_rule_json_matches_schema_v1 --locked`
- `cargo test -p polint --test cli polint_test_json_matches_schema_v1 --locked`
- `cargo test -p polint --test cli new_rule_generates_positive_and_negative_agent_fixtures --locked`
- `cargo test -p polint --test cli --locked`
- `cargo test -p polint --test cli --locked -- --test-threads=1`
- `cargo run -q -p polint -- facts list --format json`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo doc -p polint --all-features --no-deps --locked`

Attempted:

- `cargo test --workspace --all-targets --locked`
  - The library suite passed, then the CLI test binary failed after the disk filled with generated rule-host build outputs (`No space left on device`, linker `errno=28`).
  - Generated `target/polint-cli-test-*` artifacts were cleared.
  - The full CLI suite was rerun serially and passed: 139 passed, 0 failed.

Not run:

- `cargo run -q -p polint-cli -- facts list --format json`
  - There is no `polint-cli` package in this workspace. The equivalent package command, `cargo run -q -p polint -- facts list --format json`, passed.

## Review Gate

Code review passed. One reserved-capability sampling contract gap was found and fixed in `97b520c`.

