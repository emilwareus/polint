---
phase: 41-public-sdk-query-views-and-agent-ergonomics
plan: 02
subsystem: public-sdk-query-helpers
tags: [sdk, facts, rules]
key-files:
  created: []
  modified:
    - crates/polint/src/sdk/facts.rs
    - crates/polint/tests/cli.rs
    - docs/facts/resolved-imports.md
    - docs/facts/symbols-and-references.md
    - docs/facts/metrics.md
requirements-completed: [SAE-PROM-02]
duration: 0 min
completed: 2026-05-26
---

# Phase 41 Plan 02: Supported SDK View Query Ergonomics Summary

Added borrowed, deterministic helper methods to existing stable SDK views for relationship, symbol/reference, and metric policies.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 3d1334e | Relationship helpers, symbol/reference lookup helpers, metric threshold helpers, docs, and external rule tests. |

## Verification

- `cargo test -p polint --lib sdk::facts::tests --locked` PASS
- `cargo test -p polint --test cli phase41_relationship_query_helpers_external_rule --locked` PASS
- `cargo test -p polint --test cli phase41_symbol_reference_query_helpers_external_rule --locked` PASS
- `cargo test -p polint --test cli phase41_metric_query_helpers_external_rule --locked` PASS

## Deviations from Plan

No broad graph, call, data-flow, or evidence SDK surface was promoted.

## Self-Check: PASSED
