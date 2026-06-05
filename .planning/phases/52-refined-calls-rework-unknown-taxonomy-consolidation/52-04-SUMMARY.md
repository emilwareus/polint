---
phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
plan: 04
subsystem: cli
tags: [rust, cli, unknowns, schema, docs, eval]

requires:
  - phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
    provides: private unknown taxonomy and solver-projected refined calls
provides:
  - canonical polint inspect unknowns JSON command
  - compatibility rendering for existing polint unknowns --cap JSON
  - schema/docs/skill updates for consolidated unknown rows
  - final Phase 52 roadmap closeout
affects: [public-cli, public-json, generated-skill, eval-fixtures]

tech-stack:
  added: []
  patterns: [canonical inspect command, compatibility alias, optional schema extension]

key-files:
  created:
    - .planning/phases/52-refined-calls-rework-unknown-taxonomy-consolidation/52-04-SUMMARY.md
  modified:
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs
    - docs/schemas/polint-unknowns-v1.json
    - docs/CONSUMER-SETUP.md
    - docs/API-VISIBILITY-PLAN.md
    - crates/polint/src/cli/skill.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - tests/eval-fixtures/data-flow/core/expected.polint-eval.toml
    - tests/eval-fixtures/refined-calls/direct-vs-refined/expected.polint-eval.toml
    - tests/eval-fixtures/refined-calls/extension-model/expected.polint-eval.toml
    - .planning/ROADMAP.md

key-decisions:
  - "`polint inspect unknowns --format json` is the canonical consolidated unknown inspection command."
  - "`polint unknowns --cap ... --format json` remains a shape-compatible cap-filtered compatibility command."
  - "Consolidated rows expose taxonomy metadata only through optional JSON fields."
  - "Eval fixtures now assert solver/direct projection behavior, not retired v1.2 heuristic refined-call flooding."

patterns-established:
  - "Inspect unknowns no-cap output analyzes resolved_imports, symbols, and references, then renders the consolidated taxonomy queue."
  - "Unsupported capability output exits 2 with the full taxonomy row under inspect unknowns and compact compatibility row under top-level unknowns."
  - "Data-flow eval taxonomy recognizes resolved direct-call projections after refined calls move to direct_only/solver-backed provenance."

requirements-completed: [GRAPH-05, TAX-01]

duration: 2h20m
completed: 2026-06-05T10:58:00Z
---

# Phase 52 Plan 04 Summary

**Public inspect unknowns command and Phase 52 closeout are complete**

## Accomplishments

- Added `polint inspect unknowns --format json` with optional `--cap`, positional paths, and `--no-cache`.
- Kept `polint unknowns --cap ... --format json` compatible by stripping new taxonomy metadata from the legacy row renderer.
- Extended `docs/schemas/polint-unknowns-v1.json` with optional `category`, `provider`, `family`, and `source_stable_key`.
- Updated consumer docs, API visibility docs, and generated skill text to prefer `inspect unknowns` for consolidated inspection.
- Updated eval fixtures to match Phase 52 refined-call behavior: direct rows are `direct_only` and resolved; extension models remain audit facts and no longer project to refined-call edges.
- Marked Phase 52 complete in `.planning/ROADMAP.md` with all four plans listed.

## Task Commits

1. Pending final closeout commit.

## Files Created/Modified

- `crates/polint/src/cli/mod.rs` - Adds `InspectCommand::Unknowns`, full taxonomy rendering, and compatibility rendering.
- `crates/polint/tests/cli.rs` - Adds consolidated and cap-filtered inspect unknowns coverage.
- `docs/schemas/polint-unknowns-v1.json` - Adds optional taxonomy metadata fields.
- `docs/CONSUMER-SETUP.md` - Documents canonical inspect unknowns usage.
- `docs/API-VISIBILITY-PLAN.md` - Records public CLI status for inspect/legacy unknowns.
- `crates/polint/src/cli/skill.rs` - Updates generated agent command guidance.
- `crates/polint/src/eval/observed.rs` and eval fixtures - Align evaluation expectations with solver/direct projection closeout.
- `.planning/ROADMAP.md` - Marks Phase 52 complete.

## Deviations from Plan

The final verification surfaced stale eval fixture expectations for retired refined-call heuristic projection paths. The closeout updated those fixtures to assert the intended Phase 52 behavior: no extension-model projection into `polint.refined_calls`, zero changed edges for direct projection, and a resolved direct-call data-flow marker.

## Verification

- `cargo test -p polint --test cli inspect_unknowns`
- `cargo test -p polint --test cli unknowns_json_reports_public_setup_and_resolution_gaps`
- `cargo test -p polint --test cli`
- `cargo test -p polint --test public_surface_leak`
- `cargo test -p polint --lib unknown_taxonomy`
- `cargo test -p polint --lib refined_calls`
- `cargo test -p polint --lib eval::fixtures::eval_native_fixture_runner_tests`
- `cargo test -p polint`
- `cargo clippy -p polint --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## User Setup Required

None.

## Next Phase Readiness

Phase 52 is ready for milestone-level audit or shipping workflow. No follow-up build task is required for this phase.

---
*Phase: 52-refined-calls-rework-unknown-taxonomy-consolidation*
*Completed: 2026-06-05*
