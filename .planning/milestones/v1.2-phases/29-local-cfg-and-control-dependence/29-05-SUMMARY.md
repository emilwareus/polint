---
phase: 29-local-cfg-and-control-dependence
plan: 05
subsystem: static-analysis-engine
tags: [rust, cfg, typescript, javascript, semantic-mir, control-flow]

requires:
  - phase: 29-local-cfg-and-control-dependence
    plan: 01
    provides: private CFG fact contracts and storage
  - phase: 29-local-cfg-and-control-dependence
    plan: 02
    provides: shared CFG builder and derived analyses
  - phase: 29-local-cfg-and-control-dependence
    plan: 03
    provides: private CFG provider slot, validation, cache identity, and debug output
  - phase: 29-local-cfg-and-control-dependence
    plan: 04
    provides: Go CFG lowering
provides:
  - TS/JS semantic-MIR-to-CFG lowering
  - provider merge path for Go and TS/JS CFG output
  - TS/JS CFG tests for branches, loops, returns, throws, short-circuiting, nullish, async/yield, cleanup, optional chaining, dynamic import, and unsupported rows
affects: [phase-29, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [private TS/JS CFG lowering, conservative dynamic-language CFG facts, merged language provider output]

key-files:
  created:
    - crates/polint/src/analysis/cfg/lower_ts.rs
  modified:
    - crates/polint/src/analysis/cfg/mod.rs
    - crates/polint/src/analysis/cfg/provider.rs

key-decisions:
  - "Lower TS/JS CFG from private semantic MIR rows and do not store Oxc AST/span objects in CFG facts."
  - "Merge Go and TS/JS base CFG outputs in the private provider with deterministic ID offsets before derived analyses run."
  - "Represent dynamic, async, cleanup, optional/nullish, throw, and unsupported semantics with typed CFG edges or unsupported control-flow rows instead of exact scheduler/runtime claims."

patterns-established:
  - "TS/JS branch rows lower into conservative conditional, loop, short-circuit, or nullish shapes based on MIR/source evidence."
  - "Throw rows add abrupt edges to the exceptional exit and leave following MIR operations in unreachable blocks."
  - "Unsupported MIR rows affecting CFG become UnsupportedControlFlowFact rows with conservative precision/status."

requirements-completed: []

duration: 31 min
completed: 2026-05-20
---

# Phase 29 Plan 05: TS/JS CFG Lowering Summary

**Private TS/JS CFG lowering over semantic MIR**

## Performance

- **Duration:** 31 min
- **Completed:** 2026-05-20
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `analysis::cfg::lower_ts::lower_ts_cfg` for TypeScript, TSX, JavaScript, and JSX MIR bodies.
- Wired the CFG provider to merge Go and TS/JS base CFG output with deterministic ID offsets before shared reachability/dominator/postdominator/control-dependence derivation.
- Lowered TS/JS branch, loop, short-circuit, nullish, return, throw, call, regular operation, and unsupported rows into deterministic CFG facts.
- Added conservative edge handling for optional chaining, await suspend/resume, yield suspend/resume, finally/cleanup, dynamic import, and unknown dynamic constructs.
- Added focused TS/JS CFG tests covering straight-line return, branch/loop/short-circuit/nullish edges, throw reachability structure, and async/cleanup/unsupported constructs.

## Task Commits

1. **Tasks 1-2:** `3b13284` feat - TS/JS CFG lowering and provider merge integration.

## Files Created/Modified

- `crates/polint/src/analysis/cfg/lower_ts.rs` - TS/JS semantic-MIR-to-CFG lowerer and tests.
- `crates/polint/src/analysis/cfg/mod.rs` - Registered the private TS/JS CFG module.
- `crates/polint/src/analysis/cfg/provider.rs` - Merged Go and TS/JS base CFG outputs before shared derived rows.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Accounted for language ID collisions when adding a second lowerer**
- **Found during:** Provider integration
- **Issue:** Independent Go and TS/JS lowerers both allocate run-local CFG IDs from 1, which would collide if their outputs were appended directly.
- **Fix:** Added provider-side deterministic ID offsetting for functions, nodes, blocks, edges, and unsupported rows before merging language outputs.
- **Files modified:** `crates/polint/src/analysis/cfg/provider.rs`
- **Verification:** `cargo test -p polint --lib cfg_provider --locked` and `cargo test -p polint --lib analysis::cfg --locked` passed.
- **Committed in:** `3b13284`

---

**Total deviations:** 1 auto-fixed (1 Rule 2)
**Impact on plan:** The provider now supports multi-language CFG output without run-local ID collisions.

## Issues Encountered

- The plan used shorthand `Language::Ts` / `Language::Js` names, while the codebase uses `Language::TypeScript` / `Language::JavaScript` plus `Tsx` / `Jsx`.
- Existing TS/JS MIR exposes coarse control evidence, so the CFG lowerer conservatively classifies shapes and preserves dynamic runtime uncertainty as unsupported CFG evidence.

## Verification

- `cargo test -p polint --lib analysis::cfg::lower_ts --locked` passed.
- `cargo test -p polint --lib ts_cfg_async_cleanup_and_unsupported --locked` passed.
- `cargo test -p polint --lib cfg_provider --locked` passed.
- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo test -p polint --lib analysis_kernel::validation::cfg --locked` passed.
- `cargo test -p polint --lib provider_order --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance `rg` checks for TS/JS CFG lowering, no derived-analysis calls inside `lower_ts.rs`, async/cleanup/optional/nullish evidence, and no Oxc AST/span leakage passed.

## Known Stubs

- TS/JS CFG lowering is conservative over the current MIR shape; richer target-aware modeling can improve once MIR carries structured statement nesting, labels, and terminator targets.
- Promise scheduling, dynamic import/eval behavior, and try/finally precision remain represented as conservative CFG evidence rather than exact runtime control flow.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-06 can add eval fixtures, public-boundary proof, and documentation alignment for the private CFG/control-dependence phase.

## Self-Check: PASSED

- Verified created files exist.
- Verified the task commit exists in git history.
- Verified targeted tests, CFG validation tests, formatting, and acceptance searches pass.

---
*Phase: 29-local-cfg-and-control-dependence*
*Completed: 2026-05-20*
