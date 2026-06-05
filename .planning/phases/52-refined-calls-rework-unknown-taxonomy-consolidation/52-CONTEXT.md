# Phase 52: Refined-Calls Rework & Unknown Taxonomy Consolidation - Context

**Gathered:** 2026-06-05
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 52 --auto`

<domain>
## Phase Boundary

Phase 52 delivers **GRAPH-05** and **TAX-01**: refined calls become a compatibility projection over unified solver output, and unsupported/unknown states become one consolidated taxonomy exposed through stable public JSON.

This phase should add:

1. A reworked `analysis::refined_calls::provider` whose canonical resolved/ambiguous call edges come from `polint.solver` derived edges.
2. Preservation of the existing private `RefinedCallEdgeFact` shape so downstream `data_flow`, `evidence`, internal eval, and any SDK-facing views that depend on refined-call behavior do not need a contract rewrite.
3. Removal or demotion of v1.2 heuristic refined-call producers (`framework`, `go`, `ts_js`, `summaries`, `extensions`) as primary derivation paths.
4. A private `analysis::unknown_taxonomy` module that consolidates setup, unsupported, missing-fact, out-of-scope, budget, sidecar, solver, model, and unresolved-call reasons into stable categories.
5. The roadmap-owned public CLI surface `polint inspect unknowns --format json`, with stable JSON backed by `docs/schemas/polint-unknowns-v1.json`.
6. Compatibility tests proving v1.2 refined-call fixtures, data-flow direct-call bridging, evidence rendering, public no-leak gates, and existing public agent JSON remain stable or document intentional improvements.

This phase explicitly does **not**:

- Add new solver algorithms or precision drivers. Go RTA, TS token propagation, TS object modeling, and adaptation model edges already landed in Phases 48-51.
- Perform the milestone-wide cache/budget audit. That is Phase 53.
- Enforce final benchmark promotion floors or final recall claims. That is Phase 54.
- Promote `CallGraph<'_>`, solver, semantic graph, refined-call internals, data-flow, or evidence internals to the public SDK.
- Add public graph/query/eval CLI surfaces. In v1.3, the only new public CLI surface is the unknown-taxonomy inspection path.

</domain>

<decisions>
## Implementation Decisions

### Refined-Call Projection Contract

- **D-01:** Treat `polint.solver` derived edges as the canonical source for refined dynamic call edges. `refined_calls::provider` should be a projection/compatibility layer, not a second independent call solver.
- **D-02:** Preserve the existing `RefinedCallEdgeFact` struct shape and semantics for downstream internal consumers. The planner may add private helper mappers, but should not rename/remove fields or force downstream `data_flow`/`evidence` rewrites.
- **D-03:** Keep direct-call mirroring only as a compatibility floor for existing direct/static `CallTargetFact` rows that are not solver-derived. All dynamic/solver-relevant edges should come from `DerivedEdgeFact` rows and their `DerivedEdgeProvenance`.
- **D-04:** Projection must carry provenance honestly. `input_stable_keys` and `evidence` should include the solver derived-edge stable key and contributing fact keys; do not collapse provenance to vague labels like only `"solver"`.
- **D-05:** Map solver status/precision through the existing call/refined-call vocabularies without laundering uncertainty. Budget-exceeded, unsupported, unresolved, setup-missing, and model/rejected states must remain visible.
- **D-06:** Stable keys should compose the solver derived-edge stable key plus projection tier/algorithm labels. Do not key projected refined calls by dense IDs, display strings alone, or provider iteration order.
- **D-07:** `RefinedCallTier` can be reused as a compatibility vocabulary, but the planner may add private mapping helpers if existing tier names do not exactly match solver provenance. Do not add public tier documentation that suggests a supported SDK call graph.
- **D-08:** The old primary producer modules (`framework`, `go`, `ts_js`, `summaries`, `extensions`) should either be deleted, reduced to solver-input/projection helpers, or covered by tests proving they no longer duplicate solver-derived edges. Avoid two sources producing the same semantic edge under different stable keys.

### Downstream Compatibility

