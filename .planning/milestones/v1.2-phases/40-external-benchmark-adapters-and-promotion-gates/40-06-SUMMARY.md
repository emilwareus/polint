---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 06
subsystem: eval
tags: [rust, evaluation-harness, adaptation, delta-reports, prompt-artifacts]
requires:
  - phase: 40-01
    provides: adaptation schema
  - phase: 40-03
    provides: report output and performance sections
  - phase: 40-05
    provides: supported-language smoke suites and tier runner
provides:
  - default benchmark adaptation-agent prompt
  - adapted-run artifact validation
  - baseline-vs-adapted delta reporting
  - native adaptation-delta fixture
affects: [phase-40, eval, adaptation]
tech-stack:
  added: []
  patterns: [recorded prompt hashes, forbidden-input declarations, case-level adaptation deltas]
key-files:
  created:
    - crates/polint/src/eval/delta.rs
    - research/evaluation-harness/prompts/default-adaptation-agent.md
    - tests/eval-fixtures/extension/adaptation-delta/expected.polint-eval.toml
    - tests/eval-fixtures/extension/adaptation-delta/repo/src/app.ts
  modified:
    - crates/polint/src/eval/adaptation.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/mod.rs
key-decisions:
  - "Adapted runs must record prompt path/hash, budget, allowed and forbidden inputs, changed artifacts or no-change reason, and rule/extension digests."
  - "Delta reports are case-level artifacts, not only aggregate score changes."
  - "Rejected extension facts remain visible even when adapted scanner score improves."
requirements-completed: []
duration: 13 min
completed: 2026-05-26
---

# Phase 40 Plan 06: Agent Adaptation Prompt Artifacts And Delta Reports Summary

**Recorded adaptation prompt, validation gates, and default-vs-adapted delta reporting**

## Performance

- **Duration:** 13 min
- **Started:** 2026-05-26T07:48:14Z
- **Completed:** 2026-05-26T08:01:15Z
- **Tasks:** 4
- **Files modified:** 12

## Accomplishments

- Added `research/evaluation-harness/prompts/default-adaptation-agent.md` with allowed context, forbidden context, adaptation process, deliverables, budget placeholders, and anti-gaming constraints.
- Strengthened adaptation record validation for prompt metadata, positive budget, forbidden-input declaration, changed-file/no-change evidence, and rule/extension digests.
- Added `AdaptationDeltaReport` with case-level changed item keys for new TP, removed FN, removed FP, new FP, unknown changes, graph/path changes, accepted/rejected extension facts, and optional runtime overhead.
- Added `adaptation_delta` to `EvaluationRun` while preserving deterministic normalization and hashing.
- Added a native synthetic adaptation-delta fixture proving recall improvement can coexist with a rejected extension fact.

## Task Commits

1. **Tasks 1-4: Prompt artifact, validation, delta model, and fixture** - `c50a9be` (`feat(40-06)`)

**Plan metadata:** this summary commit.

## Verification

- `rg -n "Do not read benchmark expected labels|Do not hardcode benchmark case IDs|Use the polint skill" research/evaluation-harness/prompts/default-adaptation-agent.md` - passed
- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib eval::adaptation --locked` - passed, 7 tests
- `cargo test -p polint --lib eval::delta --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::report --locked` - passed, 7 tests
- `cargo test -p polint --lib eval_observed --locked` - passed, 13 tests

## User Setup Required

None.

## Next Phase Readiness

Ready for Plan 40-07. Competitor baseline records can now reference polint baseline and agent-adapted runs with prompt-linked delta evidence.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. The prompt explicitly forbids expected-label access and benchmark case-id hardcoding before adaptation.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*
