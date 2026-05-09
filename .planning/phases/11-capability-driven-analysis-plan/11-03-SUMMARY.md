---
phase: 11-capability-driven-analysis-plan
plan: "03"
subsystem: core-analysis
tags: [rust, cli, analysis-plan, local-rule-host, capabilities, docs]

requires:
  - phase: 11-02
    provides: RulePlanInputs, real child-host plan construction, and plan-hash cache identity
provides:
  - child-host `polint-local-rules explain plan` with human and JSON output
  - parent `polint explain plan` with empty-plan output and local-rule-host delegation
  - CLI proof for deterministic explain JSON, unsupported reserved capabilities, and capability-sensitive cache entries
  - public docs for capability plan output and Phase 11 capability support boundaries
affects: [11-03, capability-planning, runner, cli, docs, examples]

tech-stack:
  added: []
  patterns:
    - typed serde report structs for deterministic machine output
    - parent-to-child local rule host delegation through explicit ProcessCommand args
    - temp-repo tests that compile external rule hosts against public SDK and runner APIs

key-files:
  created:
    - docs/facts/capability-plans.md
    - .planning/phases/11-capability-driven-analysis-plan/11-03-SUMMARY.md
  modified:
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs
    - docs/facts/README.md
    - examples/go-test-quality/.polint/rules/src/go_test_quality.rs

key-decisions:
  - "Use `ExplainPlanReport` as a crate-private typed serde boundary shared by child and parent explain-plan commands."
  - "Keep `polint explain plan --format json` stdout as the child report itself for a single local rule host; no human prelude is emitted."
  - "Keep current Go test evidence on the supported `go_tests` capability; `test_suite_metrics` remains reserved for normalized future metrics."

patterns-established:
  - "Child explain-plan construction follows `RulePlanInputs::collect` -> `rule_options_from_config` -> `AnalysisPlan::from_inputs` and never loads source files."
  - "Parent explain-plan delegation invokes `cargo run --quiet --manifest-path ... -- explain plan --format json` through `ProcessCommand` args and parses typed JSON."
  - "CLI tests for rule-authoring behavior generate `.polint/rules` temp repos that import only `polint::sdk::prelude::*` and register through `polint::runner::run_cli`."

requirements-completed: [PLAN-01, PLAN-03, PLAN-04]

duration: 22m 23s
completed: 2026-05-09
---

# Phase 11 Plan 03: Explain Plan CLI Summary

**Deterministic `polint explain plan` output with local-rule-host delegation, unsupported capability diagnostics, and cache invalidation proof**

## Performance

- **Duration:** 22m 23s
- **Started:** 2026-05-09T08:11:58Z
- **Completed:** 2026-05-09T08:34:21Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added crate-private `ExplainPlanReport`, `ExplainPlanRule`, `ExplainPlanCapability`, and `ExplainPlanSetupCheck` serde structs plus human rendering.
- Added child and parent `explain plan` commands, including no-local-rule empty output and local rule host delegation with typed JSON parsing.
- Added temp-repo CLI proof for external rule hosts, deterministic explain JSON, unsupported `cfg` diagnostics, and plan-sensitive cache entries.
- Documented `polint explain plan --format json`, JSON fields, supported Phase 11 capabilities, and unsupported reserved capability names.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: explain report tests** - `ef19157` (test)
2. **Task 1 GREEN: child explain plan output** - `dd4c4f7` (feat)
3. **Task 2 RED: parent empty explain plan test** - `d1923a4` (test)
4. **Task 2 GREEN: parent explain plan command** - `7c15137` (feat)
5. **Task 3 proof and docs** - `0e2c454` (test)
6. **Gate fix: clippy capability filtering** - `2e43a3a` (fix)
7. **Gate fix: go-test-quality capability declaration** - `74b5f4e` (fix)

Plan metadata is committed separately after state updates.

## Files Created/Modified

