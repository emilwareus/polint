# Phase 41: Public SDK Query Views and Agent Ergonomics - Context

**Gathered:** 2026-05-26
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 41 --auto`

<domain>
## Phase Boundary

Phase 41 delivers the deliberate promotion layer for validated public rule-authoring and agent-authoring ergonomics. It should choose a small set of analysis surfaces that have enough implementation, fixtures, docs, and benchmark evidence to become supported SDK views, query builders, or stable CLI JSON contracts.

This phase does **not** expose raw `AnalysisDb`, parser ASTs, provider stores, internal graph schemas, layer-cache/query internals, or a broad graph database API. It does not make every v1.2 private family public at once. Any advanced view or command that lacks fixtures, docs, stable JSON shape, bounded query semantics, cache/input discipline, and temp-repo public-SDK proof remains reserved, preview/experimental, hidden, or deferred.

</domain>

<decisions>
## Implementation Decisions

### Promotion Thresholds

- **D-01:** Promotion is evidence-gated, not roadmap-name-gated. A fact family may become public only when native fixtures, public no-leak tests, docs, deterministic cache/input behavior, precision/status semantics, and temp-repo external-rule tests all prove the contract.
- **D-02:** Prefer promoting conservative relationship/query surfaces first over broad internals. Existing supported `ResolvedImports<'_>`, `ModuleGraphFacts<'_>`, `Symbols<'_>`, `References<'_>`, and metric views should get ergonomic query-builder upgrades before exposing low-level call/data-flow/evidence internals.
- **D-03:** Advanced families such as calls, call graph, data flow, effects/summaries, evidence, paths, slices, type/value/alias, entrypoints, and extension/model facts should be promoted selectively. If a family cannot provide bounded, status-aware, documented query methods in this phase, keep its capability unsupported or mark the CLI path preview/experimental without stable SDK semantics.
- **D-04:** Keep capability names honest. Reserved capabilities must continue to emit `polint/capability` diagnostics until the corresponding public SDK view can read real facts and has docs/tests proving limits and setup behavior.

### Public SDK Query Shape

- **D-05:** Public rule authors should consume typed views and domain-specific query builders, not raw stores, raw graph rows, provider IDs, Datalog-like query strings, or internal relation tables.
- **D-06:** Query-builder methods must return iterators or bounded result handles that avoid cloning large source strings and avoid materializing unbounded graph/path results by default.
- **D-07:** Any expensive query must require an explicit limit, budget, or clearly named `.unbounded()`-style opt-in. Path, slice, data-flow, and call-graph queries must carry precision/status/unknown/budget evidence so a missing path never looks like proof of absence.
- **D-08:** The first public query ergonomics should extend existing views with methods such as import/module path helpers, symbol/reference lookup helpers, metric thresholds, and bounded samples before promoting whole-program call graph or data-flow paths.
- **D-09:** If `CallGraph<'_>`, `DataFlow<'_>`, `Cfg<'_>`, `Evidence<'_>`, or similar views are promoted in this phase, start with a narrow stable subset whose method names communicate cost and uncertainty. Leave lower-level debug rows and provider vocabulary private.

### Stable CLI JSON Contracts

- **D-10:** Stabilize only CLI commands whose output shape is useful to agents today and supportable as a public product contract. Candidate commands include `polint inspect rule --format json`, `polint test`, and narrow `polint facts` / `polint unknowns` / `polint explain` paths if the implementation is already real and bounded.
- **D-11:** Public JSON must include schema version, command/tool version, requested scope, limits/budgets, stable IDs where available, precision/status labels, setup gaps, and deterministic ordering. It must avoid raw source bodies, absolute workspace paths, parser object IDs, timestamps in hash identity, private provider IDs unless explicitly documented, and unstable internal debug names.
- **D-12:** `polint eval` remains internal/hidden/preview unless Phase 41 intentionally stabilizes a narrow report contract from Phase 40. Benchmark scorecards can inform promotion, but the internal eval schema should not accidentally become public API.
- **D-13:** Public command docs must say whether a command is stable, preview, experimental, or internal. Preview/experimental output may exist for agent feedback, but stable docs and README examples should not overclaim it.

### Agent Authoring Workflow

- **D-14:** Agent ergonomics should preserve the artifact boundary: rules emit diagnostics from existing typed views; model packs teach framework/API semantics; summaries describe reusable function/API behavior; provider extensions emit validated facts when declarative models are insufficient; fixtures/benchmarks prove the result.
- **D-15:** Generated rules and examples must continue to use `use polint::sdk::prelude::*`, `#[polint::rule]`, derived capabilities from typed parameters, and `polint::runner::run_cli`. Do not add manual `impl Rule`, handwritten capability declarations, internal imports, or broad `RuleCtx` fact access as an escape hatch.
- **D-16:** `RuleCtx` stays narrow: diagnostics, options/settings, path/context helpers, and capability/setup metadata. Query access belongs on typed SDK views passed into the rule function.
- **D-17:** Improve scaffolding only where it produces real, testable artifacts: generated rule modules, positive/negative fixtures, `polint-test.toml`, docs/README stubs with precision and limitation notes, and deterministic snapshots. Do not generate placeholder advanced rules that request unsupported facts without an actionable capability diagnostic.
- **D-18:** Agent-facing inspect/explain output should help choose the right artifact type by reporting setup gaps, unknowns, precision limits, unsupported capabilities, and suggested next artifact category: rule, model, summary, provider extension, or fixture.

