---
phase: 03-core-facts-and-diagnostics
plan: "01"
subsystem: core
tags: [rust, polint-core, facts, diagnostics, proptest, rayon]

requires:
  - phase: 01-workspace-foundation
    provides: Rust workspace and crate boundaries
  - phase: 02-cli-config-and-discovery
    provides: CLI/config/discovery loop that feeds AnalysisDb
provides:
  - Hardened AnalysisDb fact accessors and deterministic typed ID coverage
  - Byte-offset span conversion tests for UTF-8, newlines, empty ranges, and monotonicity
  - Deterministic rule runner behavior for sequential and parallel execution
  - Rule registry, filtering, severity override, error/panic containment, and dedupe tests
affects: [phase-04-go-adapter, phase-05-ts-adapter, phase-06-sdk-rules, phase-07-cache, phase-08-ci-output]

tech-stack:
  added: [proptest workspace dev-dependency for polint-core]
  patterns:
    - Vec-backed append-only AnalysisDb with internally assigned typed IDs
    - Ordered Rayon collection followed by deterministic diagnostics dedupe
    - Property coverage for span monotonicity over valid UTF-8 char boundaries

key-files:
  created:
    - .planning/phases/03-core-facts-and-diagnostics/03-01-SUMMARY.md
  modified:
    - Cargo.lock
    - crates/polint-core/Cargo.toml
    - crates/polint-core/src/lib.rs

key-decisions:
  - "Preserved the existing Vec-backed AnalysisDb contract and added only the missing coverage accessor."
  - "Kept Span::diagnostic_range as the narrow core-to-diagnostics conversion point."
  - "Replaced mutex-based parallel diagnostic appending with ordered Rayon collection before dedupe."

patterns-established:
  - "Runner tests use local test-rule structs to exercise the SDK-facing Rule contract without broadening built-in rules."
  - "Duplicate diagnostic tests pin deterministic dedupe behavior when fingerprints collide."

requirements-completed: [CORE-01, CORE-02, TEST-01, TEST-04]

duration: 7min
completed: 2026-04-28
---

# Phase 03 Plan 01: Core Facts and Runner Summary

**Core fact storage, span conversion, and rule execution are now covered by deterministic unit/property tests with ordered parallel runner output.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-28T11:30:12Z
- **Completed:** 2026-04-28T11:37:01Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added focused fact-model tests proving deterministic `FileId`, `FunctionId`, `ImportId`, and `BranchId` assignment while preserving `SourceFile.source: Arc<str>`.
- Added `AnalysisDb::coverage()` so all Phase 3 fact families have read accessors without changing the append-only storage model.
- Hardened `span_from_byte_range` by clamping and normalizing byte offsets before line/column conversion.
- Added unit and property coverage for UTF-8 spans, newline boundaries, empty ranges, clamped ranges, and monotonic diagnostic ranges.
- Added runner tests for capability declarations, enabled-rule matching, severity overrides, panic/error containment, sequential/parallel equivalence, and duplicate fingerprint dedupe.
- Replaced mutex-based parallel diagnostic appending with ordered `par_iter().map(...).collect()` followed by flattening and `dedupe_diagnostics`.

## Task Commits

1. **Task 1 RED:** `0b8c0d7` test(03-01): add failing core fact and span tests
2. **Task 1 GREEN:** `73fa89d` feat(03-01): harden core fact and span contracts
3. **Task 2 RED:** `4994265` test(03-01): add failing runner determinism tests
4. **Task 2 GREEN:** `93c3287` feat(03-01): make parallel rule execution deterministic
5. **Verification cleanup:** `563d2e5` chore(03-01): format core test additions
6. **Stub-scan cleanup:** `5a95425` test(03-01): use explicit coverage fixture source

## Files Created/Modified

- `Cargo.lock` - Locked `proptest` and transitive test dependencies pulled from workspace dependency usage.
- `crates/polint-core/Cargo.toml` - Added `proptest.workspace = true` under dev-dependencies.
- `crates/polint-core/src/lib.rs` - Added coverage accessor, span clamping, ordered parallel runner collection, and focused tests.
- `.planning/phases/03-core-facts-and-diagnostics/03-01-SUMMARY.md` - Execution record for this plan.

## Verification

- `cargo test -p polint-core --lib analysis_db` - passed
- `cargo test -p polint-core --lib span_from_byte_range` - passed
- `cargo test -p polint-core --lib line_col` - passed
- `cargo test -p polint-core --lib run_rules` - passed
- `cargo test -p polint-core --lib registry_exposes_capability_declarations` - passed
- `cargo clippy -p polint-core --all-targets -- -D warnings` - passed
- `cargo fmt -- --check` - passed
- `cargo test -p polint-core --lib` - passed, 11 tests
- `cargo check -p polint-go -p polint-ts -p polint-rules` - passed

## Decisions Made

- Kept the public core API additive: adapters and rules continue to compile with the preserved `AnalysisDb`, `RuleCtx`, `RuleRegistry`, and `run_rules` shapes.
- Treated coverage facts as an honest v1 model with `covered: Option<bool>` and a source label, without claiming exact runtime coverage.
- Used ordered parallel collection instead of a shared mutex so Rayon scheduling cannot affect which duplicate diagnostic survives dedupe.

## Deviations from Plan

None to implementation scope - the plan was executed as written.

## Issues Encountered

- `cargo fmt -- --check` found formatting drift in the new tests. Applied `cargo fmt` and committed the cleanup in `563d2e5`.
- Stub tracking initially matched a generic stub keyword in test fixture data. Renamed it to `synthetic-coverage` in `5a95425`; no product stub existed.

## Known Stubs

None - stub scan returned no matches in files modified by this plan.

## Auth Gates

None.

## Next Phase Readiness

Phase 4 and Phase 5 adapters can rely on stable typed IDs, source text shared through `Arc<str>`, complete Phase 3 fact accessors, and hardened span conversion. SDK and rules work can rely on deterministic runner output for both sequential and parallel execution.

---
*Phase: 03-core-facts-and-diagnostics*
*Completed: 2026-04-28*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/03-core-facts-and-diagnostics/03-01-SUMMARY.md`.
- Verified task commits exist: `0b8c0d7`, `73fa89d`, `4994265`, `93c3287`, `563d2e5`, `5a95425`.
- Stub scan of source files modified by this plan returned no matches.