- `crates/polint/src/analysis_plan.rs` - Added explain report structs, human renderer, JSON status mapping, and report unit tests.
- `crates/polint/src/runner/mod.rs` - Added `polint-local-rules explain plan` and safe child-host plan construction without file loading.
- `crates/polint/src/cli/mod.rs` - Added parent `polint explain plan`, no-host empty output, local-host JSON delegation, and typed child-output parsing.
- `crates/polint/tests/cli.rs` - Added temp-repo local-rule-host tests for explain output, unsupported capability diagnostics, determinism, and cache invalidation.
- `docs/facts/capability-plans.md` - Added public explain-plan and capability support documentation.
- `docs/facts/README.md` - Linked the capability plan reference.
- `examples/go-test-quality/.polint/rules/src/go_test_quality.rs` - Declared only the supported `go_tests` capability.

## Decisions Made

- `ExplainPlanReport` stays crate-private: it is an internal serialization contract, not a public SDK surface.
- Parent `--format json` preserves machine stdout by parsing child JSON and reserializing the report directly.
- Multiple child-host reports are combined deterministically if config ever exposes more than one manifest, while the single-host path preserves the child report unchanged.
- The Go test-quality example should use `go_tests`; `test_suite_metrics` remains unsupported/reserved until normalized metrics exist.

## Verification

- `cargo test -p polint --lib analysis_plan_explain_report --locked`
- `cargo test -p polint --test cli explain_plan_no_rules_outputs_empty_json_without_parsing_sources --locked`
- `cargo test -p polint --test cli explain_plan --locked`
- `cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed clippy failure in capability filtering**
- **Found during:** Phase gate after Task 3
- **Issue:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` rejected `filter_map(... bool::then(...))` in `requested_capabilities`.
- **Fix:** Replaced it with `filter` plus `map` while preserving capability ordering.
- **Files modified:** `crates/polint/src/analysis_plan.rs`
- **Verification:** `cargo test -p polint --lib analysis_plan_explain_report --locked`; clippy gate passed.
- **Committed in:** `2e43a3a`

**2. [Rule 2 - Missing Critical] Kept example capability declaration truthful**
- **Found during:** Full workspace test gate
- **Issue:** The `go-test-quality` example requested reserved `test_suite_metrics`, which now correctly emits `polint/capability`; the example only consumes current Go test evidence.
- **Fix:** Changed the example rule to declare `go_tests` only.
- **Files modified:** `examples/go-test-quality/.polint/rules/src/go_test_quality.rs`
- **Verification:** `cargo test -p polint --test cli checked_in_examples_are_runnable_cli_fixtures --locked`; full workspace test gate passed.
- **Committed in:** `74b5f4e`

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 missing critical)
**Impact on plan:** Both fixes were required for Phase 11 truthfulness and the mandated verification gate. No additional capability families were implemented.

## Issues Encountered

- Task 3's new proof tests passed immediately because Tasks 1 and 2 had already implemented the required behavior. The tests were still committed as the external-consumer proof for the plan.
- Local-rule-host integration tests are slow because they compile temporary rule crates through Cargo; this is expected for end-to-end coverage.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Stub-pattern scan found only intentional test fixture strings such as `TODO`, empty profile rule arrays, and empty config excludes.

## Next Phase Readiness

Phase 12 can build on a deterministic explain-plan surface and explicit unsupported-capability handling. Future capability phases should update `docs/facts/capability-plans.md` as capabilities move from reserved to supported.

## Self-Check: PASSED

- Found `.planning/phases/11-capability-driven-analysis-plan/11-03-SUMMARY.md`.
- Found key modified files for analysis plan, runner, CLI, CLI tests, fact docs, and the Go test-quality example.
- Found task commits `ef19157`, `dd4c4f7`, `d1923a4`, `7c15137`, `0e2c454`, `2e43a3a`, and `74b5f4e`.

---
*Phase: 11-capability-driven-analysis-plan*
*Completed: 2026-05-09*
