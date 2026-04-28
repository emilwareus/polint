# Phase 3: Core Facts and Diagnostics - Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 3 hardens the core fact model, analysis database, rule registry/runner, diagnostic model, deterministic ordering, and SDK-facing primitives that later Go, TypeScript, rules, cache, and CI phases build on.

This phase does not implement full Go or TypeScript semantic extraction, cache persistence, final CLI command hardening, or production SARIF completeness. It should make the existing main-branch implementation reliable, tested, and explicit at the shared core/diagnostics boundary.

</domain>

<decisions>
## Implementation Decisions

### Existing implementation stance
- **D-01:** Treat the current `main` implementation of `polint-core`, `polint-diagnostics`, `polint-fs`, and `polint-sdk` as the Phase 3 baseline.
- **D-02:** Plan targeted hardening and tests around the current architecture rather than replacing it with a query engine or broad refactor.
- **D-03:** Work directly in `/Users/emilwareus/Development/exlint` on `main`; do not create or use GSD worktrees.

### Fact model boundaries
- **D-04:** Keep `AnalysisDb` as the shared, append-only, Vec-backed fact store for v1, with typed IDs for files, nodes, functions, packages, branches, imports, and rules.
- **D-05:** Stable IDs in Phase 3 mean deterministic IDs within a run, derived from deterministic file discovery and insertion order, plus stable fingerprints where cross-run identity is externally visible.
- **D-06:** Keep source text stored once per `SourceFile` using shared storage such as `Arc<str>`; avoid cloning large source strings in core APIs.
- **D-07:** Core fact models should cover files, spans, functions, imports, branch obligations, tests, coverage placeholders, TS components, string literals, JSX attributes, and analysis database accessors.
- **D-08:** Exact Go type information, exact dynamic branch coverage, and deeper language-specific extraction remain later work. Phase 3 should provide stable models and honest placeholders those later phases can fill.

### Span and source location contract
- **D-09:** Keep spans byte-based plus one-based line/column positions so parsers can map native byte offsets while diagnostics stay human-readable.
- **D-10:** Phase 3 should harden span conversion for UTF-8, newline boundaries, empty ranges, and monotonic ordering with unit/property tests.
- **D-11:** Diagnostics should consume core spans through a narrow conversion method such as `Span::diagnostic_range()` rather than duplicating conversion logic in rules.

### Rule registry and runner behavior
- **D-12:** Preserve the current `Rule`, `RuleMeta`, `Capabilities`, `RuleCtx`, `RuleOptions`, and `RuleRegistry` shape as the SDK-facing core contract.
- **D-13:** `run_rules` should keep enabled-rule filtering, rule option/severity overrides, deterministic output ordering, diagnostic deduplication, and optional parallel execution.
- **D-14:** Rule panics and rule errors must become controlled internal diagnostics, not process crashes.
- **D-15:** Capability declarations should be treated as a real contract: exposed, tested, and available for planner/verifier checks. Full fact-pruning or dynamic loading based on capabilities can remain later work unless needed for correctness.
- **D-16:** Rule execution should be deterministic whether `parallel` is true or false.

### Diagnostic contract
- **D-17:** Diagnostics should include severity, file/range, message, labels, evidence, suggestions, fixes, help text, stable fingerprints, and deterministic sorting/deduplication.
- **D-18:** Human and JSON diagnostic output are in scope for Phase 3 snapshots. SARIF-like output exists but full production CI/SARIF hardening remains Phase 8.
- **D-19:** Diagnostic fingerprints should remain stable for the same rule/file/range/message inputs and should not depend on map iteration order or parallel execution order.
- **D-20:** Human output should stay concise and useful for local rule authors; JSON output should remain machine-parseable and stable enough for snapshots.

### File discovery determinism
- **D-21:** Phase 3 closes `FS-02` by proving file discovery output is deterministic across filesystem traversal order, include/exclude behavior, and supported language filtering.
- **D-22:** Do not broaden discovery semantics beyond Phase 2 unless required to make ordering deterministic and tested.

### Testing focus
- **D-23:** Add focused unit tests for core IDs/facts, registry filtering, panic/error containment, capability declarations, diagnostic dedupe/sort/fingerprints, and span conversion.
- **D-24:** Add snapshot tests for human and JSON diagnostics using the existing Rust test stack; do not require snapshot coverage for SARIF in Phase 3.
- **D-25:** Add property tests where they give leverage: span roundtrips/monotonicity, diagnostic sorting determinism, and file discovery exclusion/order invariants.
- **D-26:** Keep tests targeted to shared core behavior. Clean/failing Go/TS fixture breadth belongs to Phases 4 and 5, and broader CI command coverage remains Phase 8.

