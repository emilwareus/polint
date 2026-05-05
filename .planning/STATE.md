---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: MVP
status: archived
stopped_at: quick task 260505-e2y complete
last_updated: "2026-05-05T08:09:44Z"
last_activity: 2026-05-05
progress:
  total_phases: 10
  completed_phases: 10
  total_plans: 35
  completed_plans: 35
  percent: 100
---

# State: exlint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-02)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** v1.0 archived - ready to start the next milestone

## Current Status

- Repository root: `/Users/emilwareus/Development/exlint`.
- Active branch policy: work directly on `main`; do not use GSD worktrees for this project.
- Planning initialized from `docs/INITIAL_PROMPT.md`.
- v1.0 requirements and full roadmap archived under `.planning/milestones/`.
- Live `.planning/REQUIREMENTS.md` is intentionally absent until the next milestone is started.
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
- Phase 6 completed SDK helpers, all eight example rules, CLI fixture proof, snapshots, code review fixes, verification, and security with `threats_open: 0`; see `.planning/phases/06-sdk-and-example-rules/06-VERIFICATION.md` and `.planning/phases/06-sdk-and-example-rules/06-SECURITY.md`.
- Phase 7 completed cache key invalidation, source-free cached parser facts, deterministic Rayon-backed execution, repeated-run output proof, profiling rows, code review, verification, and security with `threats_open: 0`; see `.planning/phases/07-cache-and-performance/07-VERIFICATION.md` and `.planning/phases/07-cache-and-performance/07-SECURITY.md`.
- Phase 8 completed CI output, command contracts, deterministic DOT graph command coverage, code review, verification, and security with `threats_open: 0`; see `.planning/phases/08-ci-output-and-graph-commands/08-VERIFICATION.md` and `.planning/phases/08-ci-output-and-graph-commands/08-SECURITY.md`.
- Phase 9 completed the experimental WIT plugin boundary, structured manifest/Wasmtime validation skeleton, docs, code review, verification, and security with `threats_open: 0`; see `.planning/phases/09-plugin-skeleton/09-VERIFICATION.md` and `.planning/phases/09-plugin-skeleton/09-SECURITY.md`.
- Phase 10 completed README, examples, final CLI smoke tests, release verification, code review, and security with `threats_open: 0`; see `.planning/phases/10-docs-examples-and-release-hardening/10-VERIFICATION.md` and `.planning/phases/10-docs-examples-and-release-hardening/10-SECURITY.md`.
- Next action: start the next milestone with `/gsd-new-milestone`.

## Current Position

Milestone: v1.0 MVP - ARCHIVED
Status: Archived
Plan: Next milestone not started
Last activity: 2026-05-05 - Completed quick task 260505-e2y: Add README try-it workflow and verify it

## Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260502-dql | Remove README note that the repository is named exlint now that the repo will be renamed to polint | 2026-05-02 | a07de50 | [260502-dql-remove-readme-note-that-the-repository-i](./quick/260502-dql-remove-readme-note-that-the-repository-i/) |
| 260502-dto | Improve examples with real minimal linted code, README coverage, and CLI e2e tests | 2026-05-02 | 10ea4a4 | [260502-dto-improve-examples-with-real-minimal-linte](./quick/260502-dto-improve-examples-with-real-minimal-linte/) |
| 260502-ehi | Remove built-in rules and move example policies into examples | 2026-05-02 | 5701608 | [260502-ehi-remove-built-in-rules-and-move-example-r](./quick/260502-ehi-remove-built-in-rules-and-move-example-r/) |
| 260502-qsd | Make examples self-contained with one local rule each | 2026-05-02 | 27caa40 | [260502-qsd-make-examples-self-contained-with-one-lo](./quick/260502-qsd-make-examples-self-contained-with-one-lo/) |
| 260503-a9n | Add clear explanatory comments to self-contained examples | 2026-05-03 | 1dcdc80 | [260503-a9n-add-clear-explanatory-comments-to-self-c](./quick/260503-a9n-add-clear-explanatory-comments-to-self-c/) |
| 260503-adu | Rewrite example READMEs to remove meta-comments and improve user guidance | 2026-05-03 | f0e57ef | [260503-adu-rewrite-example-readmes-to-remove-meta-c](./quick/260503-adu-rewrite-example-readmes-to-remove-meta-c/) |
| 260503-ba9 | Add multi-rule example with one local rule-pack Cargo manifest | 2026-05-03 | 23f5622 | [260503-ba9-add-multi-rule-example-with-one-local-ru](./quick/260503-ba9-add-multi-rule-example-with-one-local-ru/) |
| 260503-l2p | Publish main-branch CLI release assets and install script | 2026-05-03 | 9d07731 | [260503-l2p-publish-main-branch-cli-release-assets-a](./quick/260503-l2p-publish-main-branch-cli-release-assets-a/) |
| 260503-l7c | Update publish workflow actions to Node 24 majors | 2026-05-03 | c556f95 | [260503-l7c-update-publish-workflow-actions-to-node-](./quick/260503-l7c-update-publish-workflow-actions-to-node-/) |
| 260503-leg | Build macOS release targets from the available macOS runner | 2026-05-03 | a7e9d86 | [260503-leg-build-macos-release-targets-from-the-ava](./quick/260503-leg-build-macos-release-targets-from-the-ava/) |
| 260503-lht | Fix release checksum paths for installer | 2026-05-03 | b528398 | [260503-lht-fix-release-checksum-paths-for-installer](./quick/260503-lht-fix-release-checksum-paths-for-installer/) |
| 260503-lwv | Add interactive CLI skill installer for Claude and Codex | 2026-05-03 | ec606b2 | [260503-lwv-add-interactive-cli-skill-installer-for-](./quick/260503-lwv-add-interactive-cli-skill-installer-for-/) |
| 260503-p7f | Add make install command for source installs | 2026-05-03 | 4da0454 | [260503-p7f-add-make-install-command-for-source-inst](./quick/260503-p7f-add-make-install-command-for-source-inst/) |
| 260505-e2y | Add README try-it workflow and verify it | 2026-05-05 | this commit | [260505-e2y-add-readme-try-it-workflow-and-verify-it](./quick/260505-e2y-add-readme-try-it-workflow-and-verify-it/) |

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 | Complete | Rust workspace foundation committed and verified |
| 2 | Complete | CLI, config, discovery, and JSON output first loop verified |
| 3 | Complete | Core facts, diagnostics, deterministic discovery, and review fixes verified |
| 4 | Complete | 4/4 plans complete; parser-backed Go facts verified through unit and CLI integration coverage |
| 5 | Complete | 4/4 plans complete; Oxc-backed TS/JS facts, review fixes, and phase verification passed |
| 6 | Complete | 6/6 plans complete; SDK helpers, example rules, snapshots, review fixes, verification, and security passed |
| 7 | Complete | 4/4 plans complete; cache, deterministic parallelism, profiling, review, verification, and security passed |
| 8 | Complete | 4/4 plans complete; CI output, command contracts, DOT graph commands, verification, and security passed |
| 9 | Complete | 3/3 plans complete; experimental WIT plugin boundary, manifest validation, docs, verification, and security passed |
| 10 | Complete | 4/4 plans complete; README, examples, final CLI smoke tests, release verification, code review, and security passed |

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
- [Phase 06-sdk-and-example-rules]: Used polint_sdk::prelude::* for example rule authoring while keeping run_rules access limited to focused unit tests.
- [Phase 06-sdk-and-example-rules]: Kept denied regex literal handling syntax-level by reporting the available literal text and matched deny token only.
- [Phase 06-sdk-and-example-rules]: Deduped raw-color findings by file, byte range, and literal value so overlapping string and JSX facts produce one diagnostic.
- [Phase 06-sdk-and-example-rules]: Used RuleCtx::branches and RuleCtx::go_tests_for_related_file for Go branch evidence instead of direct AnalysisDb access.
- [Phase 06-sdk-and-example-rules]: Defined the Go test-suite score as 1 + subtests*4 + table_rows*2 + assertions with default max 24.
- [Phase 06-sdk-and-example-rules]: Kept all three Go heuristic diagnostics explicit about heuristic behavior and limited evidence to extracted facts.
- [Phase 06-sdk-and-example-rules]: Used a small fixture expectation test as the Task 1 RED step before creating the missing failing Go test fixture.
- [Phase 06-sdk-and-example-rules]: Kept Phase 6 CLI proof in temp repos with exact profile rule IDs and parsed JSON assertions.
- [Phase 06-sdk-and-example-rules]: Fixed clean branch-obligation suppression through realistic Go test case evidence instead of weakening heuristic rule behavior.
- [Phase 06-sdk-and-example-rules]: Kept snapshot coverage on built_in_rules instead of private rule structs so tests exercise the public registration path.
- [Phase 06-sdk-and-example-rules]: Used synthetic AnalysisDb facts for deterministic snapshot data instead of CLI fixtures, keeping snapshots focused on rule diagnostics.
- [Phase 06-sdk-and-example-rules]: Filtered the all-rule-ID JSON snapshot to the first diagnostic per rule ID so the snapshot proves all eight IDs without duplicating every finding.
- [Phase 08-ci-output-and-graph-commands]: Kept `test-rules` human prelude text out of JSON/SARIF-like stdout.
- [Phase 08-ci-output-and-graph-commands]: Kept CI output described as SARIF-like and avoided full SARIF certification claims.
- [Phase 08-ci-output-and-graph-commands]: Used typed serialization structs for SARIF-like output to avoid feature-dependent JSON field ordering.
- [Phase 08-ci-output-and-graph-commands]: Kept graph commands DOT-only and syntactic, with missing function names returning valid empty DOT.
- [Phase 09]: Plugin WIT boundary exposes typed metadata, capabilities, run, typed diagnostics, and narrow host fact queries. — Matches phase context and avoids full AST/source transfer across the sandbox boundary.
- [Phase 09]: Plugin manifest loading uses typed PluginError variants and manifest-relative component path resolution. — Future plugin CLI surfaces can classify setup failures without parsing free-form error strings.
- [Phase 09]: Plugin docs are explicit that repo-local Wasm rules are experimental and not executed by polint check in v1. — Prevents overclaiming runtime support while preserving the future sandboxed plugin direction.
- [Phase 10]: README is the canonical v1 user guide and documents current behavior without unsupported dynamic loading claims. — Closes FND-03 while preserving project truthfulness constraints.
- [Phase 10]: Top-level examples stay compact and command-oriented. — The README carries the broader guide, while example READMEs should be easy to copy and run.
- [Phase 10]: Runnable examples own minimal local configs. — Each example can be checked in isolation with explicit include globs and profile rule IDs.
- [Phase 10]: Phase 10 smoke tests use checked-in example configs. — This keeps release proof tied to the same examples users copy.
- [Phase 10]: Existing property tests remain the TEST-04 traceability source. — Span, diagnostic sorting, discovery, and cache-key invariants are already covered in the owning crates.
- [Phase 10]: Phase 10 release readiness is command-verified v1 behavior. — The release matrix covers docs inventory, targeted CLI smoke tests, fmt, clippy, and workspace tests without implying publication or future runtime features.
- [Phase 10]: Post-v1 release and runtime capabilities remain future work. — crates.io publishing, release tags, exact Go semantics, dynamic branch coverage, and automatic repo-local Wasm compilation were intentionally not claimed as implemented.

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
| Phase 06-sdk-and-example-rules P05 | 7 min | 3 tasks | 5 files |
| Phase 06-sdk-and-example-rules P06 | 31 min | 2 tasks | 3 files |
| Phase 07 P01 | 10 min | 3 tasks | 5 files |
| Phase 07 P02 | 12 min | 3 tasks | 8 files |
| Phase 07 P03 | 7 min | 3 tasks | 10 files |
| Phase 07 P04 | 6 min | 3 tasks | 5 files |
| Phase 08 P01 | 5 min | 3 tasks | 2 files |
| Phase 08 P02 | 4 min | 3 tasks | 2 files |
| Phase 08 P03 | 5 min | 3 tasks | 2 files |
| Phase 08 P04 | 8 min | 3 tasks | 2 files |
| Phase 09 P01 | 5 min | 3 tasks | 2 files |
| Phase 09 P02 | 4 min | 3 tasks | 3 files |
| Phase 09 P03 | 2 min | 3 tasks | 3 files |
| Phase 10 P01 | 4 min | 3 tasks | 1 files |
| Phase 10 P02 | 3 min | 3 tasks | 7 files |
| Phase 10 P03 | 4 min | 3 tasks | 1 files |
| Phase 10 P04 | 2 min | 3 tasks | 1 files |

## Session

**Last Date:** 2026-05-01T16:17:24.935Z
**Stopped At:** Completed 10-04-PLAN.md
**Resume File:** None

## Important Context For Execution

- Do not fake functionality. If a feature remains heuristic or experimental, label it that way.
- Keep policy rules out of the shipped CLI; each example owns exactly one local rule crate under `examples/*/.polint/rules/`.
- Use deterministic ordering everywhere output can be observed.
- Prefer a smaller complete v1 over broad shallow behavior.
- Keep source and GSD planning changes in `/Users/emilwareus/Development/exlint` on `main`.
- Do not create or use GSD worktrees for this project.
