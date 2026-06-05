# Phase 53: Cache & Solver Budgets Consolidation - Context

**Gathered:** 2026-06-05
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 53 --auto`

<domain>
## Phase Boundary

Phase 53 delivers **CACHE-01** and **CACHE-02**: every v1.3 graph-engine fact family must have uniform, auditable cache-key participation and solver-budget behavior, backed by positive and negative fixtures that prove the right layers invalidate and preserve cache hits.

This phase should add:

1. A milestone-wide dependency-index audit for the new v1.3 families: semantic graph, Go semantic sidecar, solver core, Go RTA, TS tokens, TS object model, adaptation models, refined calls, and unknown taxonomy where it consumes those outputs.
2. A single cache-dependency ledger or equivalent internal test fixture source that makes required digest inputs explicit: sidecar binary/source digest, Go toolchain and `x/tools` versions, Go lifecycle config, adaptation model files and validated model rows, solver budgets, upstream provider output digests, algorithm/schema versions, budget status, and stable output keys.
3. Must-invalidate fixtures that mutate one behavior-affecting input at a time and prove the affected downstream layer recomputes.
4. Must-preserve-hit fixtures that mutate irrelevant inputs or reorder deterministic inputs and prove cache hits remain valid.
5. Consolidated budget enforcement across token-set size, property abstraction, dynamic-call fanout, adaptation model expansion, package/lifecycle depth where applicable, and solver worklist/iteration caps.
6. Benchmark report columns for cold/warm RSS thresholds without turning Phase 53 into the final promotion gate.

This phase explicitly does **not**:

- Add new call-graph precision algorithms. Go RTA, TS token propagation, TS object modeling, adaptation models, and refined-call projection already landed in Phases 48-52.
- Change public SDK surfaces or promote solver/cache internals to rule authors.
- Enforce final Go/Jelly precision floors, F-score beta=0.5, per-language deltas, or final v1.3 recall claims. That remains Phase 54.
- Replace the existing layer-cache architecture with a new cache subsystem.
- Convert every benchmark metric into a public CLI contract. The acceptance target is internal eval/benchmark reporting needed for Phase 54.

</domain>

<decisions>
## Implementation Decisions

### Cache Dependency Ledger

- **D-01:** Consolidate cache-key participation by creating a single internal dependency ledger or test-owned fixture matrix for v1.3 families. The ledger may be code, test data, or a documented helper, but it must be executable enough to fail when a required digest input is omitted.
- **D-02:** Treat existing provider-specific cache helpers as the source material, not as final proof. `analysis::solver::cache_key`, `go::semantic::cache_key`, `analysis::semantic_graph::cache_key`, `analysis::adaptation::cache_key`, `analysis::refined_calls::cache_key`, and related provider output digests should be audited together.
- **D-03:** Required behavior-affecting inputs include: sidecar binary/source digest, Go toolchain version, `x/tools` version, Go lifecycle config, Go syntax output digest, adaptation model file paths and normalized model contents, accepted/rejected model status, solver budget knobs, object-model enablement, upstream provider output digests, algorithm/schema labels, budget status, and stable output keys/provenance fragments.
- **D-04:** Do not duplicate digest recipes into a parallel manual list that can drift. If a ledger is introduced, it should call or reconstruct the same helpers production uses and assert their behavior through focused mutations.
- **D-05:** Prefer explicit schema/algorithm labels over implicit structural hashing. When a row vocabulary or derivation algorithm changes, a named version string should move and tests should fail until the cache recipe is updated.

### Invalidation Fixture Strategy

- **D-06:** Use paired positive and negative fixtures. Positive fixtures mutate one relevant input and require the appropriate downstream layer to miss/recompute; negative fixtures reorder inputs or mutate irrelevant fields and require cache hits to be preserved.
- **D-07:** Positive fixtures should cover at least these lanes: Go sidecar digest, Go toolchain or `x/tools` version, Go lifecycle build tags/package patterns/include-tests, solver Go budget, solver JS token budget, solver JS object budget and enablement flag, adaptation model file content, adaptation model validation status, semantic graph upstream digest, solver output digest into refined calls, and budget-exceeded status.
- **D-08:** Negative fixtures should prove deterministic reordering and irrelevant-field changes do not poison cache reuse: model file order, stable row order, Go lifecycle fields that are intentionally not digest-relevant, and no-op config changes that normalize to the same effective budget.
- **D-09:** Cache tests should verify the observed cache behavior, not just digest inequality. Where the layer cache supports stats/manifests, assert hits/misses/recomputes/writes or manifest replacement in addition to direct digest changes.
- **D-10:** Keep fixture scope small and native. Use temp-repo or existing eval fixture shapes rather than external corpora; Phase 54 owns external benchmark gates.

### Budget Taxonomy And Enforcement

- **D-11:** Keep `BudgetStatus::BudgetExceeded` as the unified top-level signal. Do not create separate public-facing budget enums for each driver.
- **D-12:** Preserve sub-budget specificity internally. Budget evidence should identify whether exhaustion came from cross-domain solver steps, Go RTA candidate fanout/worklist/rounds/address-taken caps, TS token fanout/token/worklist caps, TS object property/prototype/receiver/object caps, adaptation model expansion caps, or package/lifecycle bounds where applicable.
- **D-13:** Budget exhaustion must remain an output fact or report row with stable evidence, not a silent precision drop. Edges derived before a cap was hit may remain, but the run must carry the budget-exceeded reason.
- **D-14:** Solver budget changes must invalidate all derived outputs whose behavior can change. This includes downstream refined-call projection and any eval/unknown rows that reflect budget status.
- **D-15:** Effective-budget normalization matters. Existing positive-only config overlay behavior means absent and zero/invalid overrides often normalize to defaults; cache fixtures should assert the effective budget, not raw TOML text, is what participates.

### Benchmark RSS Reporting

- **D-16:** Add cold/warm RSS threshold columns to internal benchmark/eval reporting as required Phase 53 evidence. The columns should be deterministic and schema-tested, with absent measurements represented explicitly rather than omitted unpredictably.
- **D-17:** Keep RSS thresholds advisory or fixture-gated for this phase unless an existing benchmark runner already has stable local measurement support. Phase 54 owns final promotion enforcement.
- **D-18:** Do not compare transient absolute temp paths, timestamps, or machine-specific labels in benchmark report snapshots. Follow existing markdown/report renderer determinism patterns.

### Scope And Public Surface

- **D-19:** Keep all new dependency-ledger, budget-evidence, and cache-audit helpers `pub(crate)` or test-only. Do not extend `polint::sdk::prelude::*`, public rule APIs, README promises, or public CLI JSON unless the roadmap already made them public.
- **D-20:** Existing public `polint inspect unknowns --format json` from Phase 52 may include budget-related unknowns, but Phase 53 should not add a second public budget inspection command.
- **D-21:** Update internal docs/comments and generated agent guidance only when they describe real behavior. Do not claim exact benchmark precision or corpus-level recall improvements before Phase 54.

### Verification And Acceptance

- **D-22:** Add a focused cache-consolidation test suite that can be run before the full workspace regression. It should cover digest participation and observable hit/miss behavior across the v1.3 family set.
- **D-23:** Add regression tests for budget-exceeded reason stability and downstream digest invalidation when budget status changes.
- **D-24:** Keep Phase 43 determinism gates, Phase 42 public-surface leak tests, and Phase 52 unknown/refined-call compatibility tests green.
- **D-25:** Run targeted tests first (`cache_key`, `solver`, `go_semantic`, `adaptation`, `refined_calls`, eval report/markdown), then `cargo test -p polint`, `cargo clippy -p polint --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`.

### the agent's Discretion

- Exact ledger shape: Rust tests, TOML fixture matrix, internal helper module, or a combination.
- Exact names for budget-reason strings, provided they are stable, specific, and deterministic.
- Whether RSS threshold fields live in `EvalPerformanceReport`, `MetricSections::performance`, benchmark comparison rows, or a minimal new internal report section.
- Natural plan slicing. A likely split is: (1) dependency ledger and cache-key audit, (2) positive/negative invalidation fixtures, (3) budget evidence/reason consolidation, (4) RSS report columns and final regression/roadmap closeout.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 53 goal, dependencies, success criteria, and Phase 54 boundary.
- `.planning/REQUIREMENTS.md` - CACHE-01 and CACHE-02 requirement mapping plus BENCH-01 deferral.
- `.planning/PROJECT.md` - Product constraints, performance/reliability/truthfulness posture, and internal-first graph-engine discipline.
- `.planning/STATE.md` - Current v1.3 milestone state, completed Phase 52 status, deferred repo-admin leak-gate action, and accumulated cache/budget decisions.

### Upstream Phase Context

- `.planning/phases/47-unified-solver-core-derived-edge-provenance/47-CONTEXT.md` - Solver budget model, `BudgetStatus::BudgetExceeded`, solver provider cache recipe, and explicit Phase 53 cache/budget deferral.
- `.planning/phases/48-go-rta-driver/48-CONTEXT.md` - Go RTA sub-budget knobs, Go sidecar/RTA-signal cache participation, iteration/fanout budget evidence, and polyglot/determinism expectations.
- `.planning/phases/49-js-ts-function-token-propagation-driver/49-CONTEXT.md` - TS token sub-budget, `"too-many-tokens"` sentinel, JS budget config overlay, and token budget-exceeded fixtures.
- `.planning/phases/50-js-ts-object-property-prototype-this-model-driver/50-CONTEXT.md` - Object-model enablement flag, object/property/prototype/receiver budgets, and object-model cache digest decisions.
- `.planning/phases/51-adaptation-model-layer/51-CONTEXT.md` - Adaptation model file/fact digests, model expansion budgets, accepted/rejected model status, and held-out/cache delta reporting.
- `.planning/phases/52-refined-calls-rework-unknown-taxonomy-consolidation/52-CONTEXT.md` - Refined-call projection over solver output, solver digest participation, and consolidated budget/unknown taxonomy compatibility.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/solver/budget.rs` - Unified `SolverBudget`, `BudgetStatus`, Go/JS/object/adaptation sub-budget structs, and default budget contract.
- `crates/polint/src/analysis/solver/cache_key.rs` - Solver provider parameter digest, algorithm-version strings, budget digest parts, and locked recipe tests.
- `crates/polint/src/analysis/solver/provider.rs` - Production solver derivation, output digest construction, cache stats, and downstream dependency path.
- `crates/polint/src/analysis/semantic_graph/cache_key.rs` - Semantic graph provider parameter digest and documented dependency-index present/deferred inputs.
- `crates/polint/src/analysis/semantic_graph/provider.rs` - Semantic graph output digest and upstream provider digest folding.
- `crates/polint/src/go/semantic/cache_key.rs` - Go semantic sidecar digest, Go version, `x/tools` version, lifecycle digest, and positive/negative input digest tests.
- `crates/polint/src/go/semantic/process.rs` - Go frontend digest and local Go toolchain version discovery.
- `crates/polint/src/go/semantic/provider.rs` - Go semantic output digest, sidecar lifecycle/error digest inputs, and RTA-signal digest participation tests.
- `crates/polint/src/analysis/adaptation/cache_key.rs` - Adaptation model digest over schema, validator, budget, and model store digest parts.
- `crates/polint/src/analysis/adaptation/{budget.rs,store.rs,loader.rs,validate.rs}` - Model expansion caps, deterministic store parts, model file loading, and accepted/rejected validation status.
- `crates/polint/src/analysis/refined_calls/cache_key.rs` - Refined-call cache identity and solver output digest participation.
- `crates/polint/src/analysis/unknown_taxonomy/{facts.rs,collect.rs}` - Consolidated budget/setup/model/unsupported row collection from Phase 52.
- `crates/polint/src/config/mod.rs` - `[solver]`, `[solver.go]`, `[solver.js]`, object-model config, and positive-only effective-budget overlay tests.
- `crates/polint/src/eval/report.rs` - Internal evaluation schema, metric sections, solver metrics, performance metrics, and report normalization.
- `crates/polint/src/eval/markdown.rs` - Deterministic markdown renderer, provider cache stats table, adaptation section, and no-transient-path snapshot pattern.
- `crates/polint/src/eval/performance.rs` - Provider cache/runtime performance report structures used by eval output.
- `crates/polint/src/eval/{go_rta.rs,ts_tokens.rs,ts_object_model.rs,adaptation.rs,determinism_gate.rs}` - Existing budget, determinism, adaptation, and native fixture gates to extend or mirror.
- `tests/eval-fixtures/{determinism,go-rta,ts-tokens,ts-object-model,adaptation-model,refined-calls}` - Existing local fixtures for budget and cache proof seeds.
- `crates/polint/tests/public_surface_leak.rs` - Public API leak gate; Phase 53 must not extend the public prelude.

