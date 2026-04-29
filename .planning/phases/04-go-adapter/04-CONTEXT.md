# Phase 4: Go Adapter - Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 4 hardens the Go adapter so Go files parse through `tree-sitter-go`, parser failures become controlled diagnostics, and useful syntax-level Go facts are extracted for later rules and graph work. The scope is packages, imports, functions, methods, tests, subtests, table-test evidence, branch obligations, basic error-path heuristics, cyclomatic complexity, and import graph facts where the current core model supports them.

This phase does not add full Go type checking, exact dynamic coverage, or production rule-suite completeness. Those remain later semantic, coverage, SDK, and rule phases.

</domain>

<decisions>
## Implementation Decisions

### Parser diagnostics and extraction source
- **D-01:** Use `tree-sitter-go` as the parsing source of truth for Phase 4, not the current line-oriented extraction as the long-term contract.
- **D-02:** Parser errors must become `parser/go` diagnostics with stable file/range/message behavior instead of crashes or silent failures.
- **D-03:** When tree-sitter reports syntax errors, the adapter may still extract best-effort facts from valid subtrees, but diagnostics must make the parse problem explicit.
- **D-04:** Keep the adapter local to `crates/polint-go`; do not introduce a Go toolchain sidecar, `go/packages`, or `go/analysis` in this phase.

### Go fact breadth
- **D-05:** Extract the full Phase 4 syntax-level fact set: package names where the existing model can carry them, imports, functions, methods, test functions, subtests, table-test evidence, calls, and basic cyclomatic complexity.
- **D-06:** Preserve the Phase 3 `AnalysisDb` contract: facts are pushed through core APIs, IDs remain deterministic by insertion order, and source text should not be cloned unnecessarily.
- **D-07:** If a desired Go fact has no exact core field yet, prefer a narrow additive core field or clearly documented approximation over broad refactors.
- **D-08:** Import graph facts should be computed in the practical v1 form supported by current crates; deeper graph commands and DOT output hardening remain Phase 8 unless a minimal integration is needed for `GO-04`.

### Branch obligations and error-path heuristics
- **D-09:** Extract branch obligations for `if`, `switch`, `case`, `default`, `for`, and `range` constructs using stable spans and fingerprints.
- **D-10:** Mark basic error paths with conservative syntax heuristics such as `err != nil`, returned `error` values, and recognizable error result branches. This must be described as heuristic, never exact coverage.
- **D-11:** Branch obligation fingerprints should stay stable for the same file/function/location/condition/edge identity and must not depend on traversal nondeterminism.
- **D-12:** Exact branch coverage and semantic path analysis remain out of scope.

### Test evidence and fixtures
- **D-13:** Extract Go test evidence from `_test.go` files: test function names, `t.Run` subtests, assertion/error-check counts, table rows where practical, and evidence terms useful to heuristic rules.
- **D-14:** Use focused adapter unit tests for parsing/extraction internals and CLI integration fixtures for clean/failing Go behavior.
- **D-15:** Reuse and expand the existing Go fixtures under `tests/fixtures/go/` and the Go branch-obligation example instead of inventing unrelated fixture layouts.
- **D-16:** Verify built-in Go rules only to the extent they prove Phase 4 facts are usable. Full SDK/rule authoring completeness remains Phase 6.

### Execution policy
- **D-17:** Work directly in `/Users/emilwareus/Development/exlint` on `main`; do not create or use GSD worktrees.
- **D-18:** Keep changes narrow and test-driven. Prefer tree-sitter node traversal helpers over ad hoc string scanning when the parser gives a reliable structured API.

### the agent's Discretion
- The agent may choose the exact tree-sitter traversal helper structure.
- The agent may decide whether to split the work by parser diagnostics, fact extraction, branch/test evidence, or CLI fixtures, as long as plans remain independently verifiable.
- The agent may add small core accessors or fields only when needed to represent Phase 4 facts honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and phase scope
- `docs/INITIAL_PROMPT.md` - Original product prompt, Go analysis expectations, non-goals, and quality bar.
- `.planning/PROJECT.md` - Core value, validated requirements, active Go analysis requirement, parser choices, and no-worktree repository layout.
- `.planning/REQUIREMENTS.md` - Phase 4 requirement IDs: `GO-01`, `GO-02`, `GO-03`, `GO-04`, `TEST-01`, and `TEST-02`.
- `.planning/ROADMAP.md` - Phase 4 goal and success criteria.
- `.planning/STATE.md` - Current main-branch execution policy and Phase 4 focus.

