---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: Capability Fulfillment
status: executing
stopped_at: Phase 13 context gathered
last_updated: "2026-05-12T20:06:25.100Z"
last_activity: 2026-05-12 -- Phase 13 execution started
progress:
  total_phases: 9
  completed_phases: 2
  total_plans: 14
  completed_plans: 8
  percent: 57
---

# State: polint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-08)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** Phase 13 — symbols-and-references

## Current Status

- **GitHub:** `emilwareus/polint` (public repository name).
- Local checkout (this machine): `/Users/emilwareus/Development/exlint`.
- Active branch policy: work directly on `main`; do not use GSD worktrees for this project.
- Planning initialized from `docs/INITIAL_PROMPT.md`.
- v1.0 requirements and full roadmap archived under `.planning/milestones/`.
- Live `.planning/REQUIREMENTS.md` defines v1.1 Capability Fulfillment requirements.
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
- Phase 9 completed README, examples, final CLI smoke tests, release verification, code review, and security with `threats_open: 0`; see `.planning/phases/10-docs-examples-and-release-hardening/10-VERIFICATION.md` and `.planning/phases/10-docs-examples-and-release-hardening/10-SECURITY.md`.
- Capability Fulfillment research lives in `docs/CAPABILITY-FULFILLMENT-RESEARCH.md`.
- Human-readable capability roadmap lives in `docs/roadmap/00_ROADMAP.md`.
- v1.1 requirements are defined in `.planning/REQUIREMENTS.md`.
- v1.1 roadmap is defined in `.planning/ROADMAP.md`.
- Phase 12 shipped in PR #10: https://github.com/emilwareus/polint/pull/10.
- Next action: discuss and plan Phase 13, Symbols and References.

## Current Position

