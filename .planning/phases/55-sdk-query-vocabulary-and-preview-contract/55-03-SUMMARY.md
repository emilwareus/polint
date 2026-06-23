---
phase: 55-sdk-query-vocabulary-and-preview-contract
plan: 03
subsystem: public-contract
tags: [sdk, docs, public-surface, external-tests]
key-files:
  created:
    - docs/facts/calls.md
    - docs/facts/control-flow.md
    - docs/facts/events.md
  modified:
    - .agents/skills/polint/SKILL.md
    - .claude/skills/polint/SKILL.md
    - crates/polint/tests/cli.rs
    - crates/polint/tests/public_surface_leak.rs
    - docs/API-VISIBILITY-PLAN.md
    - docs/facts/README.md
    - docs/facts/capability-plans.md
    - docs/facts/data-flow.md
    - tests/fixtures/public-surface-leak-probe/src/lib.rs
requirements-completed: [API-01, API-02, API-03, API-04, API-05, API-06]
duration: 28 min
completed: 2026-06-20
---

# Phase 55 Plan 03: Public Docs External Tests And Boundary Proof Summary

External rule syntax, public docs, and leak gates now prove the Phase 55 preview policy-query contract without exposing raw analysis internals.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 718f0625 | Added temp-repo preview syntax/fail-closed coverage, reserved raw capability proof, Phase 55 docs, API promotion record, and public-surface leak-gate witnesses. |

## Verification

- `cargo test -p polint --test cli phase55_preview_rule_syntax_compiles_and_fails_closed --locked` PASS
- `cargo test -p polint --test cli reserved_cfg_and_call_graph_remain_unsupported --locked` PASS
- `cargo test -p polint --test cli facts_list_json_is_stable_and_public_only --locked` PASS
- `cargo test -p polint --test public_surface_leak --locked` PASS
- `cargo fmt --all --check` PASS
- `cargo test -p polint-macros --locked` PASS
- `cargo check -p polint --locked` PASS
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` PASS
- `rg -n "Events<'_>|Calls<'_>|ControlFlow<'_>|FlowQuery|SourcePattern|polint/capability|fail closed|preview" docs/API-VISIBILITY-PLAN.md docs/facts` PASS
- `rg -n "use (:+)?polint::(core|analysis|analysis_kernel|go|ts|graph|eval|cache|config)|polint::core|polint::analysis|polint::graph|AnalysisDb" docs/facts docs/API-VISIBILITY-PLAN.md README.md .agents/skills/polint/SKILL.md .claude/skills/polint/SKILL.md` PASS; matches were only negative/internal-boundary wording.
- `git diff --check` PASS

## Deviations from Plan

- Updated `.agents/skills/polint/SKILL.md` and `.claude/skills/polint/SKILL.md` in addition to the planned docs so agent rule-authoring guidance no longer describes policy-level `DataFlow<'_>` as only reserved.

## Issues Encountered

- The first run of `phase55_preview_rule_syntax_compiles_and_fails_closed` passed compilation and fail-closed diagnostics but failed the manifest assertion because `inspect rule` uses `rule_id`, not `id`. The test was corrected and rerun successfully.

## Next Phase Readiness

Phase 56 can implement provider-backed `Events<'_>` and `Calls<'_>` behavior behind the already documented and fail-closed preview capability names. Phase 55 intentionally proves compile/manifest/capability/public-boundary behavior only; full query-result semantics remain deferred to Phases 56-59.

## Self-Check: PASSED
