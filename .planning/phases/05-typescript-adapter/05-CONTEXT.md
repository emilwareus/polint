# Phase 5: TypeScript Adapter - Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 5 hardens the TypeScript/JavaScript adapter so `.ts`, `.tsx`, `.js`, and `.jsx` files parse through Oxc, parser failures become controlled diagnostics, and useful syntax-level TS/JS facts are extracted for later example rules, SDK work, and graph work. The scope is imports/exports, functions, classes, React-ish component functions, JSX attributes, string literals, basic cyclomatic complexity, and import graph facts where the current core model supports them.

This phase does not add full TypeScript type checking, production module resolution, comprehensive React semantics, custom rule loading, final graph commands, or production SARIF/CI behavior. Those remain later semantic, SDK, Phase 8, and release-hardening work.

</domain>

<decisions>
## Implementation Decisions

### Parser diagnostics and extraction source
- **D-01:** Use Oxc as the parsing source of truth for Phase 5, not the current line-oriented/string-oriented extraction as the long-term contract.
- **D-02:** Parser errors must become `parser/ts` diagnostics with stable file/range/message behavior instead of crashes or silent failures.
- **D-03:** When Oxc reports parse errors, the adapter may still extract best-effort facts from the parsed tree where Oxc provides one, but diagnostics must make the parse problem explicit.
- **D-04:** Use Oxc syntax/AST APIs first. Do not introduce TypeScript compiler services, full semantic analysis, or production `oxc_resolver` behavior unless a minimal helper is needed to represent Phase 5 facts honestly.
- **D-05:** Avoid cloning large source strings. The adapter should parse borrowed `&str` from the shared `Arc<str>` source where Oxc APIs allow it.

### TS/JS fact breadth
- **D-06:** Extract the full Phase 5 syntax-level fact set: imports, re-exports/export-from module specifiers, functions, arrow-function declarations, methods where practical, classes, component-like functions, JSX attributes, string/template literals, calls, and basic cyclomatic complexity.
- **D-07:** Preserve the Phase 3 `AnalysisDb` contract: facts are pushed through core APIs, IDs remain deterministic by insertion order, and traversal order must be deterministic.
- **D-08:** If a desired TS/JS fact has no exact core field yet, prefer a narrow additive core field/accessor, such as a class fact, over overloading unrelated facts or broad refactors.
- **D-09:** Import graph facts should be computed in the practical v1 form supported by current crates: parser-backed module specifiers feed `ImportFact` and graph helpers. Production graph commands, DOT hardening, and full Node/TS resolution remain Phase 8 or later.

### Component, JSX, and raw color heuristics
- **D-10:** React-ish component detection is syntax-level and heuristic: PascalCase functions/classes, exported component-like declarations, and JSX-returning functions are enough for Phase 5. Do not claim exact React component discovery.
- **D-11:** JSX attribute facts should come from Oxc JSX AST nodes, including string attributes and expression attributes where a stable string value is practical.
- **D-12:** String literal extraction should include TS/JS string literals and template literals with static text where practical. Dynamic template expressions can be represented conservatively or skipped if exact value would be misleading.
- **D-13:** Raw color fixture proof should focus on `examples/ts-no-raw-colors` and `examples/config-query-no-literal` style rule consumption, not a comprehensive CSS parser.

### Complexity and branches
- **D-14:** Compute basic TS/JS cyclomatic complexity from AST control-flow constructs such as `if`, loops, switch cases, `catch`, logical operators, and conditionals.
- **D-15:** Function spans, call names, and complexity should be parser-backed enough that rules report useful locations and stable evidence.
- **D-16:** Do not broaden Phase 5 into exact control-flow graph or call graph construction. The core `cfg`/`call_graph` capability flags remain future-facing unless a narrow fact is already present.

