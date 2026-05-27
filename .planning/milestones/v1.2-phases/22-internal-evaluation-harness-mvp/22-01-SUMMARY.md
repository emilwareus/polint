---
phase: 22-internal-evaluation-harness-mvp
plan: "01"
subsystem: evaluation-harness
tags: [rust, eval, deterministic-json, stable-hash, internal-api]

requires:
  - phase: 20-private-analysis-kernel-facade
    provides: Crate-private analysis-kernel boundary and provider manifest discipline
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Internal provenance, precision, validation, and stable-key metadata vocabulary
provides:
  - Crate-private canonical evaluation model for expected and observed diagnostics, facts, graph edges, paths, invariants, and runtime budgets
  - Deterministic evaluation report serialization with sorted cases, items, and matches
  - Deterministic output hashing over canonical JSON using cache stable_hash while excluding runtime duration observations
affects: [22-02-generic-matchers, 22-03-native-fixture-runner, evaluation-harness, promotion-gates]

tech-stack:
  added: []
  patterns:
    - crate-private eval module with no SDK, runner, crate-root public, or CLI surface
    - deterministic report normalization before JSON serialization and hashing
    - runtime duration observations remain reportable but excluded from semantic output hashes

key-files:
  created:
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/report.rs
    - .planning/phases/22-internal-evaluation-harness-mvp/22-01-SUMMARY.md
  modified:
    - crates/polint/src/lib.rs

key-decisions:
  - "Keep eval crate-private and internal; no public SDK, runner, crate-root public, or CLI contract was introduced."
  - "Normalize reports by sorting cases, expected items, observed items, and matches before serialization and hashing."
  - "Compute output hashes from canonical JSON with output_hash cleared and runtime durations removed, while preserving runtime pass/fail semantics."
  - "Use a scoped dead_code lint expectation on the eval module until later Phase 22 plans consume the foundation types."

patterns-established:
  - "Evaluation item identity is derived from stable, normalized fields rather than transient runtime or machine-local data."
  - "Report output_hash is computed from semantic canonical JSON and is not self-referential."

requirements-completed: [SAE-FND-03]

duration: 10 min
completed: 2026-05-17
---

# Phase 22 Plan 01: Evaluation Model and Report Hashing Summary

**Crate-private evaluation schema with deterministic report JSON and semantic output hashing for Phase 22 fixtures**

## Performance

- **Duration:** 10 min
- **Started:** 2026-05-17T16:25:59Z
- **Completed:** 2026-05-17T16:36:10Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `crates/polint/src/eval/` as a crate-private internal module registered from `lib.rs`.
- Added canonical expected/observed item models for diagnostics, facts, graph edges, paths, invariants, runtime budgets, assertion modes, fixture areas, and observed statuses.
- Added deterministic evaluation report types and helpers: `normalize_run`, `to_deterministic_json_pretty`, and `deterministic_output_hash`.
- Proved report JSON is order-independent for normalized data and that hashes ignore runtime duration observations while changing on semantic output changes.

## Task Commits

Each TDD step was committed atomically:

1. **Task 1 RED: Add failing eval model tests** - `1d129a9` (test)
2. **Task 1 GREEN: Implement eval item model** - `4cb966b` (feat)
3. **Task 2 RED: Add failing eval report hashing tests** - `8557ce6` (test)
4. **Task 2 GREEN: Implement deterministic eval reports** - `55304cc` (feat)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/eval/mod.rs` - Internal eval module registration and scoped dead-code expectation for this foundation slice.
- `crates/polint/src/eval/model.rs` - Canonical crate-private expected/observed evaluation item model and focused unit tests.
- `crates/polint/src/eval/report.rs` - Deterministic report schema, normalization, JSON serialization, output hashing, and hash behavior tests.
- `crates/polint/src/lib.rs` - Registers `eval` as `pub(crate) mod eval;`.

## Decisions Made

- Kept the entire eval foundation crate-private and did not add a public `polint eval` command, SDK export, runner API, or crate-root public module.
- Forced the report schema version during normalization so serialized reports consistently use `polint-eval-internal-1`.
- Excluded runtime duration fields from output hashes by clearing `RuntimeObservation.observed_runtime_ms` and observed runtime-budget durations before hashing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Scoped dead-code expectation for unused internal eval foundation**
- **Found during:** Task 2 (deterministic evaluation report serialization and output hashing)
- **Issue:** `cargo clippy -p polint --lib --all-features --locked -- -D warnings` failed because the new crate-private eval model/report types are intentionally not consumed by production code until later Phase 22 plans.
- **Fix:** Added a module-scoped `cfg_attr(not(test), expect(dead_code, ...))` in `crates/polint/src/eval/mod.rs`.
- **Files modified:** `crates/polint/src/eval/mod.rs`
- **Verification:** `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- **Committed in:** `55304cc`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The fix keeps workspace linting strict while allowing this planned internal foundation to compile before later harness consumers exist.

## Issues Encountered

None beyond the auto-fixed clippy/dead-code blocker documented above.

## Verification

- `cargo test -p polint --lib eval_model --locked`
- `cargo test -p polint --lib eval_report --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo test -p polint --lib eval --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None. Stub scan found no TODO/FIXME markers, placeholder text, or hardcoded empty values that flow to user-visible output.

## Threat Flags

None. The new machine-readable JSON surface is crate-private/internal, path data is modeled as relative paths, and transient runtime/local fields are omitted or excluded from hashes by test-covered helpers.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 22-02 can build generic matchers and unified metrics on the canonical item model and deterministic report/hash foundation from this plan.

## Self-Check: PASSED

- Found created files: `crates/polint/src/eval/mod.rs`, `crates/polint/src/eval/model.rs`, `crates/polint/src/eval/report.rs`, and this summary.
- Found task commits: `1d129a9`, `4cb966b`, `8557ce6`, and `55304cc`.

---
*Phase: 22-internal-evaluation-harness-mvp*
*Completed: 2026-05-17*
