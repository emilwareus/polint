# Phase 6: SDK and Example Rules - Context

**Gathered:** 2026-04-30
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 6 makes rule authoring pleasant and proves the public SDK by implementing and hardening the requested example rules. The phase delivers a documented `polint-sdk` authoring surface, `RuleCtx` helpers for already-collected facts, SDK-oriented built-in example rules, deterministic diagnostics, fixtures, and representative snapshots.

This phase does not add dynamic repo-local Rust compilation, Wasm plugin loading, production graph commands, cache/performance work, TypeScript semantic type checking, Node/TS module resolution, or final SARIF/CI hardening. Those remain later phases.

</domain>

<decisions>
## Implementation Decisions

### SDK public surface
- **D-01:** Treat the current `Rule`, `RuleMeta`, `Capabilities`, `RuleCtx`, `RuleOptions`, and `RuleRegistry` shape from `polint-core` as the Phase 6 baseline. Improve and document it rather than replacing it with a query engine or broad core rewrite.
- **D-02:** Make `polint-sdk` the public rule-authoring entry point. Rule authors should be able to start from `use polint_sdk::prelude::*;` without importing `polint-core` directly for normal use.
- **D-03:** Keep the v1 SDK additive and source-compatible where practical. Add focused docs, examples, prelude exports, and helper methods instead of renaming stable concepts or introducing a large abstraction layer.

### Rule query helpers
- **D-04:** Expose the Phase 3-5 fact model through high-level `RuleCtx` helpers for files, functions, imports, branch obligations, Go tests, TS components/classes, string literals, JSX attributes, and diagnostic reporting.
- **D-05:** Prefer borrowed slices, iterators, or small filtered helpers over cloning large fact vectors. Preserve deterministic ordering from `AnalysisDb`.
- **D-06:** Add graph/query helpers only where needed to satisfy `SDK-02` and the example rules. Production graph commands, full call graphs, and Node/TS resolver semantics stay out of Phase 6.

### Example rule strategy
- **D-07:** Keep all eight requested rules as built-in SDK dogfood examples with the existing `examples/...` rule IDs: Go complexity, TS complexity, Go import boundaries, TS raw colors, Go branch obligations, Go test suite size, Go assertion-after-action, and config-query denied literals.
- **D-08:** Example rules should consume SDK-facing APIs rather than private core shortcuts. If a rule needs a fact access pattern that only `AnalysisDb` exposes today, add a narrow `RuleCtx`/SDK helper before using it.
- **D-09:** Use the current `built_in_rules()` registration path for Phase 6. Do not add dynamic plugin loading or automatic repo-local Rust rule compilation.
- **D-10:** Reuse existing Go/TS fixtures and examples where possible, expanding them only enough to prove each requested rule family.

### Config and diagnostics
- **D-11:** Reuse the existing `RuleOptions` and TOML config shape for thresholds, file globs, allow-lists, denied literals, and forbidden imports. Add only narrow option fields if a requested rule cannot be configured honestly with the existing fields.
- **D-12:** Every example rule diagnostic should be deterministic and useful: stable rule ID, accurate file/range, concise message, evidence where it helps, and help text for remediation.
- **D-13:** Heuristic rules must say they are heuristic in metadata, messages, help, or evidence where relevant. In particular, Go branch obligations, test suite size, and assertion-after-action must not claim exact coverage or semantic proof.
- **D-14:** TS raw-color and denied-literal rules report syntax-level literal findings, not CSS semantic validation or design-token authority.

### Testing proof
- **D-15:** Use focused SDK/core unit tests for the public prelude, helper methods, capability declarations, rule configuration, and rule diagnostics.
- **D-16:** Use CLI integration tests to prove configured profiles run each requested rule against clean/failing fixtures and parsed JSON output.
- **D-17:** Add representative snapshots for diagnostic output by rule family, especially human and JSON diagnostics. Full production SARIF hardening remains Phase 8 unless a small snapshot naturally falls out of this work.
- **D-18:** Continue asserting structured diagnostic fields where possible rather than relying on substring-only tests.

### Scaffolding and docs
- **D-19:** Keep `polint new-rule` templates aligned with `polint-sdk::prelude::*`, capabilities, and `RuleCtx` helpers.
- **D-20:** Add enough SDK docs/examples that a repo-local rule author can understand the trait, metadata, capabilities, options, query helpers, and diagnostic reporting loop.
- **D-21:** Do not promise that generated repo-local Rust rules are automatically compiled or loaded in v1. Scaffolding and native registration remain the honest boundary until plugin/loader phases.