- **D-09:** Data-flow direct-call bridging remains the main compatibility consumer. `analysis::data_flow::direct_calls::derive_direct_call_edges` should keep reading `db.refined_call_edges()` and should not need to know whether an edge came from v1.2 direct targets or v1.3 solver projection.
- **D-10:** Evidence should continue to derive from data-flow/evidence facts, not directly from solver internals. Any richer solver provenance that reaches evidence must pass through existing private evidence/data-flow fields or an explicitly planned internal bridge.
- **D-11:** Public no-leak tests remain mandatory. `RefinedCallEdgeFact`, solver types, semantic graph nodes/constraints, and unknown-taxonomy internals must stay out of `polint::sdk::prelude::*`, `polint check --format json`, README, and public fact docs unless already intentionally stable.
- **D-12:** Integration tests should include at least one Go RTA-derived edge, one TS token-derived edge, one TS object-model-derived edge when object model is enabled, and one adaptation `ModelEdge`-derived edge projecting into refined calls without changing downstream consumer code.
- **D-13:** Existing v1.2 refined-call fixtures should either remain byte-identical where the solver emits equivalent edges, or have documented snapshot deltas that show stricter provenance/unknown handling rather than recall flooding.

### Unknown Taxonomy Model

- **D-14:** Add a private `analysis::unknown_taxonomy` module as the single normalization boundary for provider unknowns. It should collect from existing public setup/resolution gaps plus internal graph-engine families rather than scattering category mapping across CLI code.
- **D-15:** Required top-level taxonomy categories are `SetupMissing`, `UnsupportedSemantic`, `MissingFact`, and `OutOfScope`, plus sidecar-specific `GoPackagesLoadFailed`, `GoVersionUnsupported`, and `GoSidecarTimeout`. The planner should include budget and rejected/model-missing cases either as stable subreasons or additional private categories mapped into the public schema without hiding them.
- **D-16:** Unknown rows must be actionable: each row should include capability/family, file/span when known, normalized status/category, reason, precision when available, source provider or fact family, docs path when available, and suggested artifact (`config`, `model`, `provider`, `rule`, or `none`) where useful.
- **D-17:** Unknown taxonomy rows are not diagnostics suppression. Providers should continue to emit their real internal facts/diagnostics; the taxonomy aggregates and normalizes them for inspection and agent follow-up.
- **D-18:** Unknown reduction from adaptation or solver improvements should preserve the original unknown rows in eval/audit contexts when needed for delta reporting. Do not destroy auditability to make current unknown counts look cleaner.

### Public CLI Surface

- **D-19:** Make `polint inspect unknowns --format json` the canonical Phase 52 public command because that is the roadmap and requirements contract.
- **D-20:** Preserve the existing stable `polint unknowns --cap ... --format json` path as a compatibility alias unless removing it is explicitly required by tests or CLI discipline. Existing docs and tests already advertise it; breaking it would be unnecessary churn.
- **D-21:** The `inspect unknowns` command should support consolidated workspace-level graph-engine unknowns. It may also support `--cap` filtering for compatibility, but the default Phase 52 value is the consolidated queue, not only one public fact view.
- **D-22:** Keep the JSON schema versioned through `docs/schemas/polint-unknowns-v1.json`. If the current schema cannot represent consolidated rows, update the schema and tests in the same plan slice.
- **D-23:** Human output is optional. The required stable contract is JSON. Avoid adding verbose default CLI output that becomes a broad public product promise.

### Cache, Ordering, And Validation

- **D-24:** `polint.refined_calls` cache identity must include the solver output digest. The current refined-call cache inputs include calls, entrypoints, summaries, type/value/alias, extensions, models, and tool components; Phase 52 should add solver digest participation directly.
- **D-25:** Projection output must remain deterministic under shuffled solver/input order. Sort by stable keys, assign dense IDs after sorting, and keep snapshot tests that prove byte stability.
- **D-26:** Validation should reject duplicate projected stable keys, dangling callsites/functions/symbols, exact precision claims for dynamic/model/solver-derived edges, malformed synthetic targets, and missing evidence/input keys.
- **D-27:** Unknown taxonomy output must also be deterministic: stable row order, normalized category strings, stable schema URL, and no absolute temp paths.

### Verification And Acceptance

- **D-28:** Add focused unit tests for solver-derived-edge-to-`RefinedCallEdgeFact` mapping, including provenance/evidence, status/precision, stable-key composition, duplicate handling, and budget/unsupported mapping.
- **D-29:** Add integration/eval fixtures proving downstream data-flow/evidence consume solver-projected refined calls without internal API promotion.
- **D-30:** Add CLI tests for `polint inspect unknowns --format json`, compatibility coverage for existing `polint unknowns --cap ... --format json`, unsupported/reserved capability behavior, and schema stability.
- **D-31:** Run the relevant filtered suites first (`refined_calls`, `solver`, `data_flow`, `evidence`, `unknowns`, public leak), then full `cargo test -p polint`, `cargo clippy -p polint --all-targets`, `cargo fmt --all -- --check`, and `git diff --check`.

### the agent's Discretion

