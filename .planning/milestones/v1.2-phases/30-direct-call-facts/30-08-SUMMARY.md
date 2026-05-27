---
phase: 30-direct-call-facts
plan: 08
subsystem: public-boundary
tags: [rust, cli-tests, sdk-boundary, direct-calls, capability-honesty]

requires:
  - phase: 30-direct-call-facts
    provides: "Plans 30-01 through 30-07 private direct-call facts, provider/debug/eval wiring, fixtures, and call indexes"
provides:
  - "Public no-leak integration proof for private direct-call internals across check, inspect, test, help, SDK, runner, CLI, README, and facts docs"
  - "External temp-repo compatibility proof using only supported SDK views while direct-call internals stay private"
  - "Unsupported call_graph capability proof for CallGraph<'_> with no fabricated rule execution"
affects: [phase-30-final-proof, phase-41-public-promotion, sdk-boundary, direct-calls]

tech-stack:
  added: []
  patterns: ["public-boundary source scans", "external temp-repo SDK fixture", "reserved capability regression"]

key-files:
  created:
    - .planning/phases/30-direct-call-facts/30-08-SUMMARY.md
  modified:
    - crates/polint/tests/cli.rs
    - crates/polint/src/analysis_plan.rs

key-decisions:
  - "Kept direct-call internals private and test-facing; no SDK, runner, CLI, README, or docs/facts call surface was promoted."
  - "Kept CallGraph<'_> as an inert reserved SDK view whose call_graph capability remains unsupported."
  - "Recorded the verification-only regression task as an empty test commit to preserve the per-task commit contract."

patterns-established:
  - "Public no-leak tests pair public JSON/output checks with source-surface scans and external-consumer fixture rules."
  - "Reserved capability tests assert both the capability diagnostic and absence of rule execution."

requirements-completed: [SAE-SEM-05]

duration: 10 min
completed: 2026-05-21
---

# Phase 30 Plan 08: Direct Call Public Boundary Summary

**Public-boundary proof that private direct-call facts do not leak and call_graph remains unsupported**

## Performance

- **Duration:** 10 min
- **Started:** 2026-05-21T09:40:19Z
- **Completed:** 2026-05-21T09:50:33Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `direct_calls_internals_stay_private`, covering `polint check --format json`, `polint inspect rule --format json`, `polint test --format json`, CLI help, SDK/runner/CLI source, README, and `docs/facts`.
- Added an external temp-repo rule that imports only `polint::sdk::prelude::*`, registers through `polint::runner::run_cli`, and requests supported `ResolvedImports<'_>`, `ModuleGraphFacts<'_>`, `Symbols<'_>`, and `References<'_>` views.
- Added `call_graph_capability_remains_unsupported` plus `analysis_plan::tests::reserved_capabilities_remain_unsupported` to prove `CallGraph<'_>` maps to unsupported `call_graph` and does not execute with fabricated call facts.

## Task Commits

1. **Task 1: Add public no-leak proof for private direct-call internals** - `75b2db3` (test)
2. **Task 2: Prove `call_graph` capability remains unsupported** - `58d42d4` (test)
3. **Task 3: Run public-boundary regression set and formatting** - `dab3b1d` (test, empty verification commit)

## Files Created/Modified

- `crates/polint/tests/cli.rs` - Direct-call public no-leak fixture and call_graph unsupported capability integration test.
- `crates/polint/src/analysis_plan.rs` - Focused unit assertion that `call_graph` remains a reserved unsupported capability with roadmap docs.
- `.planning/phases/30-direct-call-facts/30-08-SUMMARY.md` - Execution summary.

## Decisions Made

- No production direct-call or SDK behavior changed; the new tests prove the existing private/public boundary.
- `CallGraph<'_>` remains present only as a reserved SDK fact-view type with no query methods and no supported capability.
- The broad planned `rg` check for CallGraph methods also matches existing `ModuleGraphFacts::outgoing` and `incoming`; the actual CallGraph block was checked directly and left inert.

## Deviations from Plan

None - plan executed as written. The new TDD-tagged tests passed against the existing implementation, so no GREEN production code was needed.

## Issues Encountered

- The literal acceptance `rg` for `outgoing(` and `incoming(` matches existing public `ModuleGraphFacts` methods unrelated to `CallGraph<'_>`. I did not change that public API; a scoped scan of the `CallGraph` block confirmed it has no public query methods.
- Targeted CLI tests still print pre-existing dead-code warnings for crate-private call-store accessors. They did not fail verification and were not caused by this plan's changes.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --test cli direct_calls_internals_stay_private --locked`
- `cargo test -p polint --test cli call_graph_capability_remains_unsupported --locked`
- `cargo test -p polint --lib analysis_plan::tests::reserved_capabilities_remain_unsupported --locked`
- `cargo test -p polint --test cli semantic_mir_internals_stay_private --locked`
- `cargo test -p polint --test cli module_topology_internals_stay_private --locked`
- `cargo test -p polint --test cli cfg_public_no_leak --locked`
- `cargo test -p polint --test cli cfg_capability_remains_unsupported --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None. Stub scan hits were existing test fixture literals (`TODO`, `exclude = []`) and fixture source strings, not unwired product stubs.

## Threat Flags

None. This plan adds test coverage and a unit assertion only; it introduces no new network endpoints, auth paths, file-access contracts, schemas, or public fact surfaces.

## Next Phase Readiness

Phase 30 public-boundary proof is complete. Direct-call facts remain private, and later promotion work can start from a protected unsupported `call_graph` contract.

## Self-Check: PASSED

- Verified summary and modified key files exist on disk.
- Verified task commits `75b2db3`, `58d42d4`, and `dab3b1d` exist in git history.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*
