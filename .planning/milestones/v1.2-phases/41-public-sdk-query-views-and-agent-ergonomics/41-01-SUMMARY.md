---
phase: 41-public-sdk-query-views-and-agent-ergonomics
plan: 01
subsystem: public-sdk-promotion
tags: [sdk, capabilities, docs, no-leak]
key-files:
  created: []
  modified:
    - docs/API-VISIBILITY-PLAN.md
    - docs/facts/README.md
    - crates/polint/src/analysis_plan.rs
    - crates/polint/tests/cli.rs
requirements-completed: [SAE-PROM-02]
duration: 0 min
completed: 2026-05-26
---

# Phase 41 Plan 01: Promotion Audit And Reserved Capability Disposition Summary

Added an explicit Phase 41 promotion audit and aligned reserved capability diagnostics with current public docs paths.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 3d1334e | Promotion audit, reserved capability docs paths, and Phase 41 public no-leak baseline. |

## Verification

- `cargo test -p polint --lib analysis_plan::tests::reserved_capabilities_remain_unsupported --locked` PASS
- `cargo test -p polint-macros --locked` PASS
- `cargo test -p polint --test cli phase41_public_promotion_baseline_no_leak --locked` PASS

## Deviations from Plan

Tasks were implemented in one production commit with the rest of Phase 41 to keep cross-cutting CLI/docs/test changes coherent.

## Self-Check: PASSED