### Docs, Tests, and Compatibility Gates

- **D-19:** Every promoted public fact view or command needs docs under `docs/facts/` or a relevant user doc that explains limits, precision tiers, heuristic behavior, setup-missing behavior, budget semantics, and examples using public SDK imports only.
- **D-20:** Add at least one temp-repo style test for every new public rule-authoring feature. The test must compile generated `.polint/rules`, import only `polint::sdk::prelude::*`, consume real facts through typed parameters, run through `polint check --format json`, and assert diagnostics or command JSON.
- **D-21:** Public no-leak proof must remain active for private vocabularies from calls, refined calls, summaries, demand queries, extensions, framework facts, type/value/alias, data flow, evidence, benchmarks, and eval.
- **D-22:** Existing public behavior must remain compatible unless a breaking change is explicit and documented. Current examples and SDK docs should keep working while advanced views are added behind curated surfaces.

### The Agent's Discretion

- The planner may decide the exact first set of promoted query builders after inspecting code and tests, but should bias toward improving existing supported views before promoting new broad analysis families.
- The planner may choose exact stability labels and JSON schema module names, provided stable/preview/experimental/internal status is visible in docs and tests.
- The planner may split Phase 41 into slices such as SDK ergonomic upgrades, reserved-capability promotion audit, public command JSON stabilization, scaffold/test workflow hardening, docs/facts alignment, and public-boundary/no-leak proof.
- The planner may leave `CallGraph<'_>`, `DataFlow<'_>`, `Cfg<'_>`, `Evidence<'_>`, or other advanced views unsupported if they cannot meet the promotion threshold in this phase.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 41 goal, SAE-PROM-02 mapping, research references, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-PROM-02 requirement text and v1.2 boundaries.
- `.planning/PROJECT.md` — Product boundary, private-analysis-first milestone intent, agent-extensible thesis, and public API discipline.
- `.planning/STATE.md` — Current milestone state, Phase 40 completion, and accumulated v1.2 decisions.
- `research/ROADMAP.md` — PR 22 public SDK query views, promotion dependency chain, and research lens for native inference versus agent-authored augmentation.

### Rule and Agent Authoring Research

- `research/agent-rule-authoring/FINAL-REPORT.md` — Typed Rust rules, artifact boundary, agent feedback loop, model/provider/fixture distinction, and public SDK shape.
- `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md` — Rule manifest, narrow `RuleCtx`, agent inspect commands, `polint test`, `polint new-rule`, domain query builders, model packs, provider extensions, preview gates, and acceptance criteria.
- `research/agent-rule-authoring/VALIDATION.md` — Validation expectations for agent-authored rules, manifests, fixtures, generated code, and public SDK behavior.
- `.claude/skills/polint/SKILL.md` — Current user-facing rule authoring workflow and stable fact-view guidance.

### Promotion and Visibility Inputs