### Fixtures and CLI proof
- **D-17:** Use focused adapter unit tests for Oxc parsing and extraction internals, and CLI integration fixtures for clean/failing TS/JS behavior.
- **D-18:** Reuse and expand existing TS fixtures under `tests/fixtures/ts/`, `tests/fixtures/mixed/view.ts`, and `examples/ts-design-tokens/Button.tsx` instead of inventing unrelated fixture layouts.
- **D-19:** Verify built-in TS rules only to the extent they prove Phase 5 facts are usable, especially `examples/ts-cyclomatic-complexity`, `examples/ts-no-raw-colors`, and string-literal based rule paths. Full SDK/rule authoring completeness remains Phase 6.
- **D-20:** Clean TS/JS fixtures should parse without `parser/ts` diagnostics. Failing fixtures should produce useful TS rule diagnostics from Phase 5 facts.

### Execution policy
- **D-21:** Work directly in `/Users/emilwareus/Development/exlint` on `main`; do not create or use GSD worktrees.
- **D-22:** Keep changes narrow and test-driven. Prefer Oxc AST visitors/helpers over ad hoc line scanning when the parser gives reliable structured APIs.
- **D-23:** Keep all heuristic language honest in diagnostics/help text and documentation. If a fact is approximate, say so.

### the agent's Discretion
- The agent may choose the exact Oxc AST traversal helper structure and whether to split visitor logic by fact family or syntax kind.
- The agent may decide whether to add a narrow class fact to `polint-core` or represent class-like callable behavior through existing facts only where that remains truthful.
- The agent may decide how much static template-literal text to collect, as long as dynamic values are not presented as exact.
- The agent may split work by parser diagnostics, import/export facts, declarations/classes/components, JSX/string facts, complexity, and CLI fixtures, as long as plans remain independently verifiable.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and phase scope
- `docs/INITIAL_PROMPT.md` - Original product prompt, TS/JS analysis expectations, non-goals, and quality bar.
- `.planning/PROJECT.md` - Core value, validated requirements, active TypeScript analysis requirement, parser choices, and no-worktree repository layout.
- `.planning/REQUIREMENTS.md` - Phase 5 requirement IDs: `TS-01`, `TS-02`, `TS-03`, `TEST-01`, and `TEST-02`.
- `.planning/ROADMAP.md` - Phase 5 goal and success criteria.
- `.planning/STATE.md` - Current main-branch execution policy and Phase 5 focus.

### Prior phase decisions and evidence
- `.planning/phases/01-workspace-foundation/01-CONTEXT.md` - Locked crate boundaries and main-branch/no-worktree policy.
- `.planning/phases/02-cli-config-and-discovery/02-CONTEXT.md` - CLI/config/discovery contracts and clean/failing fixture testing style.
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` - Stable fact model, deterministic IDs, runner, diagnostic, and testing decisions that Phase 5 must honor.
- `.planning/phases/03-core-facts-and-diagnostics/03-VERIFICATION.md` - Evidence that Phase 3 core facts, diagnostics, deterministic discovery, and review fixes are verified.
- `.planning/phases/04-go-adapter/04-CONTEXT.md` - Adapter hardening pattern for parser diagnostics, syntax facts, heuristic honesty, fixtures, and no production graph-command broadening.
- `.planning/phases/04-go-adapter/04-VERIFICATION.md` - Evidence that the Go adapter pattern was verified and can guide TS adapter closure.

### Existing implementation touchpoints
- `crates/polint-ts/src/lib.rs` - Current Oxc parser invocation plus line/string extraction baseline to replace or harden.
- `crates/polint-core/src/lib.rs` - `AnalysisDb`, TS-relevant fact models, spans, imports, functions, components, string literals, JSX attributes, and rule context queries.
- `crates/polint-rules/src/lib.rs` - Built-in TS rules consuming TS complexity, string literals, JSX attributes, and generic string-literal facts.
- `crates/polint-cli/src/main.rs` - Integration point that runs Go analysis, TS analysis, rules, dedupe, and output rendering.
- `crates/polint-cli/tests/cli.rs` - Existing CLI integration test pattern with `assert_cmd`, `tempfile`, generated config, and JSON assertions.
- `crates/polint-graph/src/lib.rs` - Existing graph helper crate available for practical import graph work.

### TS/JS fixtures and examples
- `tests/fixtures/ts/clean/component.tsx` - Existing clean TSX fixture.
- `tests/fixtures/ts/failing/component.tsx` - Existing failing/raw-color TSX fixture.
- `tests/fixtures/mixed/view.ts` - Existing mixed-language TS fixture.
- `examples/ts-design-tokens/Button.tsx` - Existing raw-color example source.
- `examples/custom-rule-ts/README.md` - Existing TS custom-rule example documentation.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-ts/src/lib.rs`: Already uses Oxc `Parser`, `Allocator`, and `SourceType`, and pushes imports, functions, string literals, JSX attributes, and TS component facts, but much extraction is currently string/line based and clones source into a `String`.
- `crates/polint-core/src/lib.rs`: Provides `FunctionFact`, `ImportFact`, `TsComponentFact`, `StringLiteralFact`, `JsxAttributeFact`, `Span`, `AnalysisDb`, and deterministic fact insertion. There is no dedicated class fact yet.
- `crates/polint-rules/src/lib.rs`: Includes TS example rules for cyclomatic complexity and raw color literals, plus generic denied string literal behavior.
- `crates/polint-cli/tests/cli.rs`: Provides the existing integration-test shape for temp repos, config writing, command execution, JSON parsing, and output assertions.
- `tests/fixtures/ts/` and `examples/ts-design-tokens/`: Existing source material that can be expanded into clean/failing fixture coverage.

