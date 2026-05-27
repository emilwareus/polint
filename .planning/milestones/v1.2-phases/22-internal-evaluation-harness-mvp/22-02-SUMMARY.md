---
phase: 22-internal-evaluation-harness-mvp
plan: "02"
subsystem: evaluation-harness
tags: [rust, eval, matcher, metrics, deterministic-json, internal-api]

requires:
  - phase: 22-internal-evaluation-harness-mvp
    provides: Crate-private canonical evaluation model and deterministic report hashing from Plan 22-01
provides:
  - Crate-private generic matcher for normalized diagnostics, facts, graph edges, paths, invariants, and runtime budgets
  - Unified metric aggregation for matcher outcomes, graph/path uncertainty, traps, unknowns, and runtime budget pass/fail
  - Extended deterministic report summaries with typed match outcomes and metric fields
affects: [22-03-native-fixture-runner, evaluation-harness, promotion-gates]

tech-stack:
  added: []
  patterns:
    - crate-private matcher and metrics modules with no SDK, runner, crate-root public, or CLI surface
    - deterministic literal matcher keys over normalized expected/observed rows
    - ratio helpers return Option<f64> for zero denominators

key-files:
  created:
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/metrics.rs
    - .planning/phases/22-internal-evaluation-harness-mvp/22-02-SUMMARY.md
  modified:
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/report.rs

key-decisions:
  - "Keep matcher and metric logic crate-private and pure over normalized in-memory eval rows."
  - "Represent matcher outcomes as typed report data instead of outcome strings so metrics can aggregate deterministically."
  - "Clear observed runtime durations from match summaries before deterministic output hashing, preserving pass/fail semantics without wall-clock hash input."
  - "Extend the existing MetricSummary report type from Plan 22-01 instead of adding a duplicate metric report shape."

patterns-established:
  - "Matcher rows carry expected_key and observed_key so metrics can count expected, observed, and unconfirmed graph/path rows without re-reading original inputs."
  - "Runtime budget outcomes are derived only from ObservedRuntimeBudget.budget_passed, not recomputed from observed_runtime_ms."

requirements-completed: [SAE-FND-03]

duration: 15 min
completed: 2026-05-17
---

# Phase 22 Plan 02: Generic Matchers and Metrics Summary

**Deterministic eval matching and unified accuracy/cost metrics over normalized internal harness rows**

## Performance

- **Duration:** 15 min
- **Started:** 2026-05-17T16:41:32Z
- **Completed:** 2026-05-17T16:56:01Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `eval::matcher` with typed `MatchOutcome`, `MatcherConfig`, stable keys, line-tolerant diagnostic matching, forbidden assertions, false-positive traps, unknown/setup/unsupported preservation, partial graph/path unconfirmed handling, and runtime budget pass/fail outcomes.
- Added `eval::metrics` with `ComputedMetrics`, confusion-matrix counts, graph/path counts, trap/forbidden/unknown counts, runtime budget counters, precision/recall/F1/F2/F3/FPR ratios, and zero-denominator `None` behavior.
- Extended `report::MatchSummary` and `report::MetricSummary` so report data can carry typed outcomes and computed metrics deterministically.

## Task Commits

Each TDD step was committed atomically:

1. **Task 1 RED: Add failing matcher behavior tests** - `f52c796` (test)
2. **Task 1 GREEN: Implement deterministic eval matcher** - `6f32a5c` (feat)
3. **Task 2 RED: Add failing metric aggregation tests** - `b5c5b3b` (test)
4. **Task 2 GREEN: Implement eval metric aggregation** - `e664f23` (feat)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/eval/matcher.rs` - Generic matcher engine and focused matcher behavior tests.
- `crates/polint/src/eval/metrics.rs` - Unified metric aggregation and metric/report conversion tests.
- `crates/polint/src/eval/mod.rs` - Registers the crate-private matcher and metrics modules.
- `crates/polint/src/eval/model.rs` - Adds first-class false-positive trap flags to expected diagnostics and facts.
- `crates/polint/src/eval/report.rs` - Extends match and metric summaries while preserving deterministic report ordering and hash behavior.

## Decisions Made

- Kept all matcher and metric code crate-private under `eval`; no public SDK, runner, crate-root public, or CLI contract was added.
- Chose typed `MatchOutcome` and `MatchItemKind` values in `MatchSummary` so metrics do not parse strings.
- Counted `TrapHit` and `ForbiddenHit` separately from generic false positives, preserving explicit trap/forbidden evidence.
- Kept exact observed runtime durations out of deterministic output hashes, including the new match-summary duration field.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The Task 2 acceptance grep for benchmark/baseline gates matched existing local test variable names in `report.rs`; those locals were renamed to `reference`/`reference_hash` so the literal scope gate proves no benchmark tier or baseline implementation was added.

## Verification

- `cargo test -p polint --lib eval_matcher --locked`
- `cargo test -p polint --lib eval_metrics --locked`
- `cargo test -p polint --lib eval_report --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo test -p polint --lib eval --locked`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None. Stub scan found no TODO/FIXME markers, placeholder text, or hardcoded empty values that flow to user-visible output.

## Threat Flags

None. The new matcher and metrics layers remain crate-private, compare normalized in-memory data only, introduce no network/auth/file execution surface, and keep exact observed runtime durations out of deterministic hashes.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 22-03 can build native fixture execution on top of the canonical model, deterministic report hashing, typed matcher outcomes, and unified metrics delivered by Plans 22-01 and 22-02.

## Self-Check: PASSED

- Found created files: `crates/polint/src/eval/matcher.rs`, `crates/polint/src/eval/metrics.rs`, and this summary.
- Found task commits: `f52c796`, `6f32a5c`, `b5c5b3b`, and `e664f23`.

---
*Phase: 22-internal-evaluation-harness-mvp*
*Completed: 2026-05-17*