- `.planning/phases/40-external-benchmark-adapters-and-promotion-gates/40-CONTEXT.md` — Promotion-gate evidence, benchmark report boundaries, adapted-run quality/cost reporting, and Phase 41 deferrals.
- `.planning/phases/39-slicing-paths-and-evidence-bundles/39-CONTEXT.md` — Evidence/path contracts, bounded path behavior, renderer semantics, and public view deferrals.
- `.planning/phases/38-local-plus-summary-projected-data-flow/38-CONTEXT.md` — Data-flow facts, source/sink/sanitizer/barrier models, bounded query-scoped path search, unknowns, budgets, and public view deferrals.
- `.planning/phases/37-refined-call-graph-providers/37-CONTEXT.md` — Refined call graph facts, direct-versus-refined deltas, public view deferrals, and no-leak constraints.
- `.planning/phases/36-p0-type-value-place-alias-substrate/36-CONTEXT.md` — Type/value/access-path/points-to/alias precision, budget facts, and public view deferrals.
- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Entrypoints, trust boundaries, framework dispatch, extension overlays, and public view deferrals.
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Repo-local extension host, typed sinks, validation, precision ceilings, cache quarantine, and public ergonomics deferrals.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand-query trace/cache/quarantine substrate for expensive bounded public queries.
- `docs/API-VISIBILITY-PLAN.md` — Visibility discipline and Phase 40 note that eval/query promotion is deferred to Phase 41.
- `AGENTS.md` — Public API visibility discipline, rule-authoring platform contract, generated skill/docs constraints, and GSD workflow expectations.

### Existing Public SDK and CLI Surface

- `crates/polint/src/sdk/mod.rs` — Current public SDK prelude, hidden generated-code helpers, and supported rule-author exports.
- `crates/polint/src/sdk/facts.rs` — Current typed fact views and query methods.
- `crates/polint-macros/src/lib.rs` — `#[polint::rule]` fact-view parameter parsing, canonical path enforcement, derived capability mapping, and manifest metadata generation.
- `crates/polint/src/analysis_plan.rs` — Capability support status, unsupported reserved capability diagnostics, setup handling, and plan digest inputs.
- `crates/polint/src/rule_manifest.rs` — Rule manifest model and inspect JSON contract.
- `crates/polint/src/rule_test.rs` — `polint test` fixture runner and temp-repo test behavior.
- `crates/polint/src/cli/mod.rs` — Public CLI command surface, help text, hidden/unstable command boundaries, and JSON output integration.
- `crates/polint/src/runner/mod.rs` — Public rule-pack entrypoint and runner contract.
- `crates/polint/tests/cli.rs` — External temp-repo rule tests, public no-leak tests, inspect/test coverage, reserved capability diagnostics, and generated-rule assertions.
- `docs/facts/README.md` — Current public fact documentation index.
- `docs/facts/data-flow.md` — Current reserved `DataFlow<'_>` documentation.
- `docs/facts/evidence.md` — Current internal diagnostic evidence documentation and future public view caveat.
- `docs/facts/resolved-imports.md`, `docs/facts/symbols-and-references.md`, and `docs/facts/metrics.md` — Examples of supported fact-view docs with precision/status guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/polint/src/sdk/mod.rs` already exports a curated `polint::sdk::prelude::*` with rule contracts, diagnostics, severity types, path-scope helpers, stable fact views, and hidden generated-code glue.
- `crates/polint/src/sdk/facts.rs` already implements query-style methods for supported views such as source files, functions, imports, resolved imports, module graph facts, symbols/references, literals/JSX, and metrics. These are the best first targets for ergonomic query-builder improvements.
- `crates/polint-macros/src/lib.rs` already enforces canonical SDK view paths, placeholder lifetime arguments, and derived capability mapping from rule parameters. It currently recognizes reserved views such as `Cfg`, `CallGraph`, and `DataFlow`, while analysis planning keeps their capabilities unsupported.
- `crates/polint/src/analysis_plan.rs` already emits `polint/capability` diagnostics for unsupported reserved capabilities and setup-missing hard capabilities. Phase 41 should update this only when a view becomes genuinely consumable.
- `crates/polint/src/rule_manifest.rs`, `polint inspect rule --format json`, and `polint test` already provide the agent-facing rule manifest and temp-repo fixture loop started in Phase 25.
- `crates/polint/tests/cli.rs` contains repeated temp-repo external-rule tests using `polint::sdk::prelude::*` and `polint::runner::run_cli`; use this as the mandatory gate pattern for new public SDK features.
- `docs/facts/data-flow.md` and `docs/facts/evidence.md` already document reserved/internal status. These docs must change only when promotion is intentional and backed by tests.

### Established Patterns

- Public API discipline is strict: `sdk` and `runner` are supported surfaces; `core`, parser adapters, analysis modules, eval, cache internals, and provider stores are implementation detail unless explicitly promoted.
- Rule authors request typed fact views in `#[polint::rule]` signatures; capabilities are generated, not handwritten.
- `RuleCtx` stays focused on diagnostics, options, source paths, and capability/setup metadata rather than broad fact access.
- Unsupported, setup-missing, unknown, ambiguous, budget-exceeded, partial, rejected, and heuristic states must be visible and honest.
- Public command output is treated as a product contract. Hidden/internal/preview commands must not be documented as stable.
- Examples are external consumers of the SDK, not privileged internal rule packs.
- New docs and generated skills must advertise only completed and supportable workflows.

