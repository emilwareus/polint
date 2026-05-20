---
phase: 29-local-cfg-and-control-dependence
plan: 06
subsystem: static-analysis-engine
tags: [rust, cfg, eval, public-boundary, capability-honesty]

requires:
  - phase: 29-local-cfg-and-control-dependence
    plan: 01
    provides: private CFG contracts and storage
  - phase: 29-local-cfg-and-control-dependence
    plan: 02
    provides: shared CFG builder and derived analyses
  - phase: 29-local-cfg-and-control-dependence
    plan: 03
    provides: CFG provider, validation, cache identity, and debug output
  - phase: 29-local-cfg-and-control-dependence
    plan: 04
    provides: Go CFG lowering
  - phase: 29-local-cfg-and-control-dependence
    plan: 05
    provides: TS/JS CFG lowering
provides:
  - CFG eval fixture area and observed fact rows
  - Go and TS/JS CFG core eval fixture coverage
  - CFG public no-leak and unsupported capability proof
  - final Phase 29 validation for SAE-SEM-04
affects: [phase-29, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [private eval fixtures, compact debug-derived observations, public-boundary tests]

key-files:
  created:
    - tests/eval-fixtures/cfg-core/expected.polint-eval.toml
    - tests/eval-fixtures/cfg-core/repo/go/cfg.go
    - tests/eval-fixtures/cfg-core/repo/ts/cfg.ts
  modified:
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/tests/cli.rs
    - crates/polint/src/analysis/cfg/builder.rs
    - crates/polint/src/analysis/cfg/validate.rs
    - crates/polint/src/analysis_kernel/metadata.rs

key-decisions:
  - "Keep CFG eval support crate-private and test-facing, sourced only from metadata_debug_json_for_test."
  - "Use the existing TOML eval fixture manifest format instead of adding JSON fixture files."
  - "Keep CFG internals out of README, docs/facts, SDK, runner, CLI help, check JSON, inspect JSON, and polint test JSON."
  - "Keep the reserved public cfg capability unsupported until a later intentional promotion phase."
  - "CFG stable keys must use MIR/body stable identity, not run-local CFG IDs, to avoid cross-language and cross-function collisions."

patterns-established:
  - "CFG eval observations use compact semicolon payload fragments and do not include raw source or absolute paths."
  - "The cfg-core fixture asserts CfgFunction, CfgNode, BasicBlock, CfgEdge, reachability, dominator, postdominator, control-dependence, unsupported-control-flow, and determinism rows."
  - "Provider precision metadata remains setup-aware for semantic/module-derived facts whose precision depends on configured lifecycle or semantic setup."

requirements-completed:
  - SAE-SEM-04

duration: 68 min
completed: 2026-05-20
---

# Phase 29 Plan 06: CFG Eval and Privacy Proof Summary

**CFG validation fixtures and public-boundary proof**

## Performance

- **Duration:** 68 min
- **Completed:** 2026-05-20
- **Tasks:** 3
- **Files modified:** 19

## Accomplishments

- Added CFG as an internal eval fixture area with observed rows for functions, nodes, blocks, edges, reachability, dominators, postdominators, control dependence, and unsupported control flow.
- Added `cfg-core` eval fixture coverage with Go and TS/JS source exercising branches, loops, returns, short-circuiting, panic/throw behavior, cleanup/unsupported evidence, and unreachable code.
- Added deterministic fixture assertions, including `cfg.current_determinism`.
- Added CLI public-boundary tests proving CFG internals do not leak through public surfaces.
- Added capability-honesty coverage proving reserved `cfg` remains unsupported and rules do not run with placeholder CFG facts.
- Fixed integration issues surfaced by full verification: CFG operation nodes now carry their actual block, CFG stable keys avoid run-local ID collisions, edge validation distinguishes node endpoints, provider-order tests include `polint.cfg`, and setup-aware provider precision ceilings are enforced consistently.

## Task Commits

1. **Tasks 1-3:** `e67fd60` feat - CFG eval fixtures, public-boundary proof, and integration fixes.

## Files Created/Modified

- `tests/eval-fixtures/cfg-core/` - Native CFG fixture repo and TOML expected observations.
- `crates/polint/src/eval/model.rs` - CFG fixture area and required family labels.
- `crates/polint/src/eval/observed.rs` - CFG debug JSON observation into eval facts.
- `crates/polint/src/eval/fixtures.rs` - CFG fixture runner, determinism invariant, and coverage tests.
- `crates/polint/tests/cli.rs` - CFG public no-leak and capability-honesty tests.
- `crates/polint/src/analysis/cfg/*` - Stable-key/block-reference validation fixes discovered by full integration testing.
- `crates/polint/src/analysis_kernel/*`, `crates/polint/src/module_graph/mod.rs`, `crates/polint/src/symbol_graph/mod.rs` - Provider-order and setup-aware metadata precision alignment.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Used existing TOML eval fixture format**
- **Found during:** Fixture implementation
- **Issue:** The plan named `manifest.json` and `expected.json`, but the current eval harness uses `expected.polint-eval.toml`.
- **Fix:** Implemented `cfg-core` with the existing TOML fixture format and runner path.
- **Verification:** `cargo test -p polint --lib eval::fixtures::cfg_core --locked` passed.
- **Committed in:** `e67fd60`

**2. [Rule 2 - Missing Critical] Fixed CFG row identity and validation after full-suite integration**
- **Found during:** `cargo test -p polint --all-targets --locked`
- **Issue:** Multi-function and multi-language CFG rows could collide when stable keys included run-local CFG IDs; operation nodes also kept the placeholder block id until validation caught it.
- **Fix:** CFG builder stable keys now include MIR/body stable identity, operation nodes record their real block, and edge validation keys include node endpoints.
- **Verification:** `cargo test -p polint --all-targets --locked` passed.
- **Committed in:** `e67fd60`

**3. [Rule 2 - Missing Critical] Aligned provider precision ceilings with active metadata validation**
- **Found during:** Public CLI fixture tests
- **Issue:** Existing semantic/module-derived metadata could claim `Exact` while their providers are setup-aware.
- **Fix:** Mapped exact file resolution and exact semantic symbol precision to setup-aware metadata precision while preserving source fact exactness.
- **Verification:** Full CLI integration tests passed.
- **Committed in:** `e67fd60`

---

**Total deviations:** 3 auto-fixed (3 Rule 2)
**Impact on plan:** The phase now validates against the full existing test suite, not only the new CFG-focused tests.

## Issues Encountered

- The plan listed invalid multi-filter Cargo commands. I ran the equivalent filters separately and also ran the full all-targets suite.
- `crates/polint/src/runner.rs` does not exist; public source scanning targets the actual `crates/polint/src/runner/` module directory.
- Broad `docs/` contains older research/roadmap mentions of future CFG concepts. The no-leak assertion targets supported public surfaces: README, `docs/facts/`, SDK, runner, CLI, crate-root public prelude, and CLI help/output.

## Verification

- `cargo test -p polint --lib eval::observed::cfg --locked` passed.
- `cargo test -p polint --lib eval::fixtures::cfg_core --locked` passed.
- `cargo test -p polint --test cli cfg_public_no_leak --locked` passed.
- `cargo test -p polint --test cli cfg_capability_remains_unsupported --locked` passed.
- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo test -p polint --lib analysis_kernel --locked` passed.
- `cargo test -p polint --all-targets --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance `rg` checks for CFG observed rows, fixture area wiring, fixture source constructs, expected fact families, public no-leak tests, and capability-honesty tests passed.

## Known Stubs

- CFG remains private/internal; no `docs/facts/cfg.md`, SDK view, runner API, or public CLI/debug schema was promoted.
- CFG precision is intentionally conservative for dynamic language cleanup, async/yield, optional/nullish, and unsupported constructs.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

Phase 29 is complete. Phase 30 can build direct call facts on top of private semantic MIR and CFG infrastructure.

## Self-Check: PASSED

- Verified created fixture files exist.
- Verified task commit exists in git history.
- Verified targeted eval, CFG, public-boundary, and full all-targets tests pass.
- Verified CFG internals remain private and `cfg` capability remains unsupported.

---
*Phase: 29-local-cfg-and-control-dependence*
*Completed: 2026-05-20*
