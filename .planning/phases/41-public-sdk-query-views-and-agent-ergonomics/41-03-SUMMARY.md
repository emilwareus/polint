---
phase: 41-public-sdk-query-views-and-agent-ergonomics
plan: 03
subsystem: rule-authoring-loop
tags: [inspect, test, new-rule, schemas]
key-files:
  created: []
  modified:
    - crates/polint/src/cli/mod.rs
    - crates/polint/src/cli/skill.rs
    - crates/polint/tests/cli.rs
    - docs/CONSUMER-SETUP.md
requirements-completed: [SAE-PROM-02]
duration: 0 min
completed: 2026-05-26
---

# Phase 41 Plan 03: Inspect Test And New Rule Agent Contracts Summary

Hardened the existing inspect/test/new-rule loop and changed generated TypeScript fixtures so generated rules can pass positive and negative fixture cases immediately.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 3d1334e | Stable inspect/test schema checks, deterministic fixture contract tests, and agent-ready generated fixtures. |

## Verification

- `cargo test -p polint --test cli inspect_rule_json_matches_schema_v1 --locked` PASS
- `cargo test -p polint --test cli polint_test_json_matches_schema_v1 --locked` PASS
- `cargo test -p polint --test cli new_rule_generates_positive_and_negative_agent_fixtures --locked` PASS
- `cargo test -p polint --test cli new_rule_generates_fixture_that_inspect_and_test_can_run --locked` PASS

## Deviations from Plan

Existing schema files were kept compatible; no unrelated assertion language was added to `polint test`.

## Self-Check: PASSED
