---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 06-04-PLAN.md
last_updated: "2026-04-30T09:54:45.974Z"
last_activity: 2026-04-30
progress:
  total_phases: 10
  completed_phases: 5
  total_plans: 20
  completed_plans: 18
  percent: 90
---

# State: exlint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-04-30)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** Phase 06 — sdk-and-example-rules

## Current Status

- Repository root: `/Users/emilwareus/Development/exlint`.
- Active branch policy: work directly on `main`; do not use GSD worktrees for this project.
- Planning initialized from `docs/INITIAL_PROMPT.md`.
- Requirements and roadmap created.
- Source implementation committed on `main` as `7828215` (`Implement initial polint workspace`).
- Verification passed: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Phase 1 has been closed through GSD plan execution and verification.
- Phase 2 has been closed through GSD plan execution and verification.
- Phase 3 has been closed through GSD plan execution, advisory review fixes, and verification.
- Phase 4 Plan 04-01 completed parser-backed Go package facts and parser diagnostics; see `.planning/phases/04-go-adapter/04-01-SUMMARY.md`.
- Phase 4 Plan 04-02 completed parser-backed Go imports, declarations, calls, test evidence, and complexity; see `.planning/phases/04-go-adapter/04-02-SUMMARY.md`.
- Phase 4 Plan 04-03 completed parser-backed Go branch obligations, stable branch fingerprints, and conservative error-path heuristics; see `.planning/phases/04-go-adapter/04-03-SUMMARY.md`.
- Phase 4 Plan 04-04 completed expanded Go fixtures, CLI integration coverage, and workspace verification; see `.planning/phases/04-go-adapter/04-04-SUMMARY.md`.
- Phase 4 verification passed with no gaps; see `.planning/phases/04-go-adapter/04-VERIFICATION.md`.
- Phase 5 Plan 05-01 completed the Oxc parser foundation and controlled `parser/ts` diagnostics; see `.planning/phases/05-typescript-adapter/05-01-SUMMARY.md`.
- Phase 5 Plan 05-02 completed parser-backed TS imports, exports, functions, classes, methods, component heuristics, and calls; see `.planning/phases/05-typescript-adapter/05-02-SUMMARY.md`.
- Phase 5 Plan 05-03 completed parser-backed TS literals, JSX attributes, complexity, and import graph proof; see `.planning/phases/05-typescript-adapter/05-03-SUMMARY.md`.
- Phase 5 Plan 05-04 completed expanded TS fixtures, CLI integration tests, and full workspace verification; see `.planning/phases/05-typescript-adapter/05-04-SUMMARY.md`.
- Phase 5 code review passed clean after review fixes; see `.planning/phases/05-typescript-adapter/05-REVIEW.md` and `.planning/phases/05-typescript-adapter/05-REVIEW-FIX.md`.
- Phase 5 verification passed with no gaps; see `.planning/phases/05-typescript-adapter/05-VERIFICATION.md`.
- Phase 5 security gate passed with `threats_open: 0`; see `.planning/phases/05-typescript-adapter/05-SECURITY.md`.
- Next action: discuss Phase 6 on `main`.

## Current Position

Phase: 06 (sdk-and-example-rules) — EXECUTING
Status: Ready to execute
Plan: 5 of 6
Last activity: 2026-04-30

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 | Complete | Rust workspace foundation committed and verified |
| 2 | Complete | CLI, config, discovery, and JSON output first loop verified |
| 3 | Complete | Core facts, diagnostics, deterministic discovery, and review fixes verified |
| 4 | Complete | 4/4 plans complete; parser-backed Go facts verified through unit and CLI integration coverage |
| 5 | Complete | 4/4 plans complete; Oxc-backed TS/JS facts, review fixes, and phase verification passed |
| 6 | Not Started | SDK and requested example rules have working initial implementation; GSD phase execution not started |
| 7 | In Progress | Cache crate exists; deeper parse/fact persistence remains |
| 8 | In Progress | SARIF-like output, exit codes, profile-rules, explain, and graph commands exist |
| 9 | In Progress | WIT and plugin host skeleton exist |
| 10 | In Progress | README, examples, fixtures, and tests exist; more snapshots/hardening remain |

## Decisions Made

