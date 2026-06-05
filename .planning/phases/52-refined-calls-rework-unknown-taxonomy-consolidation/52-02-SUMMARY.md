---
phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
plan: 02
subsystem: analysis
tags: [rust, data-flow, evidence, public-api, verification]

requires:
  - phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
    provides: solver-projected refined-call rows
provides:
  - downstream compatibility verification for data-flow and evidence
  - public no-leak verification for refined-call/solver internals
  - CLI compatibility verification after taxonomy routing
affects: [data-flow, evidence, cli, public-api]

tech-stack:
  added: []
  patterns: [verification-only phase summary]

key-files:
  created: []
  modified: []

key-decisions:
  - "No downstream code changes were needed; data-flow already consumes RefinedCallEdgeFact through db.refined_call_edges()."
  - "Public surfaces remained clean without expanding public allowlists."

patterns-established:
  - "Use focused downstream tests plus full CLI/public-surface sweeps to validate compatibility plans."

requirements-completed: [GRAPH-05]

duration: 35min
completed: 2026-06-05T10:25:00Z
---

# Phase 52 Plan 02 Summary

**Data-flow, evidence, CLI, and public-surface compatibility verified for solver-projected refined calls**

## Performance

- **Duration:** 35 min
- **Started:** 2026-06-05T09:50:00Z
- **Completed:** 2026-06-05T10:25:00Z
- **Tasks:** 4
- **Files modified:** 0

## Accomplishments

- Verified `analysis::data_flow::direct_calls` consumes refined calls through `db.refined_call_edges()` and has no direct solver import.
- Verified evidence tests remain deterministic and continue to operate through data-flow/evidence rows.
- Verified public-surface leak tests pass without exposing solver, refined-call, semantic-graph, or taxonomy internals.
- Verified the full CLI suite, including `unknowns_json_reports_public_setup_and_resolution_gaps`, passes after taxonomy routing.

## Task Commits

No code commits. This plan completed as a verification-only compatibility sweep.

## Files Created/Modified

None for product code. This summary is the only artifact for the plan.

## Decisions Made

No changes were needed. Existing downstream architecture already used the intended compatibility boundary.

## Deviations from Plan

No fixture was added because the focused unit/integration tests already covered the required downstream consumers, and `eval::refined_calls` currently has no matching test target in the crate.

## Issues Encountered

The first combined Cargo command used invalid multiple test filters (`data_flow::direct_calls evidence`). It was rerun as separate valid filters.

## Verification

- `cargo test -p polint --lib data_flow::direct_calls`
- `cargo test -p polint --lib evidence`
- `cargo test -p polint --test public_surface_leak`
- `cargo test -p polint --test cli`
- `cargo test -p polint eval::refined_calls` (0 matching tests)

## User Setup Required

None.

## Next Phase Readiness

Plan 52-04 can safely add the canonical `polint inspect unknowns --format json` command on top of the private taxonomy collector.

---
*Phase: 52-refined-calls-rework-unknown-taxonomy-consolidation*
*Completed: 2026-06-05*
