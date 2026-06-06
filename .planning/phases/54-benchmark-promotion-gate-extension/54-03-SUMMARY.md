---
phase: 54-benchmark-promotion-gate-extension
plan: 03
subsystem: ci
tags: [github-actions, eval, polyglot, public-api, determinism]

requires:
  - phase: 54-benchmark-promotion-gate-extension
    provides: Plan 54-01 metric/report foundation
provides:
  - Named promotion gate CI job
  - Local proof that polyglot canary selector runs all three canary lanes
  - Local proof that public-surface leak and determinism gates pass
affects: [ci, phase-54-promotion-gates, public-api-visibility]

tech-stack:
  added: []
  patterns: [linux-macos-independent-gate, exact-ci-command-wiring]

key-files:
  created: []
  modified:
    - .github/workflows/ci.yml

key-decisions:
  - "Existing polyglot Rust tests already satisfy the named `polyglot` selector and did not need code changes."
  - "The public-surface leak allowlist stayed frozen; CI wiring reuses the existing test."

patterns-established:
  - "Promotion gate CI runs polyglot, public-surface leak, and determinism commands together on ubuntu-latest and macos-latest with fail-fast disabled."

requirements-completed: [BENCH-01]

duration: 25min
completed: 2026-06-06
---

# Phase 54 Plan 03: Benchmark Promotion Gate Extension Summary

**Linux/macOS promotion CI job for the polyglot canary, public API leak gate, and determinism gate**

## Performance

- **Duration:** 25 min
- **Started:** 2026-06-06T05:44:00Z
- **Completed:** 2026-06-06T06:09:00Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Verified `cargo test -p polint polyglot --lib --locked` runs the Go RTA, TS token, and TS object-model canary tests.
- Added a named `promotion gate` CI job that runs on `ubuntu-latest` and `macos-latest` with `fail-fast: false`.
- Preserved the existing public-surface leak gate and `ALLOWED_PRELUDE` allowlist unchanged.

## Task Commits

1. **Task 1: Polyglot canary selector verification** - no code change; existing tests already satisfied the plan.
2. **Task 2: CI promotion gate job** - `1d80e956`
3. **Task 3: Public prelude leak gate verification** - no code change; leak gate passed and allowlist stayed frozen.

## Files Created/Modified

- `.github/workflows/ci.yml` - Adds the `promotion-gate` job with exact polyglot, leak, and determinism commands.

## Decisions Made

- Did not touch `go_rta.rs`, `ts_tokens.rs`, or `ts_object_model.rs` because their test names and assertions already contained the `polyglot` selector and boundary checks.
- Did not touch `public_surface_leak.rs`; the frozen allowlist remains the source of truth.

## Deviations from Plan

None - plan executed as written. Some listed files were verified rather than modified because they already met the acceptance criteria.

## Issues Encountered

None.

## Verification

- `cargo test -p polint polyglot --lib --locked` - passed, 3 tests.
- `cargo test --package polint --test public_surface_leak --locked` - passed, 5 tests.
- `cargo test -p polint --lib eval::determinism_gate --locked` - passed, 13 tests.
- `rg -n "promotion gate|cargo test -p polint polyglot --lib --locked|public_surface_leak|eval::determinism_gate" .github/workflows/ci.yml` - passed.
- `git diff --check` - passed.
- Commit hook ran `make lint` and passed for `1d80e956`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Final audit and milestone closeout can reference the CI promotion job and local gate results in Plan 54-04.

## Self-Check: PASSED

---
*Phase: 54-benchmark-promotion-gate-extension*
*Completed: 2026-06-06*