### Research And Docs

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - v1.3 benchmark motivation, graph-engine precision context, and final promotion path.
- `research/evaluation-harness/STANDARD.md` - Native fixture and observed-output conventions.
- `docs/API-VISIBILITY-PLAN.md` - Public/internal boundary discipline for graph-engine work.
- `docs/CONSUMER-SETUP.md` - Public setup/unknown guidance that must remain truthful if budget unknown behavior is mentioned.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `SolverBudget` already carries cross-domain, points-to, Go RTA, JS token, JS object-model, and adaptation sub-budgets with finite defaults.
- `BudgetStatus` already provides the stable `within_budget`, `budget_exceeded`, and `not_run` tags.
- `solver_provider_parameter_digest` already folds algorithm labels and every current budget knob into a locked digest recipe.
- `go_semantic_input_digest` already covers sidecar digest, Go version, `x/tools` version, upstream Go syntax digest, and lifecycle inputs.
- `adaptation_model_digest` already folds schema, validator version, budget parts, and deterministic model store parts.
- Eval performance reports already contain provider cache stats and runtime observations, giving RSS reporting an existing report area to extend.
- Existing determinism and native fixture tests already exercise Go RTA, TS tokens, TS object model, adaptation, refined calls, and unknown taxonomy.

### Established Patterns

