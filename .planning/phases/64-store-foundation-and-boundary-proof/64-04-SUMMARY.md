---
phase: 64-store-foundation-and-boundary-proof
plan: 04
subsystem: verification
tags: [public-api, regression-gate, benchmark, integration-tests, no-leak]

# Dependency graph
requires:
  - phase: 64-store-foundation-and-boundary-proof
    plan: 03
    provides: Kernel parity proof and isolated enabled-store measurement
  - phase: 63-ground-truth-and-performance-baseline
    provides: Committed store-disabled check baseline and locked regression gate
provides:
  - Public-source/output/generated-skill store-vocabulary leak gate with negative controls
  - Real isolated Phase 64 regression boundary against the committed baseline
  - Complete focused and all-feature workspace verification evidence
affects: [phase-65, public-api-gates, semantic-store, performance-regression]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Public-boundary scanners use curated implementation-specific markers plus per-family negative controls"
  - "Phase boundary measurements prime analysis caches in disabled mode, then include the enabled store's first-open/migration cost"

key-files:
  created: []
  modified:
    - crates/polint/tests/public_surface_leak.rs
    - tests/fixtures/public-surface-leak-probe/src/lib.rs
    - tests/fixtures/public-surface-leak-probe/Cargo.lock
    - crates/polint/src/eval/bench/gate.rs

key-decisions:
  - "The Phase 64 gate primes with a disabled digest, measures enabled first-open cost, then computes the enabled parity digest"
  - "Generic words such as store, row, and connection remain allowed; only exact internal namespaces/types/crate/schema/SQL identifiers are banned publicly"
  - "No threshold, absolute floor, prelude item, product activation path, or public contract changed"

patterns-established:
  - "Every store phase closes with a real enabled measurement plus digest parity, not just synthetic gate-unit tests"
  - "Outside-consumer compilation and public text/output scans jointly protect supported surfaces"

requirements-completed: [STORE-01, STORE-06, STORE-07, STORE-08, PERF-03, PROD-01, VAL-02]

# Metrics
duration: 31min
completed: 2026-07-10
---

# Phase 64 Plan 04: Boundary and Regression Gate Summary

**The private SQLite foundation closes behind an exact 115-name SDK boundary, a seven-test public no-leak gate, and a real enabled-store measurement that passes locked RSS, cold-time, and diagnostics-parity checks.**

## Performance

- **Duration:** ~31 min
- **Started:** 2026-07-10T10:49:28Z
- **Completed:** 2026-07-10T11:20:05Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Expanded the external/public leak gate to scan SDK, runner, CLI, crate root, README, API visibility plan, facts docs, examples, real check JSON, and generated skill text for curated store implementation markers.
- Kept `ALLOWED_PRELUDE` unchanged at exactly 115 names and compiled the excluded external probe using only `polint::sdk::prelude::*`.
- Added negative controls for private module/type, rusqlite crate, bootstrap table, SQLite flags, migration statement, and raw identifier marker families while proving generic prose does not false-positive.
- Evaluated a real store-enabled isolated fixture against `store-disabled-check.json`; after review hardening, three repeat runs measured ~41.7–42.0 MB RSS delta (about 1.03× baseline), 37–38 ms cold time, 8 KiB first-open store size, and an unchanged `28cac8a32a5bb2a9` diagnostics digest.
- Ran every focused Phase 64 fixture and the complete all-feature workspace suite with zero failures.

## Task Commits

1. **Task 1: Extend external/public semantic-store no-leak gate** - `694b0a0c` (test)
2. **Task 2: Wire real Phase 64 regression-budget boundary** - `2ffcf4ad` (test)
3. **Task 3: Run complete verification matrix** - verification-only; no source defect or additional commit was required.

## Files Created/Modified

- `crates/polint/tests/public_surface_leak.rs` - Marker scanner, recursive public-surface collection, real JSON/skill generation, and negative controls.
- `tests/fixtures/public-surface-leak-probe/src/lib.rs` - Explicitly documents semantic-store/SQLite internals as forbidden probe imports.
- `tests/fixtures/public-surface-leak-probe/Cargo.lock` - Refreshed excluded-probe lock for polint's new private rusqlite transitive dependency.
- `crates/polint/src/eval/bench/gate.rs` - Test-facing real Phase 64 boundary loader/measurement/digest evaluator and committed-fixture test.

## Decisions Made