### Prior phase decisions and evidence
- `.planning/phases/01-workspace-foundation/01-CONTEXT.md` - Locked crate boundaries and main-branch/no-worktree policy.
- `.planning/phases/02-cli-config-and-discovery/02-CONTEXT.md` - CLI/config/discovery contracts and clean/failing fixture testing style.
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` - Stable fact model, deterministic IDs, runner, diagnostic, and testing decisions that Phase 4 must honor.
- `.planning/phases/03-core-facts-and-diagnostics/03-VERIFICATION.md` - Evidence that Phase 3 core facts, diagnostics, deterministic discovery, and review fixes are verified.

### Existing implementation touchpoints
- `crates/polint-go/src/lib.rs` - Current Go parser invocation and line-oriented extraction baseline to harden.
- `crates/polint-core/src/lib.rs` - `AnalysisDb`, Go-relevant fact models, spans, branch obligations, tests, imports, functions, and rule context queries.
- `crates/polint-rules/src/lib.rs` - Built-in Go rules consuming Go complexity, import, branch, and test facts.
- `crates/polint-cli/src/main.rs` - Integration point that runs Go analysis, TS analysis, rules, dedupe, and output rendering.
- `crates/polint-cli/tests/cli.rs` - Existing CLI integration test pattern with `assert_cmd` and `tempfile`.
- `crates/polint-graph/src/lib.rs` - Existing graph helper crate available for practical import graph work.

### Go fixtures and examples
- `tests/fixtures/go/clean/payment.go` - Existing clean Go fixture.
- `tests/fixtures/go/clean/payment_test.go` - Existing Go test fixture.
- `tests/fixtures/go/failing/payment.go` - Existing failing/error-path Go fixture.
- `tests/fixtures/mixed/main.go` - Existing mixed-language Go fixture.
- `examples/go-branch-obligations/authorize.go` - Existing branch obligation example.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-go/src/lib.rs`: Already creates a tree-sitter parser and pushes imports, functions, branch obligations, and test facts, but much extraction is line/string based.
- `crates/polint-core/src/lib.rs`: Provides `FunctionFact`, `ImportFact`, `BranchObligation`, `TestFact`, `CoverageFact`, `Span`, `AnalysisDb`, and deterministic fact insertion.
- `crates/polint-rules/src/lib.rs`: Includes Go example rules for cyclomatic complexity, import boundaries, branch obligations, test suite size, and assertion-after-action.
- `crates/polint-cli/tests/cli.rs`: Provides the existing integration-test shape for temp repos, generated config, command execution, JSON parsing, and output assertions.
- `tests/fixtures/go/` and `examples/go-branch-obligations/`: Existing source material that can be expanded into clean/failing fixture coverage.

### Established Patterns
- Adapters mutate `AnalysisDb` by cloning the file list, parsing per language, and pushing facts into core.
- Diagnostics are returned from adapters as `Diagnostic` values and later combined with rule diagnostics by the CLI.
- Phase 3 expects deterministic discovery and fact insertion order; Phase 4 traversal must keep that property.
- Heuristic behavior is acceptable when labeled honestly and tested around representative fixtures.

### Integration Points
- `polint-cli::analyze_and_run` calls `polint_go::analyze(&mut db)` before TS analysis and rule execution.
- Go facts feed `RuleCtx::functions`, `RuleCtx::imports`, `RuleCtx::branch_obligations`, and `RuleCtx::go_tests`.
- Go rules depend on accurate enough complexity, import paths, branch conditions, test evidence, and file paths.
- Import graph work can use existing import facts and `polint-graph` helpers if the current graph API is sufficient.

</code_context>

<specifics>
## Specific Ideas

- Prioritize correctness and determinism over breadth that looks impressive but is not reliable.
- Keep Go analysis syntax-level for v1 and avoid pretending that heuristic test evidence proves exact coverage.
- Parser errors should be visible diagnostics even if some facts can still be extracted.
- Fixture coverage should include both clean Go and intentionally failing/error-path Go cases.

</specifics>

<deferred>
## Deferred Ideas

- Full Go type information through `go/packages` or `go/analysis` remains v2 semantic work.
- Exact dynamic branch coverage remains future coverage work.
- Comprehensive Go example rule completion and documentation remain Phase 6 and Phase 10 work unless a small check is needed to verify Phase 4 facts.
- Production graph command and DOT output hardening remain Phase 8.

</deferred>

---

*Phase: 04-go-adapter*
*Context gathered: 2026-04-29*