- Exact module layout under `analysis::unknown_taxonomy`.
- Exact normalized row type and category enum names, provided public JSON remains stable and actionable.
- Whether compatibility aliasing is implemented by reusing `UnknownsArgs` or by moving unknowns under `InspectCommand` and calling a shared renderer.
- Natural plan slicing. A likely split is: (1) projection mapper + refined-call provider/cache rework, (2) downstream compatibility fixtures and removal/demotion of old heuristic producers, (3) unknown taxonomy model and aggregation, (4) public `inspect unknowns` CLI/schema/docs/tests and final verification.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 52 goal, dependencies, success criteria, and public CLI boundary.
- `.planning/REQUIREMENTS.md` - GRAPH-05 and TAX-01 requirement text plus v1.3 out-of-scope public graph/solver surfaces.
- `.planning/PROJECT.md` - Product constraints, v1.3 graph-engine precision goal, truthfulness posture, and internal-first discipline.
- `.planning/STATE.md` - Current milestone state, Phase 51 closeout caveat that refined-call projection and unknown taxonomy remain Phase 52-owned, and branch-protection admin note.
- `docs/API-VISIBILITY-PLAN.md` - Public surface discipline; `CallGraph<'_>` and `DataFlow<'_>` deferred; existing `polint unknowns` stable surface.

### Upstream Phase Context