### the agent's Discretion
- The agent may choose whether Phase 3 is split by source area, by test type, or by dependency order as long as each plan remains small enough to verify.
- The agent may introduce small helpers or test-only fixtures when they reduce duplication or make invariants clearer.
- The agent may adjust existing public structs only when the change is needed for the Phase 3 contract and is coordinated with current adapters/rules.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and phase scope
- `docs/INITIAL_PROMPT.md` - Original product prompt, non-goals, requested fact/rule/diagnostic infrastructure, and v1 quality bar.
- `.planning/PROJECT.md` - Core value, constraints, validated requirements, active requirements, and decisions.
- `.planning/REQUIREMENTS.md` - Phase 3 requirement IDs: `FS-02`, `CORE-01`, `CORE-02`, `DIAG-01`, `TEST-01`, `TEST-03`, and `TEST-04`.
- `.planning/ROADMAP.md` - Phase 3 goal and success criteria.
- `.planning/STATE.md` - Current no-worktree execution policy and Phase 3 focus.

### Prior phase decisions and evidence
- `.planning/phases/01-workspace-foundation/01-CONTEXT.md` - Locked decisions on main-branch execution, crate boundaries, and narrow verification-first hardening.
- `.planning/phases/02-cli-config-and-discovery/02-CONTEXT.md` - Locked decisions on CLI/config/discovery boundaries, deterministic discovery expectations, and no overclaiming later phases.
- `.planning/phases/02-cli-config-and-discovery/02-VERIFICATION.md` - Evidence that Phase 2 closed the first CLI/config/discovery loop and left later requirements open.

### Existing implementation touchpoints
- `crates/polint-core/src/lib.rs` - Current fact models, analysis database, rule trait, capabilities, rule context, registry, runner, span conversion, and core tests.
- `crates/polint-diagnostics/src/lib.rs` - Current diagnostic model, sort/dedupe behavior, fingerprinting, and human/JSON/SARIF-like renderers.
- `crates/polint-fs/src/lib.rs` - Current deterministic discovery and analysis database loading.
- `crates/polint-sdk/src/lib.rs` - SDK prelude that re-exports the core and diagnostics contract.
- `crates/polint-go/src/lib.rs` - Current consumer of core facts for Go extraction; useful for compatibility checks, not the main implementation target.
- `crates/polint-ts/src/lib.rs` - Current consumer of core facts for TS/JS extraction; useful for compatibility checks, not the main implementation target.
- `crates/polint-rules/src/lib.rs` - Current built-in rules that consume `RuleCtx`, capabilities, diagnostics, and rule options.
- `crates/polint-cli/src/main.rs` - Current integration point for `load_analysis_files`, parser diagnostics, `run_rules`, dedupe, and rendering.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-core/src/lib.rs`: Already defines typed IDs, `Language`, `Span`, `SourceFile`, `AnalysisDb`, `FunctionFact`, `ImportFact`, `BranchObligation`, `TestFact`, `CoverageFact`, TS facts, `Rule`, `Capabilities`, `RuleCtx`, `RuleRegistry`, `run_rules`, `rule_id_matches`, `span_from_byte_range`, and `line_col`.
- `crates/polint-diagnostics/src/lib.rs`: Already defines `Diagnostic`, `Severity`, `TextRange`, labels, suggestions, fixes, evidence, stable fingerprints, sorting, dedupe, and renderers.
- `crates/polint-fs/src/lib.rs`: Already sorts discovered files by root-relative path and loads an `AnalysisDb`.
- `crates/polint-sdk/src/lib.rs`: Re-exports the public core and diagnostics types through a prelude.
- Root test dependencies include `insta` and `proptest`, which fit Phase 3 snapshot/property requirements.

### Established Patterns
- Public data types derive `Serialize`/`Deserialize` where they cross output or cache boundaries.
- Fact insertion assigns typed IDs inside `AnalysisDb` push methods.
- Diagnostics are sorted and deduped after rule execution, including parallel execution.
- Parser adapters clone the file list before mutating `AnalysisDb`, then push facts through core APIs.
- Current code favors small structs and direct accessors over a query framework.

### Integration Points
- `polint-fs::load_analysis_files` creates the `AnalysisDb` consumed by Go/TS adapters and rule execution.
- `polint-go` and `polint-ts` push facts into `AnalysisDb`; Phase 3 changes must avoid breaking their existing callers unless updated in the same plan.
- `polint-rules` consumes `RuleCtx` accessors and diagnostic builders; Phase 3 diagnostics and runner changes should be verified against these rules.
- `polint-cli::collect_diagnostics` calls Go/TS analysis, builds rule options, invokes `run_rules`, dedupes diagnostics, and renders output.

</code_context>

<specifics>
## Specific Ideas

- Keep the implementation honest: heuristic facts and coverage placeholders should not imply exact semantic or runtime coverage.
- Favor deterministic behavior first: the same input/config/rules should produce the same file order, fact order, diagnostic order, and snapshots.
- Keep Phase 3 as shared infrastructure hardening. Later phases can add richer language facts without changing the core contract every time.

</specifics>

<deferred>
## Deferred Ideas

- Full Go semantic/type information remains v2 or later semantic work.
- Dynamic branch coverage remains future work; Phase 3 only keeps the placeholder model honest.
- Production SARIF-like CI completeness remains Phase 8.
- Cache persistence and deterministic parallel parse/rule execution performance work remain Phase 7.
- Broad clean/failing Go/TS fixtures remain Phases 4 and 5.

</deferred>

---

*Phase: 03-core-facts-and-diagnostics*
*Context gathered: 2026-04-28*
