---
phase: 25-rule-manifest-inspect-and-test-skeleton
plan: 04
subsystem: rule-authoring-cli
tags: [rust, cli, docs, schemas, generated-rules]

requires:
  - phase: 25-rule-manifest-inspect-and-test-skeleton
    plan: 02
    provides: public rule inspect JSON
  - phase: 25-rule-manifest-inspect-and-test-skeleton
    plan: 03
    provides: public fixture test runner
provides:
  - `new-rule` fixture skeletons
  - generated-rule inspect/test/check integration proof
  - consumer docs for inspect/test authoring loop
  - final full-workspace verification for Phase 25
affects: [phase-25, new-rule, docs, cli, schemas]

tech-stack:
  added: []
  patterns:
    - generated fixture skeletons are passing empty-expect tests until authors add diagnostics
    - generated rules stay macro/prelude/run_cli based
    - docs name promoted inspect/test surfaces and explicitly exclude deferred internals

key-files:
  modified:
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs
    - docs/CONSUMER-SETUP.md

requirements-completed: [SAE-FND-06]

completed: 2026-05-18
---

# Phase 25 Plan 04: Generated Fixture and Docs Summary

Closed Phase 25 by wiring `new-rule` into the inspect/test authoring loop and documenting the promoted public surfaces.

## Accomplishments

- `polint new-rule` now creates `.polint/tests/rules/<module>/basic/` with `polint-test.toml` and a starter source fixture.
- The generated fixture is intentionally an empty-expect passing skeleton with commented `[[expect.diagnostic]]` fields, so `polint test` does not fail before the rule is implemented.
- Added an end-to-end generated-rule test covering `polint init`, `new-rule`, dependency rewrite, `inspect rule`, `test`, and `check`.
- Added schema-validity coverage for inspect/test schemas.
- Updated `docs/CONSUMER-SETUP.md` with inspect/test commands, fixture layout, expected diagnostic fields, schema links, and explicit unsupported surfaces.

## Verification

- `cargo test -p polint --test cli inspect_rule_manifest_json_is_stable_for_local_rules --locked`
- `cargo test -p polint --test cli polint_test_runs_temp_repo_fixtures --locked`
- `cargo test -p polint --test cli new_rule_generates_fixture_that_inspect_and_test_can_run --locked`
- `cargo test -p polint --test cli top_level_help_only_lists_supported_public_commands --locked`
- `cargo test -p polint --test cli inspect_and_test_schema_files_are_valid_json --locked`
- `cargo test --workspace --all-features --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`
- `jq empty docs/schemas/polint-rule-inspect-v1.json && jq empty docs/schemas/polint-test-report-v1.json`

## Deviations

- No todo fixture mode was added. Generated fixtures use an empty `[expect]` table plus commented expected-diagnostic fields to keep the default scaffold green.

## Result

Phase 25 now provides a practical repo-local rule authoring loop: generate a macro rule, inspect its manifest, run fixture tests, and run the rule through the normal check path.
