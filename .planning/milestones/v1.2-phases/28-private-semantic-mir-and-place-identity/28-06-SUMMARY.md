---
phase: 28-private-semantic-mir-and-place-identity
plan: 06
subsystem: static-analysis-engine
tags: [rust, eval, semantic-mir, places, fixtures]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: private semantic MIR provider, debug JSON, validation, and Go/TS lowering from plans 28-01 through 28-05
  - phase: 22-internal-evaluation-harness-mvp
    provides: crate-private eval fixture model, matcher, metrics, and native fixture runner
provides:
  - semantic MIR eval fact families for MirBody, MirOperation, Place, and UnsupportedSemantic
  - debug JSON observation of MIR bodies, operations, places, and unsupported rows
  - semantic-MIR native fixture covering Go and TS/JS lowering snapshots
  - cold/warm deterministic semantic-MIR fixture proof
affects: [phase-28, phase-29-cfg, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [test-only MIR debug observation, compact eval payload fragments, native fixture determinism]

key-files:
  created:
    - tests/eval-fixtures/semantic-mir/core/expected.polint-eval.toml
    - tests/eval-fixtures/semantic-mir/core/repo/.polint.toml
    - tests/eval-fixtures/semantic-mir/core/repo/go.mod
    - tests/eval-fixtures/semantic-mir/core/repo/service.go
    - tests/eval-fixtures/semantic-mir/core/repo/web/package.json
    - tests/eval-fixtures/semantic-mir/core/repo/web/src/app.ts
  modified:
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/mod.rs

key-decisions:
  - "Keep semantic-MIR eval observation crate-private and test-facing, sourced only from metadata_debug_json_for_test."
  - "Use compact semicolon payload fragments for MIR eval evidence instead of raw source, AST dumps, absolute paths, or dense IDs as identity."
  - "Treat Partial semantic-MIR rows as unknown-like evidence in matcher outcomes and metrics."

patterns-established:
  - "Semantic MIR fixture runners compare cold and warm normalized eval output before adding a determinism invariant."
  - "Native MIR fixture expected rows assert MIR/place/unsupported families only, deferring CFG, direct call targets, domains, and SDK views."

requirements-completed: [SAE-SEM-03]

duration: 12 min
completed: 2026-05-20
---

# Phase 28 Plan 06: Semantic MIR Eval Fixture Summary

**Internal eval snapshots for Go and TS/JS semantic MIR bodies, operations, places, and unsupported semantics**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-20T08:55:41Z
- **Completed:** 2026-05-20T09:07:41Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments

- Added `SEMANTIC_MIR_FACT_FAMILIES`, `FixtureArea::SemanticMir`, and `ObservedStatus::Partial`.
- Extended eval observation to read `metadata_debug_json_for_test()["mir"]` and emit `MirBody`, `MirOperation`, `Place`, and `UnsupportedSemantic` fact rows.
- Counted `Partial`, `Unknown`, and `Unsupported` MIR rows as unknown-like evidence in matcher/metrics behavior.
- Added `tests/eval-fixtures/semantic-mir/core` with Go and TS/JS code that exercises parameters, locals, globals, temporaries, call returns, unknown roots, field/property/index projections, branches, returns, calls, `defer`, and `eval`.
- Added `run_semantic_mir_core_fixture_for_test` with cold/warm deterministic output comparison.

## Task Commits

1. **Task 1 RED:** `9b7c650` test - failing semantic MIR eval row tests.
2. **Task 1 GREEN:** `ed253fb` feat - MIR debug observation, `Partial` status, and unknown-like scoring.
3. **Task 2 RED:** `647ace7` test - failing semantic MIR native fixture tests.
4. **Task 2 GREEN:** `80317e1` feat - semantic-MIR native fixture and deterministic runner.

## Files Created/Modified

- `crates/polint/src/eval/model.rs` - Adds semantic-MIR area/families and `ObservedStatus::Partial`.
- `crates/polint/src/eval/observed.rs` - Normalizes semantic MIR debug JSON rows into compact observed fact rows.
- `crates/polint/src/eval/matcher.rs` - Treats `Partial` rows as unknown-like matches.
- `crates/polint/src/eval/metrics.rs` - Classifies `Partial` fact statuses with unknown-like semantic/topology rows.
- `crates/polint/src/eval/report.rs` - Adds deterministic report sort key support for `Partial`.
- `crates/polint/src/eval/fixtures.rs` - Adds the semantic-MIR fixture runner and fixture tests.
- `crates/polint/src/eval/mod.rs` - Adds focused semantic-MIR row model/observation tests.
- `tests/eval-fixtures/semantic-mir/core/expected.polint-eval.toml` - Expected MIR/place/unsupported rows and determinism invariant.
- `tests/eval-fixtures/semantic-mir/core/repo/*` - Native Go and TS/JS fixture repo.

## Decisions Made

- Used existing test-only metadata debug JSON instead of adding any public eval, SDK, CLI, or runner surface.
- Kept payloads compact: `path`, line/column span, owner, operation kind, root, projections, construct, and conservative action.
- Expected fixture rows intentionally assert only MIR/place/unsupported facts, not CFG edges, dominance, direct call targets, abstract-domain states, summaries, or public SDK views.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed duplicate semantic-MIR runtime-budget observations**
- **Found during:** Task 2 fixture verification
- **Issue:** The warm observed rows already included a runtime-budget observation, and the semantic-MIR runner added another elapsed budget row, producing duplicate runtime-budget observations.
- **Fix:** Filtered runtime-budget rows from warm observations before adding the fixture-level elapsed budget row.
- **Files modified:** `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --lib eval::fixtures::semantic_mir_core --locked` passed.
- **Committed in:** `80317e1`

---

**Total deviations:** 1 auto-fixed (1 Rule 1)
**Impact on plan:** The fix kept fixture output deterministic and avoided duplicate runtime evidence without changing the public surface.

## Issues Encountered

- The plan's no-leak grep pattern also matches pre-existing non-MIR snapshot helper names in `eval/observed.rs` (`source_text_digest` and absolute-path normalization tests). The semantic-MIR observation code itself uses compact fragments and does not add raw source, AST, or absolute path payloads.
- Initial expected fixture rows assumed global places would be `unknown`; the actual lowering correctly emits global roots as `partial` heuristic evidence, while TS `window` access supplies the unknown-root row.

## Verification

- `cargo test -p polint --lib eval::semantic_mir_rows --locked` passed.
- `cargo test -p polint --lib eval::fixtures::semantic_mir_core --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance greps for semantic-MIR families, fixture runner, expected fixture file, and expected fixture taxonomy passed.

## Known Stubs

None. Stub scan only matched intentional fixture `exclude = []` and format strings.

## Threat Flags

None. The planned debug JSON to eval-row trust boundary was handled with compact relative payload fragments and no new network, auth, file-access, or schema boundary surface.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 28 now has deterministic internal eval proof for semantic MIR/place rows across Go and TS/JS. Phase 29 can consume the private MIR/place substrate for CFG/control-dependence work without public API promotion.

## Self-Check: PASSED

- Verified summary and semantic-MIR fixture files exist.
- Verified task commits `9b7c650`, `ed253fb`, `647ace7`, and `80317e1` exist in git history.
- Verified stub scan found no plan-blocking stubs in modified files.

---
*Phase: 28-private-semantic-mir-and-place-identity*
*Completed: 2026-05-20*