### Established Patterns
- Adapters clone the file list before mutating `AnalysisDb`, parse per language, and return parser diagnostics separately from rule diagnostics.
- Phase 3 expects deterministic discovery and fact insertion order; Phase 5 traversal must keep that property.
- Phase 4 established the adapter-hardening pattern: parser-backed facts, controlled parser diagnostics, focused unit tests, CLI integration proof, and explicit heuristic language.
- Built-in rules are SDK examples, not a comprehensive rule suite. Phase 5 should prove facts are usable without turning into Phase 6.

### Integration Points
- `polint-cli::analyze_and_run` calls `polint_ts::analyze(&mut db)` before rule execution.
- TS facts feed `RuleCtx::functions`, `RuleCtx::imports`, `RuleCtx::ts_components`, `RuleCtx::string_literals`, and `RuleCtx::jsx_attributes`.
- `examples/ts-cyclomatic-complexity` depends on accurate enough TS/JS function and complexity facts.
- `examples/ts-no-raw-colors` depends on string literal and JSX attribute facts.
- Import graph work can use parser-backed module specifiers through existing `ImportFact` and `polint-graph` helpers.

</code_context>

<specifics>
## Specific Ideas

- Prioritize Oxc AST correctness and deterministic traversal over broad regex-like extraction.
- Keep TS/JS analysis syntax-level for v1 and avoid pretending that component detection or raw-color extraction is semantically complete.
- Parser errors should be visible diagnostics even if best-effort facts can still be extracted.
- Fixture coverage should include clean TS/TSX/JS/JSX, failing raw-color TSX, invalid parser diagnostics, and import/export cases.

</specifics>

<deferred>
## Deferred Ideas

- Full TypeScript type checking, symbol resolution, and exact component semantics remain future semantic work.
- Production Node/TS import resolution through `oxc_resolver` remains later graph/CI hardening unless a narrow helper is required for Phase 5 facts.
- Exact CFG/call graph construction remains future work.
- Comprehensive TS example rule completion and SDK authoring documentation remain Phase 6 and Phase 10 work unless a small check is needed to verify Phase 5 facts.
- Production graph commands, DOT output hardening, and final CI exit semantics remain Phase 8.

</deferred>

---

*Phase: 05-typescript-adapter*
*Context gathered: 2026-04-29*
