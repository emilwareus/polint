---
phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
plan: 03
subsystem: analysis
tags: [rust, unknowns, taxonomy, cli, solver, go-semantic]

requires:
  - phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
    provides: refined-call rows and solver-derived edge facts
provides:
  - private analysis::unknown_taxonomy row model
  - collectors for public capability gaps and graph-engine unknown states
  - compatibility bridge for existing polint unknowns JSON
affects: [inspect-unknowns, eval-delta, public-json]

tech-stack:
  added: []
  patterns: [private taxonomy normalization, compatibility rendering through existing CLI rows]

key-files:
  created:
    - crates/polint/src/analysis/unknown_taxonomy/mod.rs
    - crates/polint/src/analysis/unknown_taxonomy/facts.rs
    - crates/polint/src/analysis/unknown_taxonomy/collect.rs
  modified:
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/cli/mod.rs

key-decisions:
  - "Unknown taxonomy stays crate-private under analysis::unknown_taxonomy."
  - "Existing polint unknowns --cap JSON remains shape-compatible by converting taxonomy rows back to UnknownsRow."
  - "The consolidated all_unknowns collector is staged for Plan 52-04 inspect unknowns wiring."

patterns-established:
  - "Unknown rows carry category, provider, family, capability, docs path, suggested artifact, source stable key, and stable sort key."
  - "Collectors are read-only normalization and do not suppress provider facts or diagnostics."

requirements-completed: [TAX-01]

duration: 50min
completed: 2026-06-05T09:43:00Z
---

# Phase 52 Plan 03 Summary

**Private unknown taxonomy normalizes public setup gaps and graph-engine unknown states**

## Performance

- **Duration:** 50 min
- **Started:** 2026-06-05T08:53:00Z
- **Completed:** 2026-06-05T09:43:00Z
- **Tasks:** 4
- **Files modified:** 5

## Accomplishments

- Added `analysis::unknown_taxonomy` with stable categories, row/span model, deterministic sort keys, and normalization.
- Added collectors for `resolved_imports`, `symbols`, `references`, Go semantic package errors, solver non-present edges, refined-call unknowns, and rejected adaptation models.
- Routed existing `polint unknowns --cap ...` through taxonomy while preserving the current public JSON fields.
- Added tests for category labels, deterministic row order, public import gaps, Go sidecar categories, solver budget rows, refined-call unknowns, and model-missing rows.

## Task Commits

1. **Add private unknown taxonomy** - `6103a353` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/unknown_taxonomy/facts.rs` - Private categories, span, row model, and deterministic normalization.
- `crates/polint/src/analysis/unknown_taxonomy/collect.rs` - Collectors for public and graph-engine unknown sources.
- `crates/polint/src/analysis/unknown_taxonomy/mod.rs` - Module entrypoint.
- `crates/polint/src/analysis/mod.rs` - Registers the private taxonomy module.
- `crates/polint/src/cli/mod.rs` - Keeps existing `polint unknowns` JSON compatible through taxonomy conversion.

## Decisions Made

The taxonomy collector now owns normalization, but public CLI rendering remains in `cli/mod.rs` until Plan 52-04 adds the new `inspect unknowns` surface. This avoids public schema churn before the canonical command lands.

## Deviations from Plan

`render.rs` was not created in this slice. Rendering remains in the existing CLI compatibility row type; Plan 52-04 is the correct place to add the canonical renderer/schema changes.

## Issues Encountered

Moving public unknown row construction into taxonomy made two CLI helper functions obsolete. They were removed after tests surfaced dead-code warnings.

## Verification

- `cargo test -p polint --lib unknown_taxonomy`
- `cargo test -p polint --test cli unknowns`
- `cargo check -p polint`
- Pre-commit hook: `cargo fmt --all -- --check`
- Pre-commit hook: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`

## User Setup Required

None.

## Next Phase Readiness

Plan 52-02 can verify downstream refined-call consumers still use `RefinedCallEdgeFact`; Plan 52-04 can wire `polint inspect unknowns --format json` to the staged `all_unknowns` collector.

---
*Phase: 52-refined-calls-rework-unknown-taxonomy-consolidation*
*Completed: 2026-06-05*
