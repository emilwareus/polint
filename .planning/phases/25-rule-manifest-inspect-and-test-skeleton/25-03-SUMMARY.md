---
phase: 25-rule-manifest-inspect-and-test-skeleton
plan: 03
subsystem: rule-authoring-cli
tags: [rust, cli, fixtures, rule-testing]

requires:
  - phase: 25-rule-manifest-inspect-and-test-skeleton
    plan: 01
    provides: macro-derived rule metadata and rule host bridge
provides:
  - public `polint test --format json`
  - fixture discovery under `.polint/tests/rules/*/*/polint-test.toml`
  - normalized expected/observed diagnostic matching
  - rule test JSON schema
affects: [phase-25, cli, rule-testing, schemas]

tech-stack:
  added: []
  patterns:
    - temp-repo fixture execution through real local rule-host `check --format json`
    - normalized diagnostic assertions for public JSON reports
    - test report JSON omits temp roots, cache paths, durations, and cargo target paths

key-files:
  created:
    - crates/polint/src/rule_test.rs
    - docs/schemas/polint-test-report-v1.json
  modified:
    - crates/polint/src/lib.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs

requirements-completed: [SAE-FND-06]

completed: 2026-05-18
---

# Phase 25 Plan 03: Rule Test Runner Summary

Added the first supported `polint test` fixture runner for repo-local rules.

## Accomplishments

- Added crate-private `rule_test` with fixture discovery, manifest parsing, temp-repo materialization, rule-host check execution, diagnostic normalization, and deterministic pass/fail reports.
- Added public `polint test` with `--format <human|json>`, `--rule`, `--case`, `--no-cache`, and `--keep-temp`.
- Added `docs/schemas/polint-test-report-v1.json` for the public test report contract.
- Added integration coverage for an external-style macro rule using only `polint::sdk::prelude::*` and `polint::runner::run_cli`.
- Proved both passing and failing fixture reports through `polint test --format json`.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p polint --lib rule_test --locked`
- `cargo test -p polint --test cli polint_test_runs_temp_repo_fixtures --locked`
- `cargo test -p polint --test cli top_level_help_only_lists_supported_public_commands --locked`
- Acceptance greps verified the rule test module, schema fields, public command wiring, real JSON check execution, and no `cargo test` implementation path.

## Deviations

- The initial runner treats unexpected diagnostics as failures. That gives rule authors a tighter red/green loop than only checking that expected diagnostics are present.

## Next

Plan 25-04 can update scaffolding/docs so newly generated rules include a starter fixture and point authors at `inspect` and `test`.