- [Phase 04-go-adapter]: Added only the narrow PackageFact core contract needed for Go package names.
- [Phase 04-go-adapter]: Kept Go parser diagnostics local to polint-go with stable parser/go messages for malformed source.
- [Phase 04-go-adapter]: Kept existing import/function extraction in place while moving package extraction to tree-sitter nodes for this foundation plan.
- [Phase 04-go-adapter]: Stored explicit Go import aliases in ImportFact.package while leaving unaliased imports as None.
- [Phase 04-go-adapter]: Named parser-backed Go methods as Receiver.Method with pointer/package receiver cleanup.
- [Phase 04-go-adapter]: Required _test.go plus practical testing signatures before creating Go TestFact records.
- [Phase 04-go-adapter]: Extracted Go branch obligations from parser nodes inside function and method bodies instead of line scanning.
- [Phase 04-go-adapter]: Computed branch fingerprints from stable source identity and excluded BranchId, FunctionId, and traversal counters.
- [Phase 04-go-adapter]: Kept Go error-path detection explicitly syntax-only and heuristic, without semantic type analysis or exact coverage claims.
- [Phase 04-go-adapter]: Kept graph command and DOT coverage out of Plan 04-04; Go import facts are proven through the import-boundary CLI rule path.
- [Phase 04-go-adapter]: Treated the TDD-marked CLI task as coverage-only after the new tests passed against the existing Phase 4 implementation.
- [Phase 04-go-adapter]: Recorded the verification-only task with an empty commit because all checks passed without producing file changes.
- [Phase 05-typescript-adapter]: Kept parser/ts diagnostics local to polint-ts and used the stable TS/JS parser syntax-error prefix.
- [Phase 05-typescript-adapter]: Parsed TS-family files from SourceFile.source as borrowed Arc-backed text instead of cloning full source strings.
- [Phase 05-typescript-adapter]: Introduced narrow Oxc helper boundaries while preserving lexical extraction for fact families not yet AST-backed.
- [Phase 05-typescript-adapter]: Added a narrow TsClassFact public contract with no class IDs, inheritance graph, resolver, or type information.
- [Phase 05-typescript-adapter]: Kept TS/JS module specifiers syntactic and parser-backed; no production Node or TypeScript resolution was added.
- [Phase 05-typescript-adapter]: Used Oxc module records only as a parser-backed fallback to preserve best-effort imports after unrecoverable parser errors.
- [Phase 05-typescript-adapter]: Recorded dynamic template literals as static quasi facts only instead of synthetic exact combined values.
- [Phase 05-typescript-adapter]: Computed TS/JS complexity from Oxc AST control-flow nodes rather than comments or string contents.
- [Phase 05-typescript-adapter]: Added polint-graph as a polint-ts dev-dependency solely for import graph unit proof.
- [Phase 05-typescript-adapter]: Proved TS parser diagnostics and TS rule consumption through parsed CLI JSON integration tests.
- [Phase 06-sdk-and-example-rules]: Kept the core Rule and RuleCtx contract additive while exposing new borrowed helper methods.
- [Phase 06-sdk-and-example-rules]: Returned Vec<&TestFact> only for go_tests_for_related_file because it combines same-file and companion borrowed references.
- [Phase 06-sdk-and-example-rules]: Kept polint new-rule scaffolds honest: SDK helper examples only, no dynamic loading claims.
- [Phase 06-sdk-and-example-rules]: Kept literal allow-list support as a narrow additive config field separate from allow_files.
- [Phase 06-sdk-and-example-rules]: Excluded Go import path string nodes from general string literal facts so ImportFact remains the import source of truth.
- [Phase 06-sdk-and-example-rules]: Represented TS/JS regex literals as slash-delimited source syntax only, preserving flags without evaluating regex semantics.
- [Phase 06-sdk-and-example-rules]: Used polint_sdk::prelude::* for production built-in rule authoring while keeping run_rules access limited to focused unit tests.
- [Phase 06-sdk-and-example-rules]: Kept denied regex literal handling syntax-level by reporting the available literal text and matched deny token only.
- [Phase 06-sdk-and-example-rules]: Deduped raw-color findings by file, byte range, and literal value so overlapping string and JSX facts produce one diagnostic.
- [Phase 06-sdk-and-example-rules]: Used RuleCtx::branches and RuleCtx::go_tests_for_related_file for Go branch evidence instead of direct AnalysisDb access.
- [Phase 06-sdk-and-example-rules]: Defined the Go test-suite score as 1 + subtests*4 + table_rows*2 + assertions with default max 24.
- [Phase 06-sdk-and-example-rules]: Kept all three Go heuristic diagnostics explicit about heuristic behavior and limited evidence to extracted facts.

## Performance Metrics

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 04-go-adapter P01 | 8min | 2 tasks | 2 files |
| Phase 04-go-adapter P02 | 9min | 2 tasks | 3 files |
| Phase 04-go-adapter P03 | 9min | 2 tasks | 2 files |
| Phase 04-go-adapter P04 | 6min | 3 tasks | 6 files |
| Phase 05-typescript-adapter P01 | 10min | 2 tasks | 1 files |
| Phase 05-typescript-adapter P02 | 13min | 2 tasks | 2 files |
| Phase 05-typescript-adapter P03 | interrupted/resumed | 2 tasks | 3 files |
| Phase 05-typescript-adapter P04 | 10min | 3 tasks | 5 files |
| Phase 06-sdk-and-example-rules P01 | 7 min | 3 tasks | 4 files |
| Phase 06-sdk-and-example-rules P02 | 6 min | 3 tasks | 5 files |
| Phase 06-sdk-and-example-rules P03 | 10 min | 3 tasks | 1 files |
| Phase 06-sdk-and-example-rules P04 | 5 min | 3 tasks | 1 files |

## Session

**Last Date:** 2026-04-30T09:54:45.972Z
**Stopped At:** Completed 06-04-PLAN.md
**Resume File:** None

## Important Context For Execution

- Do not fake functionality. If a feature remains heuristic or experimental, label it that way.
- Keep built-in rules as SDK examples, not a comprehensive ruleset.
- Use deterministic ordering everywhere output can be observed.
- Prefer a smaller complete v1 over broad shallow behavior.
- Keep source and GSD planning changes in `/Users/emilwareus/Development/exlint` on `main`.
- Do not create or use GSD worktrees for this project.
