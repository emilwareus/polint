---
phase: 38-local-plus-summary-projected-data-flow
plan: 05
subsystem: analysis
tags: [rust, data-flow, sources, sinks, extensions]
requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: data-flow provider and models
provides:
  - Source models from trust boundaries and data-flow models from accepted extension facts
affects: [data-flow, entrypoints, extensions]
tech-stack:
  added: []
  patterns: [native trust-boundary source models, extension model facts]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis/data_flow/provider.rs
    - docs/facts/data-flow.md
key-decisions:
  - "Accepted extension facts are converted only when their fact family declares a supported data-flow model kind."
patterns-established:
  - "Model facts carry provider id, model id, precision, confidence, evidence, and payload labels."
requirements-completed: [SAE-PREC-03]
duration: 10min
completed: 2026-05-25
---

# Phase 38 Plan 05 Summary

**Trust-boundary source models and extension-provided data-flow models**

## Accomplishments
- Added source models and source nodes for trust-boundary facts.
- Added extension model ingestion for source, sink, sanitizer, barrier, and TITO fact families.
- Documented heuristic limits and model behavior in `docs/facts/data-flow.md`.

## Task Commits
1. **Source and extension model data-flow facts** - `bf41e6c` (feat)

## Verification
- `cargo check -p polint`
- `cargo test -p polint data_flow --lib`

## Deviations from Plan
No public SDK view or rule capability was exposed; this preserves the private-boundary requirement while models mature.

## Issues Encountered
None.
