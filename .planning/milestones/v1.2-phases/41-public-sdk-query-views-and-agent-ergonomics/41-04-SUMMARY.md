---
phase: 41-public-sdk-query-views-and-agent-ergonomics
plan: 04
subsystem: agent-inspection-cli
tags: [facts, unknowns, explain, json]
key-files:
  created:
    - docs/schemas/polint-facts-v1.json
    - docs/schemas/polint-unknowns-v1.json
    - docs/schemas/polint-explain-v1.json
  modified:
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs
    - docs/facts/README.md
requirements-completed: [SAE-PROM-02]
duration: 0 min
completed: 2026-05-26
---

# Phase 41 Plan 04: Bounded Facts Unknowns And Explain Commands Summary

Added bounded versioned JSON commands for `polint facts`, `polint unknowns`, and `polint explain` without exposing private provider/debug/eval schemas.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 3d1334e | Public fact listing/sampling, unknown reporting, explain JSON, schemas, and deterministic contract tests. |

## Verification

- `cargo test -p polint --test cli facts_list_json_is_stable_and_public_only --locked` PASS
- `cargo test -p polint --test cli facts_sample_requires_or_applies_bounded_limit --locked` PASS
- `cargo test -p polint --test cli unknowns_json_reports_public_setup_and_resolution_gaps --locked` PASS
- `cargo test -p polint --test cli explain_json_reports_rule_capability_plan --locked` PASS
- `cargo run -q -p polint -- facts list --format json` PASS
- `cargo run -q -p polint-cli -- facts list --format json` NOT RUN: package `polint-cli` does not exist in this workspace.

## Deviations from Plan

Used the existing `polint` package for the facts-list smoke command because there is no `polint-cli` package.

## Self-Check: PASSED
