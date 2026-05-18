---
phase: 22-internal-evaluation-harness-mvp
plan: "05"
subsystem: evaluation-harness
tags: [rust, eval, fixtures, synthetic-observed, extension-delta, metrics]

requires:
  - phase: 22-internal-evaluation-harness-mvp
    provides: Generic matcher, metrics, deterministic report hashing, and native fixture runner from Plans 22-01 through 22-03
  - phase: 22-internal-evaluation-harness-mvp
    provides: Strict fixture manifest parsing, provenance matching, and cache determinism fixtures from Plan 22-04
provides:
  - Synthetic extension rejection/delta fixture covering diagnostics, facts, graph edges, paths, invariants, runtime budgets, traps, and accepted/rejected facts
  - Manifest-owned synthetic observed rows gated to extension fixtures only
  - Eval metric counters for present, accepted, and rejected fact statuses
affects: [22-06-fixture-coverage, phase-34-extension-provider, phase-40-promotion-gates, evaluation-harness]

tech-stack:
  added: []
  patterns:
    - extension fixtures may use manifest-owned synthetic observed rows only when `area = "extension"` and `synthetic_observed = true`
    - observed fact status now flows through match summaries into deterministic metric reports
    - default-vs-extension deltas are represented as invariant rows, not as an activated extension execution mode

key-files:
  created:
    - tests/eval-fixtures/extension/rejection-delta/expected.polint-eval.toml
    - tests/eval-fixtures/extension/rejection-delta/repo/.polint.toml
    - tests/eval-fixtures/extension/rejection-delta/repo/src/app.ts
    - .planning/phases/22-internal-evaluation-harness-mvp/22-05-SUMMARY.md
  modified:
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/report.rs

key-decisions:
  - "Keep synthetic observed rows manifest-owned, test-facing, and rejected outside extension fixtures."
  - "Count present, accepted, and rejected observed fact statuses separately in eval metrics."
  - "Represent extension delta evidence with normalized invariant rows and `extension.real_sink_active = false`."
  - "Do not add any real extension provider activation, merge surface, CLI, SDK, or runner contract in this plan."

patterns-established:
  - "Fixture manifests can provide synthetic observed rows only through the explicit extension-only gate."
  - "Match summaries carry observed status so metrics do not parse serialized keys."

requirements-completed: [SAE-FND-03]

duration: 8 min
completed: 2026-05-17
---

# Phase 22 Plan 05: Synthetic Extension Rejection and Delta Fixture Summary

**Extension-style accepted, rejected, and changed facts proven through a synthetic native fixture without activating extension execution**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-17T17:34:11Z
- **Completed:** 2026-05-17T17:42:12Z
- **Tasks:** 1
- **Files modified:** 9

## Accomplishments

- Added `tests/eval-fixtures/extension/rejection-delta`, a synthetic extension fixture covering diagnostics, accepted/rejected facts, a graph edge, a three-node path, delta invariants, runtime budget pass, and one false-positive trap hit.
- Extended native fixture loading so manifest-owned `observed` rows are allowed only for `area = "extension"` with `synthetic_observed = true`; other areas are rejected.
- Carried observed fact status through match summaries and metrics so present, accepted, and rejected fact rows are counted separately.
- Kept the extension delta as normalized invariant evidence, including `extension.real_sink_active = "false"`, without adding a real extension/provider execution surface.

## Task Commits

This TDD task produced two atomic commits:

1. **Task 1 RED: Add failing synthetic extension fixture** - `b08f38f` (test)
2. **Task 1 GREEN: Implement synthetic extension fixture rows** - `1249fab` (feat)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/eval/fixtures.rs` - Synthetic observed manifest loading, extension-only validation, and extension fixture tests.
- `crates/polint/src/eval/matcher.rs` - Match summaries now carry observed status.
- `crates/polint/src/eval/metrics.rs` - Adds separate fact status counters and tests for present/accepted/rejected rows.
- `crates/polint/src/eval/report.rs` - Adds observed status and fact status counters to deterministic report data.
- `crates/polint/src/eval/observed.rs` - Updates existing test report construction for the new metric fields.
- `tests/eval-fixtures/extension/rejection-delta/*` - Synthetic extension fixture manifest and minimal repo.

## Decisions Made

- Synthetic observed rows are accepted only from fixture manifests that opt in with `synthetic_observed = true` and declare `area = "extension"`.
- Rejected extension facts remain normal observed fact rows with `status = "rejected"` rather than being converted to errors or hidden from reports.
- The default-vs-extension delta is represented by invariant rows, including a real-surface inactive invariant, so this plan does not create or imply an extension execution mode.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added report-level fact status counters**

- **Found during:** Task 1 GREEN
- **Issue:** The plan required rejected extension facts to be counted separately from accepted/default facts, but existing metric summaries had no way to count observed fact statuses without parsing serialized keys.
- **Fix:** Added `observed_status` to match summaries and `facts_present`, `facts_accepted`, and `facts_rejected` to computed/report metrics.
- **Files modified:** `crates/polint/src/eval/matcher.rs`, `crates/polint/src/eval/metrics.rs`, `crates/polint/src/eval/report.rs`, `crates/polint/src/eval/observed.rs`
- **Verification:** `cargo test -p polint --lib eval_metrics --locked`; `cargo test -p polint --lib eval --locked`
- **Committed in:** `1249fab`

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Required to satisfy the rejected-fact metric requirement; no public API, CLI, SDK, runner, or real extension surface was added.

## Issues Encountered

- During GREEN verification, the new metric unit expected one too many `facts_present` rows because missing expected facts have no observed status. The expectation was corrected before the GREEN commit.

## Verification

- `cargo test -p polint --lib eval_extension_synthetic_rejection_delta_fixture_passes --locked`
- `cargo test -p polint --lib eval_metrics --locked`
- `cargo test -p polint --lib eval_matcher --locked`
- `cargo test -p polint --lib eval_extension_synthetic --locked`
- `cargo test -p polint --lib eval_fixture_manifest_rejects_synthetic_observed_rows_outside_extension_area --locked`
- `cargo test -p polint --lib eval --locked`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`
- `rg -n "eval_extension_synthetic_rejection_delta_fixture_passes|synthetic_observed" crates/polint/src/eval/fixtures.rs`
- `rg -n "extension.synthetic_rejected_fact|status = \"rejected\"|extension.default_vs_extension_delta|extension.real_sink_active|false_positive_trap" tests/eval-fixtures/extension/rejection-delta/expected.polint-eval.toml`
- `rg -n "GraphEdge|Path|RuntimeBudget|Invariant|Diagnostic|Fact" tests/eval-fixtures/extension/rejection-delta/expected.polint-eval.toml crates/polint/src/eval/fixtures.rs`
- `rg -n "ExtensionProvider|extension sink|activate_extension|provider sink|merge API" crates/polint/src/eval tests/eval-fixtures/extension/rejection-delta` - no matches

## Known Stubs

None. Stub scan hits were intentional minimal fixture config: `exclude = []` in `tests/eval-fixtures/extension/rejection-delta/repo/.polint.toml` and an existing inline fixture config in `crates/polint/src/eval/observed.rs`.

## Threat Flags

None. The new synthetic observed-row surface is test-facing, manifest-owned, gated to extension fixtures, uses relative path normalization, and is covered by the plan threat model.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 22-06 can now verify required fixture coverage and public-boundary proof across kernel, provenance, cache, and extension invariants.

## Self-Check: PASSED

- Found summary file: `.planning/phases/22-internal-evaluation-harness-mvp/22-05-SUMMARY.md`
- Found extension fixture files under `tests/eval-fixtures/extension/rejection-delta`.
- Found task commits: `b08f38f` and `1249fab`.

---
*Phase: 22-internal-evaluation-harness-mvp*
*Completed: 2026-05-17*