- `.planning/phases/51-adaptation-model-layer/51-CONTEXT.md` - Accepted `ModelEdge` lowering, adaptation reporting, model/unknown deltas, and explicit deferral of refined-call projection to Phase 52.
- `.planning/phases/50-js-ts-object-property-prototype-this-model-driver/50-CONTEXT.md` - TS object-model solver driver, object budget/unknown posture, and Phase 52 refined-call/taxonomy deferral.
- `.planning/phases/49-js-ts-function-token-propagation-driver/49-CONTEXT.md` - TS token solver, `"too-many-tokens"` sentinel, and budget-exceeded unknown handoff.
- `.planning/phases/48-go-rta-driver/48-CONTEXT.md` - Go RTA derived-edge production, budget-exceeded honesty, and decision that Phase 52 reads solver edges into observable refined-call projection.
- `.planning/phases/47-unified-solver-core-derived-edge-provenance/47-CONTEXT.md` - Solver core, `DerivedEdgeFact`, `DerivedEdgeProvenance`, provider/cache order, and explain provenance seam.
- `.planning/phases/46-go-semantic-frontend-sidecar/46-CONTEXT.md` - Go sidecar setup/failure taxonomy, package-load/toolchain/version/timeout states, and cache participation.
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Identity-vs-unsupported categories and public-surface-leak gate.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/refined_calls/{provider.rs,facts.rs,store.rs,validate.rs,cache_key.rs}` - Current refined-call provider, contract, normalization/indexes, validation, and cache identity.
- `crates/polint/src/analysis/refined_calls/{framework.rs,go.rs,ts_js.rs,summaries.rs,extensions.rs}` - Existing v1.2 heuristic/assisted producers to delete, demote, or convert into solver-projection helpers.
- `crates/polint/src/analysis/solver/{facts.rs,store.rs,provider.rs,provenance.rs,policy.rs,engine.rs,budget.rs,cache_key.rs,validate.rs}` - Derived-edge source facts, provenance, status/precision, budget status, and solver output digest.
- `crates/polint/src/analysis/data_flow/direct_calls.rs` - Primary downstream refined-call consumer that must remain insulated from solver internals.
- `crates/polint/src/analysis/data_flow/{provider.rs,facts.rs,store.rs,validate.rs}` - Data-flow compatibility and cache dependency path.
- `crates/polint/src/analysis/evidence/{provider.rs,facts.rs,render.rs,store.rs,validate.rs}` - Evidence compatibility path and unknown/evidence rendering constraints.
- `crates/polint/src/analysis_kernel/{mod.rs,provider.rs,validation.rs,metadata.rs,debug.rs}` - Provider order slot (`polint.solver` before `polint.refined_calls`), output digest threading, validation, no-leak tests, and metadata fact-family labels.
- `crates/polint/src/cli/mod.rs` - Existing top-level `unknowns` command, `inspect rule` command, `UnknownsReport` rendering, schema URL constants, and private derived-edge explain seam.
- `crates/polint/tests/cli.rs` - Current CLI tests for `polint unknowns`, no-leak markers, facts/explain JSON, and docs command snippets.
- `docs/schemas/polint-unknowns-v1.json` - Stable unknowns JSON schema that Phase 52 must keep or update deliberately.
- `docs/CONSUMER-SETUP.md` and `crates/polint/src/cli/skill.rs` - Current public docs/skill text advertising `polint unknowns --cap references --format json`; update if canonical command text changes.

### Research And Public-Fact Docs

- `research/data-flow/implementation/BOOTSTRAP-INTEGRATION.md` - Call graph/data-flow cycle break, refined-call provider sequencing, unknown/havoc visibility, and unknown reduction auditability.
- `research/data-flow/RECOMMENDED_IMPLEMENTATION.md` - Unknown/havoc and unsupported dynamic behavior posture.
- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - v1.3 benchmark and graph-engine motivation.
- `docs/facts/symbols-and-references.md` - Public setup/resolution status language and unknown inspection precedent.
- `docs/facts/resolved-imports.md` - Public unknown/setup/dynamic import status precedent.
- `docs/facts/capability-plans.md` - Unsupported reserved capability behavior.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `DerivedEdgeFact` already carries source/target semantic nodes, `PointsToStatus`, `PointsToPrecision`, stable key, and full `DerivedEdgeProvenance`.
- `SolverStore::derived_edges()` and `edges_for_constraint_kind()` provide deterministic access to solver output.
- `RefinedCallEdgeFact` already contains enough compatibility fields for projection: site, caller, target function/symbol/synthetic target, algorithm, tier, status, reason, provenance, precision, validation, confidence, evidence, input stable keys, and stable key.
- `RefinedCallStore` already normalizes and indexes by site, caller, target, status, algorithm, provenance, and tier.
- `analysis::data_flow::direct_calls` already consumes `db.refined_call_edges()` as the call boundary, so preserving that store contract shields later data-flow/evidence code from solver internals.
- `UnknownsReport`, `UnknownsRow`, `POLINT_UNKNOWNS_JSON_SCHEMA_V1_URL`, and current public fact-view unknown reporting already exist in `cli/mod.rs`.
- Public no-leak tests already look for `polint.refined_calls` and `RefinedCall` markers in public output/docs.

### Established Patterns

- All graph-engine internals stay `pub(crate)`.
- Dense IDs are assigned after stable-key sorting.
- Dynamic or model-derived edges never claim exact precision.
- Provider output digests include upstream provider digests, normalized stable keys, status/precision/provenance fragments, config, lifecycle/tool/model/model-file components, and budget inputs.
- Public JSON schemas are versioned and tested through CLI integration tests.
- Existing public command compatibility matters; avoid breaking `polint unknowns --cap ...` without a strong reason.

### Integration Points

- `AnalysisKernel::run` already executes `polint.solver` before `polint.refined_calls`; Phase 52 should use the solver output digest directly in refined-call provider/cache wiring.
- `analysis_kernel::provider` manifests already declare `polint.solver` output `solver_derived_edges` and `polint.refined_calls` output `refined_call_edges`.
- `analysis_kernel::validation` runs refined-call validation after provider execution; use this path to catch projection mistakes.
- CLI command routing currently supports `Command::Unknowns` and `InspectCommand::Rule`; add `InspectCommand::Unknowns` or a shared unknowns renderer to make the roadmap command real.

</code_context>

<specifics>
## Specific Ideas

- The first projection fixture should use an existing Go RTA solver-derived edge and assert it appears as a `RefinedCallEdgeFact` with solver/provenance evidence while data-flow direct-call edges appear unchanged.
- A TS fixture should prove a function-token or object-model solver edge projects into refined calls when the relevant solver feature is enabled, without making `CallGraph<'_>` public.
- Unknown taxonomy fixtures should include: missing resolver setup, unsupported reserved capability, Go package-load failure, Go unsupported version, Go sidecar timeout, solver budget exceeded, TS token too-many-tokens, object-model computed-property unknown, adaptation rejected model, and missing model/source fact.
- CLI docs should prefer `polint inspect unknowns --format json` after Phase 52, but retain a compatibility note for `polint unknowns --cap references --format json` if the alias remains.
- If the JSON schema needs extension, keep old fields (`file`, `span`, `status`, `reason`, `precision`, `docs_path`, `suggested_artifact`) and add optional fields instead of breaking existing consumers.

</specifics>

<deferred>
## Deferred Ideas

- Milestone-wide cache and budget proof across every v1.3 fact family remains Phase 53.
- Hard Go/Jelly precision floors, F-score beta=0.5, per-language deltas, polyglot canary as a hard gate, and final recall claims remain Phase 54.
- Public `CallGraph<'_>`, `DataFlow<'_>`, semantic graph, solver, and evidence SDK views remain v1.4+ or later promotion work.
- Public `polint inspect graph`, `polint query`, `polint eval`, or solver debug commands remain out of v1.3.

</deferred>

---

*Phase: 52-Refined-Calls Rework & Unknown Taxonomy Consolidation*
*Context gathered: 2026-06-05*