- Matched the committed baseline generator's analysis-cache state without hiding store creation: a disabled check digest primes analysis/toolchain caches, the isolated enabled point then creates/migrates the absent store, and an enabled digest afterward proves parity. This produced three consecutive 37–38 ms passes without changing thresholds.
- Retained the locked `1.20` RSS ratio, `1.25` cold ratio, 16 MiB RSS floor, and 50 ms cold floor exactly. On this tiny fixture the existing absolute cold floor yields a 76 ms effective ceiling; production-scale baselines remain ratio-governed.
- Included the probe lockfile refresh because building the excluded external consumer now legitimately resolves polint's private bundled SQLite dependency; it does not expose rusqlite through Rust APIs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Correctness] Matched real gate cache state to the committed generator**

- **Found during:** Task 2 repeated real boundary execution.
- **Issue:** Measuring before the digest run compared an unprimed enabled point to a baseline whose generator explicitly ran the digest first, producing timing-only false failures (72–135 ms) while disabled control runs showed the same toolchain variance.
- **Fix:** Reordered Phase 64 boundary evaluation to the baseline generator's documented digest-then-isolated-measurement sequence.
- **Files modified:** `crates/polint/src/eval/bench/gate.rs`
- **Verification:** The initial cache-order correction passed at 58–65 ms; code review then refined it to retain first-open cost, with three consecutive 37–38 ms passes. No threshold/floor changed.
- **Committed in:** `2ffcf4ad`

**2. [Rule 3 - Blocking] Refreshed external probe lockfile**

- **Found during:** Task 1 outside-consumer build.
- **Issue:** The excluded probe's lockfile lacked the new private rusqlite/libsqlite3 dependency graph and was rewritten by its normal cargo build.
- **Fix:** Committed the deterministic lock refresh with the probe test.
- **Files modified:** `tests/fixtures/public-surface-leak-probe/Cargo.lock`
- **Verification:** External prelude-only probe build passed in focused and full workspace tests.
- **Committed in:** `694b0a0c`

---

**Total deviations:** 2 auto-fixed (1 correctness, 1 blocking dependency fixture)
**Impact on plan:** The real gate became comparable and repeatable without relaxing budgets; the external consumer remains API-isolated.

## Issues Encountered

- The complete workspace suite is intentionally large: library tests took ~397 seconds and CLI integrations ~627 seconds. Both completed normally with zero failures.

## User Setup Required

None - the store remains disabled in production and all new activation/gate plumbing is internal or cfg(test).

## Verification

- Focused cache tests: 36 passed.
- Migration fixtures: 9 passed after review hardening (including wrong-shape and extra-marker current schemas).
- Store connection/contention/recovery suite: 11 passed.
- Kernel store/parity tests: 3 matching tests passed; dedicated six-mode parity test passed.
- Isolated store benchmark test: passed.
- Public surface leak gate: 7 passed; prelude count remained 115.
- Real Phase 64 boundary: passed; RSS, cold-time, and diagnostics checks all present and Pass.
- `make lint`: passed.
- `cargo test --workspace --all-features --locked`:
  - polint library: 2,421 passed, 1 intentional ignore; the separate slow cargo-install smoke test remained intentionally ignored.
  - CLI integration: 166 passed.
  - public leak integration: 7 passed.
  - polint-bench: 2 passed.
  - polint-macros: 11 passed.
  - example crates and doctests: all passed.
- Source inspection: `rusqlite` appears only under `crates/polint/src/analysis_kernel/store/`; no SDK/runner/CLI public store exposure and no provider/rule API parameter.

## Next Phase Readiness

- Phase 64 is implementation-complete and ready for code review plus requirements verification.
- Phase 65 can add manifest/generation/fact persistence behind the established private boundary; production activation remains intentionally off.
- No blockers and no large-repo measurement claim: this phase gate uses the committed deterministic tiny fixture. The locked real-repo suite remains the later scale-validation path.

## Post-Plan Code Review Fixes

- `8ac63000` adds a compatibility preflight before WAL, maps malformed bootstrap shapes to typed invalid-schema outcomes, requires exactly one marker row, and changes the boundary to include real first-open/migration cost after disabled cache priming.
- Phase 64's second review pass is clean with zero remaining findings.

---
*Phase: 64-store-foundation-and-boundary-proof*
*Completed: 2026-07-10*

## Self-Check: PASSED

The leak/gate files and this summary exist, task commits `694b0a0c` and `2ffcf4ad` are in history, and the focused plus all-feature workspace gates completed successfully.