- Cache recipes use frozen schema/algorithm labels plus stable ordered parts.
- Digest tests reconstruct exact expected parts lists as trip-wires.
- Effective config is normalized before budgets are built; zero or absent caps fall back to defaults under existing config patterns.
- Budget exhaustion is explicit and deterministic. It is not treated as a panic, silent truncation, or exact result.
- Dense IDs and rows are assigned or rendered after stable-key sorting.
- Public API discipline remains strict: new graph-engine internals stay `pub(crate)` and out of `polint::sdk::prelude::*`.

### Integration Points

- The production cache proof likely crosses `analysis_kernel::incremental`, provider manifests, provider output digests, and layer-cache manifests rather than only individual `cache_key.rs` unit tests.
- `AnalysisKernel::run` is the end-to-end path for observing provider cache behavior across semantic graph, solver, refined calls, unknown taxonomy, and eval output.
- Eval report/markdown rendering is the likely home for RSS threshold display because Phase 54 will consume benchmark reports.
- `config::SolverConfig` is the source of effective budget values; tests should avoid raw TOML string comparisons where normalization matters.

</code_context>

<specifics>
## Specific Ideas

- A useful first cache fixture is a temp repo where only a `[solver.js]` token cap changes; solver digest, refined-call output digest, and unknown taxonomy budget rows should change, while unrelated syntax facts preserve hits.
- A Go fixture should mutate build tags or package patterns and prove `polint.go.semantic` invalidates through `go_semantic_lifecycle_digest`; a mutation to `files_without_module_root` should preserve the hit if it remains intentionally irrelevant.
- An adaptation fixture should change one `.polint/models/*.toml` target and prove accepted/rejected model digest, semantic graph `ModelEdge` constraints, solver output, and report delta invalidate.
- A negative model fixture should reorder model facts or files and prove the same digest/hit result.
- Budget-reason strings should be specific enough for triage, such as `solver.max_steps`, `go.max_candidates_per_callsite`, `js.max_tokens_per_var`, `object.max_prototype_depth`, and `adaptation.max_model_derived_edges`.
- RSS columns can start as `cold_rss_threshold_mb`, `cold_rss_observed_mb`, `warm_rss_threshold_mb`, and `warm_rss_observed_mb` or equivalent internal names, provided snapshots are deterministic and missing data is explicit.

</specifics>

<deferred>
## Deferred Ideas

- Phase 54 owns final benchmark promotion enforcement, hard precision floors, F-score beta=0.5, per-language deltas, polyglot canary as an exit gate, and final v1.3 recall claims.
- Public SDK views for call graphs, data flow, semantic graph, solver, budget evidence, or cache internals remain out of v1.3.
- New external benchmark corpus work should wait for Phase 54 unless a tiny local fixture is needed to prove Phase 53 cache or budget behavior.

</deferred>

---

*Phase: 53-Cache & Solver Budgets Consolidation*
*Context gathered: 2026-06-05*