Milestone: v1.1 Capability Fulfillment
Status: Executing Phase 13
Phase: 13 (symbols-and-references) — EXECUTING
Plan: 1 of 6
Last activity: 2026-05-12 -- Phase 13 execution started

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
| 260505-e2y | Add README try-it workflow and verify it | 2026-05-05 | 763b9b1 | [260505-e2y-add-readme-try-it-workflow-and-verify-it](./quick/260505-e2y-add-readme-try-it-workflow-and-verify-it/) |
| 260505-ffu | Make polint check run repo-local rule hosts directly | 2026-05-05 | uncommitted | [260505-ffu-make-polint-check-run-repo-local-rule-ho](./quick/260505-ffu-make-polint-check-run-repo-local-rule-ho/) |
| 260506-iuu | Fix staged review findings for agent-quality changes | 2026-05-06 | uncommitted | [260506-iuu-fix-staged-review-findings-for-agent-qua](./quick/260506-iuu-fix-staged-review-findings-for-agent-qua/) |
| 260507-rap | Rule authoring platform hardening | 2026-05-07 | uncommitted | [260507-rap-rule-authoring-platform-hardening](./quick/260507-rap-rule-authoring-platform-hardening/) |
| 260509-h5x | Fix capability roadmap docs and add realistic CLI coverage for explain plan | 2026-05-09 | uncommitted | [260509-h5x-fix-capability-roadmap-docs-and-add-real](./quick/260509-h5x-fix-capability-roadmap-docs-and-add-real/) |
| 260509-ibk | Implement static capability derivation rule authoring API | 2026-05-09 | uncommitted | [260509-ibk-implement-static-capability-derivation-r](./quick/260509-ibk-implement-static-capability-derivation-r/) |
| 260510-dbv | Tighten public CLI surface and remove internal debug commands | 2026-05-10 | uncommitted | [260510-dbv-tighten-public-cli-surface-and-remove-in](./quick/260510-dbv-tighten-public-cli-surface-and-remove-in/) |
| 260510-dzr | Implement reusable derived metric signals for rules | 2026-05-10 | uncommitted | [260510-dzr-implement-reusable-derived-metric-signal](./quick/260510-dzr-implement-reusable-derived-metric-signal/) |
| 260510-eur | Prompt before overwriting existing installed polint skills | 2026-05-10 | uncommitted | [260510-eur-prompt-before-overwriting-existing-insta](./quick/260510-eur-prompt-before-overwriting-existing-insta/) |
| 260511-gyu | Add compact YAML baseline and central ignore ratchet workflow | 2026-05-11 | uncommitted | [260511-gyu-add-compact-yaml-baseline-and-central-ig](./quick/260511-gyu-add-compact-yaml-baseline-and-central-ig/) |
| 260511-i7m | Make the baseline file live only at .polint/baseline.yaml and remove user-selectable baseline paths | 2026-05-11 | uncommitted | [260511-i7m-make-the-baseline-file-live-only-at-poli](./quick/260511-i7m-make-the-baseline-file-live-only-at-poli/) |
| 260512-aop | Fix review findings for baseline and module relationships | 2026-05-12 | 30098c6 | [260512-aop-fix-review-findings-for-baseline-and-mod](./quick/260512-aop-fix-review-findings-for-baseline-and-mod/) |
| 260512-yml | Replace unsound serde_yml dependency | 2026-05-12 | c2f678e | [260512-yml-replace-unsound-serde-yml-dependency](./quick/260512-yml-replace-unsound-serde-yml-dependency/) |
| 260512-h4g | Fix publish script to be idempotent after partial crates.io publish | 2026-05-12 | 0729a6b | [260512-h4g-fix-publish-script-to-be-idempotent-afte](./quick/260512-h4g-fix-publish-script-to-be-idempotent-afte/) |
| 260512-tga | Research lifecycle extensibility architecture for Phase 13 scan lifecycle and update research doc | 2026-05-12 | uncommitted | [260512-tga-research-lifecycle-extensibility-archite](./quick/260512-tga-research-lifecycle-extensibility-archite/) |

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 11 | Complete | Capability-driven AnalysisPlan; requirements PLAN-01 through PLAN-04 |
| 12 | Complete | Resolved imports and module graph; requirements MOD-01 through MOD-04; verification passed |
| 13 | Pending | Symbols and references; requirements SYM-01 through SYM-04 |
| 14 | Pending | Direct and resolved call graph facts; requirements CALL-01 through CALL-04 |
| 15 | Pending | CFG facts for Go and TS/JS; requirements CFG-01 through CFG-04 |
| 16 | Pending | Coverage facts import; requirements COV-01 through COV-04 |
| 17 | Pending | Test suite metrics; requirements TEST-01 through TEST-04 |
| 18 | Pending | Python adapter with explicit initial capability tier; requirements PY-01 through PY-04 |
| 19 | Pending | Java adapter with setup-aware initial capability tier; requirements JAVA-01 through JAVA-04 |

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
- [Phase 09]: README is the canonical v1 user guide and documents current behavior without unsupported dynamic loading claims. — Closes FND-03 while preserving project truthfulness constraints.
- [Phase 09]: Top-level examples stay compact and command-oriented. — The README carries the broader guide, while example READMEs should be easy to copy and run.
- [Phase 09]: Runnable examples own minimal local configs. — Each example can be checked in isolation with explicit include globs and profile rule IDs.
- [Phase 09]: Phase 9 smoke tests use checked-in example configs. — This keeps release proof tied to the same examples users copy.
- [Phase 09]: Existing property tests remain the TEST-04 traceability source. — Span, diagnostic sorting, discovery, and cache-key invariants are already covered in the owning crates.
- [Phase 09]: Phase 9 release readiness is command-verified v1 behavior. — The release matrix covers docs inventory, targeted CLI smoke tests, fmt, clippy, and workspace tests without implying publication or future runtime features.
- [Phase 09]: Post-v1 release and runtime capabilities remain future work. — crates.io publishing, release tags, exact Go semantics, and dynamic branch coverage were intentionally not claimed as implemented.
- [Phase 11-capability-driven-analysis-plan]: Keep AnalysisPlan crate-private and expose only CapabilitySupport, CapabilitySupportStatus, and CapabilitySupportView through the SDK prelude.
- [Phase 11-capability-driven-analysis-plan]: Treat cfg, call_graph, coverage_facts, and test_suite_metrics as unsupported reserved capabilities in Phase 11.
- [Phase 11-capability-driven-analysis-plan]: Use deterministic length-prefixed strings plus stable_hash for the plan digest instead of serde JSON output.
- [Phase 11-capability-driven-analysis-plan]: Use RulePlanInputs as the single panic-contained rule metadata/capability snapshot for options, rule digest, and plan construction.
- [Phase 11-capability-driven-analysis-plan]: Keep AnalysisPlan crate-private; bench-facing analyze_with_options wrappers construct AnalysisPlan::empty() internally.
- [Phase 11-capability-driven-analysis-plan]: Include plan_hash in CacheKey::stable_id between rule_hash and cache version.
- [Phase 11-capability-driven-analysis-plan]: Use an empty AnalysisPlan in parent CLI paths where no local rule host is loaded.
- [Phase 11-capability-driven-analysis-plan]: Use ExplainPlanReport as a crate-private typed serde boundary shared by child and parent explain-plan commands.
- [Phase 11-capability-driven-analysis-plan]: Keep polint explain plan --format json stdout as the child report itself for a single local rule host; no human prelude is emitted.
- [Phase 11-capability-driven-analysis-plan]: Keep current Go test evidence on the supported go_tests capability; test_suite_metrics remains reserved for normalized future metrics.
- [Static capability derivation]: Normal rule authors use `#[polint::rule]` functions with typed fact-view parameters; capabilities are generated from those parameter types instead of handwritten declarations.
- [Static capability derivation]: `RuleCtx` is the diagnostics/options/path/support surface. Broad fact access belongs in typed SDK fact views, not the normal context API.
- [Static capability derivation]: `Rule` is an opaque value, not a public trait. Do not preserve manual `impl Rule` compatibility paths during beta; update examples, scaffolds, and tests to the typed macro path instead.
- [Phase 12-resolved-imports-and-module-relationships]: Resolved imports and module graph are known capabilities but stay Unsupported until Plan 12-02 wires the provider.
- [Phase 12-resolved-imports-and-module-relationships]: ModuleGraphFacts::reachable_from uses deterministic breadth-first traversal over Resolved and External edges only.
- [Phase 12-resolved-imports-and-module-relationships]: Public relationship facts expose polint-owned IDs and status enums, not resolver outputs or graph internals.
- [Phase 12-resolved-imports-and-module-relationships]: Run module graph derivation after Go and TS/JS syntax analysis and before derived metrics or rule execution.
- [Phase 12-resolved-imports-and-module-relationships]: Keep TS/JS and Go resolver outputs as crate-private drafts; public facts expose only polint-owned IDs and status enums.
- [Phase 12-resolved-imports-and-module-relationships]: Do not synthesize a root module node for an empty repository; empty relationship views stay empty.
- [Phase 12-resolved-imports-and-module-relationships]: Provider-derived setup-missing support rows emit their own capability diagnostics before rules are blocked.
- [Phase 12-resolved-imports-and-module-relationships]: Resolver output paths are never exposed publicly; they are normalized and mapped to FileIds before becoming relationship facts.
- [Phase 12-resolved-imports-and-module-relationships]: Use symlinks:false on the TS resolver so path identity stays lexical and matches the AnalysisDb file index.
- [Phase 12-resolved-imports-and-module-relationships]: Derive TS module ownership from nearest package.json or tsconfig.json, preferring package names for labels.
- [Phase 12-resolved-imports-and-module-relationships]: Keep the dynamic import sentinel crate-private and convert it into explicit Dynamic relationship facts in the provider.
- [Phase 12-resolved-imports-and-module-relationships]: Go metadata is loaded only from repository-root Go modules using fixed go list command execution with GOFLAGS removed.
- [Phase 12-resolved-imports-and-module-relationships]: Go package graph nodes are labeled by import path, while Go module nodes are labeled by the go list module path.
- [Phase 12-resolved-imports-and-module-relationships]: Missing Go module setup remains visible as setup-missing facts/support and blocks requesting rules through the provider support merge.
- [Phase 12-resolved-imports-and-module-relationships]: TS/JS local import graph edges originate from the importing file node so architecture rules can detect file-level boundaries.
- [Phase 12-resolved-imports-and-module-relationships]: TS/JS external package imports remain module-level DependsOn edges so project dependency relationships stay compact.
- [Phase 12-resolved-imports-and-module-relationships]: Resolved import docs treat SetupMissing, Dynamic, Unsupported, and Unresolved as public data, not hidden failures.
- [Phase 12-resolved-imports-and-module-relationships]: Test-only Go graph helper methods are cfg(test) rather than suppressed with lint allowances.

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
| Phase 09 P01 | 4 min | 3 tasks | 1 files |
| Phase 09 P02 | 3 min | 3 tasks | 7 files |
| Phase 09 P03 | 4 min | 3 tasks | 1 files |
| Phase 09 P04 | 2 min | 3 tasks | 1 files |
| Phase 11-capability-driven-analysis-plan P01 | 8 min | 2 tasks | 4 files |
| Phase 11-capability-driven-analysis-plan P02 | 16m 12s | 2 tasks | 9 files |
| Phase 11-capability-driven-analysis-plan P03 | 22m 23s | 3 tasks | 7 files |
| Phase 12-resolved-imports-and-module-relationships P01 | 11m 4s | 3 tasks | 5 files |
| Phase 12-resolved-imports-and-module-relationships P02 | 1h 1m | 3 tasks | 11 files |
| Phase 12-resolved-imports-and-module-relationships P03 | 17 min | 3 tasks | 9 files |
| Phase 12-resolved-imports-and-module-relationships P04 | 16m 9s | 3 tasks | 3 files |
| Phase 12-resolved-imports-and-module-relationships P05 | 30 min | 3 tasks | 8 files |

## Session

**Last Date:** 2026-05-12T19:32:39.462Z
**Stopped At:** Phase 13 context gathered
**Resume File:** .planning/phases/13-symbols-and-references/13-CONTEXT.md

## Important Context For Execution

- Do not fake functionality. If a feature remains heuristic or experimental, label it that way.
- Treat repo-local rule packs as external SDK consumers: tests for rule-authoring features should prove public `polint::sdk` / `polint::runner` usage from a temp repo whenever practical.
- Keep policy rules out of the shipped CLI; each example owns exactly one local rule crate under `examples/*/.polint/rules/`.
- Use deterministic ordering everywhere output can be observed.
- Prefer a smaller complete v1 over broad shallow behavior.
- Keep source and GSD planning changes in `/Users/emilwareus/Development/exlint` on `main`.
- Do not create or use GSD worktrees for this project.