### Integration Points

- Add or refine public methods in `sdk::facts` for existing supported views before widening capability support.
- Update `analysis_plan::support_for` only for fact families that have real public facts, docs, tests, and setup behavior.
- Update `polint-macros` capability mapping only when new public view names and canonical paths are stable; remove or keep reserved mappings according to the promotion plan.
- Extend `rule_manifest` and inspect JSON with any new public metadata fields such as stability, precision, limits, and option schema only when cache/output contracts are updated.
- Extend `rule_test` and generated `new-rule` scaffolding with positive/negative fixture generation only if the test runner already supports the necessary assertions.
- Add or update docs under `docs/facts/` and public README/example content in the same slice as any SDK promotion.
- Add no-leak tests for private analysis vocabulary and compatibility tests for stable JSON schemas.

</code_context>

<specifics>
## Specific Ideas

- Start Phase 41 with a promotion audit table that lists each reserved or candidate public surface: current implementation status, docs status, temp-repo test status, benchmark/fixture evidence, setup behavior, cache/input behavior, and recommended disposition (`stable`, `preview`, `experimental`, `internal`, or `defer`).
- Prefer upgrading existing public views first: `ResolvedImports<'_>`, `ModuleGraphFacts<'_>`, `Symbols<'_>`, `References<'_>`, `FileMetrics<'_>`, `FunctionMetrics<'_>`, and `ComplexityMetrics<'_>`.
- Candidate ergonomic additions include bounded symbol/reference lookups, import/module path helpers, metric threshold iterators, and stable unknown/setup inspection helpers.
- Keep `DataFlow<'_>` reserved unless Phase 41 can provide a narrow bounded API such as source/sink/path queries with explicit `.limit(...)`, unknown/budget markers, and temp-repo diagnostics proof.
- Keep `CallGraph<'_>` reserved unless Phase 41 can expose a narrow possible-target API that separates direct, refined, unresolved, dynamic, unsupported, and budgeted statuses.
- Keep evidence as renderer JSON rather than SDK fact view unless a rule-author use case is proven. A future `Evidence<'_>` view should not require rules to traverse raw evidence nodes/edges.
- Stabilize or improve `polint inspect rule --format json` and `polint test` before adding more agent CLI commands. If `polint facts`, `polint unknowns`, `polint explain`, or `polint diff` are added, scope them narrowly and version their JSON.
- Generated rule scaffolds should include at least one positive fixture, one negative fixture, and a limitations/precision note, but should not generate unsupported advanced rule parameters by default.

</specifics>

<deferred>
## Deferred Ideas

- Broad public graph database, Datalog/QL shell, or raw internal relation API.
- Whole-program unbounded call graph, data-flow, path, slice, or evidence traversal as a default public SDK behavior.
- Stable public `polint eval` JSON schema and benchmark command if Phase 41 cannot narrow and document the contract.
- Public model-pack or provider-extension authoring SDK if the current extension/model contracts are not ready for stable docs and external examples.
- Python/Java advanced SDK parity before Go and TS/JS promoted surfaces are stable.

</deferred>

---

*Phase: 41-public-sdk-query-views-and-agent-ergonomics*
*Context gathered: 2026-05-26*
