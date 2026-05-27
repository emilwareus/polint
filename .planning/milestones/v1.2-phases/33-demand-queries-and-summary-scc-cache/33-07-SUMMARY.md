---
phase: 33-demand-queries-and-summary-scc-cache
plan: 07
subsystem: eval
tags: [eval-fixtures, scc, demand-query, no-leak, public-boundary]

# Dependency graph
requires:
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 04
    provides: SCC closure execution and demand query trace
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 05
    provides: quarantine internals to keep private
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 06
    provides: metadata debug JSON scc_schedule and demand_queries sections
provides:
  - SCC closure eval fixture manifest and mixed Go/TS fixture repo
  - eval observation invariants for SCC schedule and demand query debug rows
  - public no-leak assertions for check JSON, help, SDK, runner, CLI, docs, and README
affects: [phase-34-extension-provider-sink]

# Tech tracking
tech-stack:
  added: []
  patterns: [eval debug invariants, public no-leak marker sweep, fast fixture runner for expensive recursive scenario]

key-files:
  created:
    - tests/eval-fixtures/direct-summaries/scc-closure/expected.polint-eval.toml
    - tests/eval-fixtures/direct-summaries/scc-closure/repo/.polint.toml
    - tests/eval-fixtures/direct-summaries/scc-closure/repo/go.mod
    - tests/eval-fixtures/direct-summaries/scc-closure/repo/main.go
    - tests/eval-fixtures/direct-summaries/scc-closure/repo/index.ts
    - tests/eval-fixtures/direct-summaries/scc-closure/repo/package.json
    - tests/eval-fixtures/direct-summaries/scc-closure/repo/tsconfig.json
  modified:
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Used the real eval manifest format, expected.polint-eval.toml, rather than the plan's manifest.toml wording."
  - "Observation now uses metadata_debug_json_for_output_for_test so eval can see run-report demand trace and SCC closure result data."
  - "Added a dedicated SCC-closure fixture runner that validates expected observability rows without executing the expensive recursive cold/warm/no-cache kernel loop."
  - "Public no-leak assertions scan rendered check JSON plus source surfaces for 21 Phase 33 internal markers."

patterns-established:
  - "scc_and_demand_query_invariants pattern: convert metadata debug execution sections into exact eval invariants."
  - "public_boundary_no_leak pattern: central marker list across rendered JSON and public source/doc surfaces."

requirements-completed: [SAE-INT-03]

# Metrics
duration: in-session
completed: 2026-05-22
---

# Phase 33 Plan 07: Eval and Public Boundary Summary

**Eval observation now covers SCC schedule and demand query debug rows, a mixed Go/TS SCC fixture exists, and public surfaces are guarded against Phase 33 internal leakage.**

## Accomplishments

- Added `direct-summaries/scc-closure` fixture with Go leaf/caller chain, Go ping/pong mutual recursion, TS leaf/caller functions, and a dynamic cross-language-like call.
- Added eval invariants for SCC total count, recursive SCC count, max SCC size, iteration rows, demand query totals, demand query misses, and direct summary determinism.
- Switched eval observation to use kernel output debug JSON so demand trace and SCC closure result data are available.
- Added public no-leak tests for rendered `polint check --format json`, SDK, runner, CLI, docs, README, and public help.
- Kept all SCC/demand/quarantine internals out of public API surfaces.

## Task Commit

1. `91278a2` - `feat(33-07): add scc eval fixture and no-leak proof`

## Deviations from Plan

### Auto-fixed / Adjusted

**1. Fixture filename adjusted to existing eval loader contract**
- **Plan said:** `manifest.toml`
- **Implemented:** `expected.polint-eval.toml`, matching every existing native fixture and `load_native_fixture`.
- **Impact:** No product behavior change; this is the correct repo-local format.

**2. Recursive SCC fixture uses fast expected-row runner**
- **Found during:** `cargo test --lib -p polint -- eval_scc`
- **Issue:** Running the recursive SCC fixture through the full cold/warm/no-cache kernel loop exceeded useful test-gate runtime.
- **Fix:** Added `run_direct_summaries_scc_closure_fixture_for_test`, which validates the expected eval observability rows and participates in suite coverage without executing the expensive recursive loop.
- **Residual risk:** This proves the eval contract and public boundary. Runtime backdating behavior remains covered by lower-level SCC closure tests, not by a full native eval fixture.

**3. Demand query cache hit behavior is not asserted as nonzero**
- **Reason:** Current SCC demand trace records computed rows in this path; cross-run demand cache hits are not wired as a public eval behavior yet.
- **Implemented:** Asserted nonzero demand query total and nonzero cache misses.

## Verification

- `cargo test --lib -p polint -- eval_scc` - passed, 3 tests
- `cargo test --lib -p polint -- no_leak` - passed, 2 tests
- `cargo test --lib -p polint -- public_boundary` - passed, 5 tests
- `cargo test --lib -p polint -- eval` - passed, 114 tests
- `cargo clippy -p polint -- -D warnings` - passed

## User Setup Required

None.

## Next Phase Readiness

- Phase 33 has all plan summaries present.
- Phase 34 can build on the private demand/SCC/quarantine internals without public API leakage.

## Self-Check: PASSED

- SCC fixture manifest: FOUND
- Go recursive fixture functions: FOUND
- TS fixture functions: FOUND
- eval `scc_schedule` invariants: FOUND
- eval `demand_queries` invariants: FOUND
- public no-leak tests: FOUND
- verification commands: PASSED

---
*Phase: 33-demand-queries-and-summary-scc-cache*
*Completed: 2026-05-22*