### Execution policy
- **D-22:** Work directly in `/Users/emilwareus/Development/exlint` on `main`; do not create or use GSD worktrees.
- **D-23:** Keep changes narrow and test-driven. Prefer additive SDK helpers and rule tests over sweeping rewrites.

### the agent's Discretion
- The agent may choose whether to split plans by SDK surface, rule family, diagnostics/snapshots, or CLI proof, as long as every requested rule and requirement ID is traceable.
- The agent may add small helper methods, test fixtures, or snapshot utilities when they reduce duplication and preserve deterministic output.
- The agent may decide exact snapshot file organization and fixture names.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` — Phase 6 goal, requirement IDs, and success criteria.
- `.planning/REQUIREMENTS.md` — `SDK-01`, `SDK-02`, `RULE-01` through `RULE-08`, `TEST-01`, and `TEST-03`.
- `.planning/PROJECT.md` — Product value, constraints, current validated requirements, and no-worktree repository layout.

### Prior decisions to carry forward
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` — Core rule traits, `AnalysisDb`, diagnostics, deterministic output, capability contract, and testing decisions.
- `.planning/phases/04-go-adapter/04-CONTEXT.md` — Go fact boundaries, heuristic wording, fixture strategy, and Phase 6 handoff for Go rules.
- `.planning/phases/05-typescript-adapter/05-CONTEXT.md` — TS/JS fact boundaries, syntax-level heuristics, fixture strategy, and Phase 6 handoff for TS rules.

### Source surfaces to inspect
- `crates/polint-sdk/src/lib.rs` — Current public SDK prelude.
- `crates/polint-core/src/lib.rs` — Rule trait, metadata, capabilities, `RuleCtx`, `RuleOptions`, registry, and diagnostics flow.
- `crates/polint-rules/src/lib.rs` — Existing built-in example rule implementations.
- `crates/polint-cli/src/main.rs` — `new-rule` template, built-in rule registration, check/test/profile command wiring.
- `crates/polint-config/src/lib.rs` — Current profile and rule option config shape.
- `crates/polint-cli/tests/cli.rs` — Existing CLI integration patterns for rule diagnostics and fixtures.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-sdk/src/lib.rs` already exposes a `prelude` that re-exports core rule types and diagnostic builders, but it has almost no documentation or SDK-owned tests.
- `crates/polint-core/src/lib.rs` already contains the main authoring primitives: `Rule`, `RuleMeta`, `Capabilities`, `RuleOptions`, `RuleCtx`, `RuleRegistry`, panic/error containment, capability tests, and deterministic runner behavior.
- `crates/polint-rules/src/lib.rs` already has all eight requested `examples/...` rule IDs registered in `built_in_rules()`, with partial configuration and diagnostic behavior.
- `crates/polint-cli/src/main.rs` already scaffolds repo-local rule skeletons using `polint_sdk::prelude::*` and language-specific capability defaults.
- `tests/fixtures/go/`, `tests/fixtures/ts/`, `tests/fixtures/mixed/`, `examples/go-branch-obligations/`, and `examples/ts-design-tokens/` provide existing fixture material to expand.

### Established Patterns
- Rules are ordinary Rust structs implementing `Rule`, registered as `Arc<dyn Rule>`, and run through `run_rules` with deterministic dedupe/sort behavior.
- Rule options currently flow through `RuleOptions` fields: `severity`, `files`, `allow_files`, `max`, `deny`, and `forbidden_imports`.
- CLI tests prefer temp repos, TOML profiles, `assert_cmd`, parsed JSON assertions, and targeted fixture copies.
- Prior phases require heuristic language to be explicit and syntax-level facts not to overclaim semantic coverage.

### Integration Points
- SDK improvements connect through `polint-sdk`, `polint-core::RuleCtx`, and `polint-cli` generated rule templates.
- Example rule hardening connects through `polint-rules::built_in_rules()`, default config/profile behavior, and CLI check execution.
- Snapshot work should integrate with the existing Rust test stack (`insta`, `pretty_assertions`, `assert_cmd`) without changing the CLI contract outside Phase 6 scope.

</code_context>

<specifics>
## Specific Ideas

- Preserve the existing `examples/...` rule ID naming so prior tests and profiles continue to work.
- Treat built-in rules as SDK examples, not a comprehensive lint ruleset.
- Prefer small public helpers that make rule code pleasant over a large new query framework.
- Keep generated custom rule skeletons honest: they compile-looking and demonstrate SDK shape, but they are not automatically loaded as dynamic plugins in this phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 06-sdk-and-example-rules*
*Context gathered: 2026-04-30*
