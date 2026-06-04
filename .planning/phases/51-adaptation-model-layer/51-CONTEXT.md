# Phase 51: Adaptation Model Layer - Context

**Gathered:** 2026-06-04
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 51 --auto`

<domain>
## Phase Boundary

Phase 51 delivers **ADAPT-01** and **ADAPT-02**: a private adaptation-model layer that lets polint accept repo-local, validated framework/native model facts as solver constraints, plus benchmark adapted-mode reporting that proves the adaptation is useful without leaking benchmark oracle labels.

This phase should add:

1. A private `analysis::adaptation/` layer with a TOML model schema, loader, validator, deterministic store/indexes, and semantic-graph lowering.
2. Model fact fields for source pattern, target pattern, confidence, language, scope, and evidence.
3. Validation that confirms every target resolves in the semantic graph before facts are accepted.
4. Emission of `ModelEdge` constraints only for accepted, validated facts.
5. `benchmark adapted` reporting with prompt hash, changed model files, accepted/rejected facts, unknown delta, precision/recall delta, runtime/cache delta, and held-out subset deltas.
6. A sandboxed adaptation-agent workflow that cannot read benchmark oracle files.
7. Fixtures proving oracle-path prompt sanitization, exact-oracle RHS rejection, broad/wildcard-pattern rejection, and accepted/rejected model-edge behavior.

This phase explicitly does **not**:

- Rework `refined_calls::provider` to project over solver output. That is GRAPH-05, Phase 52.
- Consolidate the unsupported/unknown taxonomy or expose `polint inspect unknowns --format json`. That is TAX-01, Phase 52.
- Perform the milestone-wide cache/budget sweep for every v1.3 family. That is CACHE-01/CACHE-02, Phase 53.
- Enforce hard per-suite precision floors, F-score beta=0.5, or final promotion gates. That is BENCH-01, Phase 54.
- Add broad native-callable shim libraries such as `Array.prototype.map` / `Promise.then` by default. That is ADAPT-FUT-01 unless a minimal fixture is needed to prove the schema.
- Add reflection or dynamic-import auto-modeling. That is ADAPT-FUT-02.
- Promote adaptation, model, solver, or graph internals to the public SDK, runner, crate-root API, README workflow, or public CLI JSON outside the explicitly scoped benchmark adapted mode.

</domain>

<decisions>
## Implementation Decisions

### Model Fact Schema And Scope

- **D-01:** Add a private `analysis::adaptation` module, likely under `crates/polint/src/analysis/adaptation/`. Every new model fact, ID, store, validator, loader, and graph-lowering helper stays `pub(crate)`.
- **D-02:** The TOML schema is the primary repo-local model format for this phase. It must encode source pattern, target pattern, confidence, language, scope, evidence, and enough stable identity material to validate against semantic graph nodes. Exact field names are planner discretion, but every field that can affect model behavior must be deterministic and digest-participating.
- **D-03:** Model facts are not benchmark answer keys. They describe source-evident repository semantics such as framework/native dispatch edges, callbacks, lifecycle hooks, or other modelable call relationships that native analysis cannot derive yet.
- **D-04:** Accepted model facts lower into the existing semantic-graph constraint vocabulary through `ConstraintKind::ModelEdge` or a planner-chosen payload refinement that preserves the closed vocabulary. Do not create a parallel public graph or call-edge family.
- **D-05:** Model identities compose existing Phase 42/44/45/46/50 stable identities. Do not key model facts by display strings alone, run-local IDs, raw source slices, benchmark case IDs, or expected-label filenames.
- **D-06:** Confidence must be explicit and honest. Default accepted model precision should be heuristic/setup-aware at most, never exact, unless an existing internal precision ceiling and evidence contract proves otherwise.

### Validation And Anti-Oracle Guardrails

- **D-07:** Validation is fail-closed. A model fact is accepted only if its target resolves to an existing semantic graph node under the configured language/scope and its source evidence resolves to a known callsite/source location or supported model source. Non-resolving targets are rejected with deterministic rejection reasons.
- **D-08:** Reject wildcard or broad-pattern models that could flood recall. Patterns must be concrete enough to produce bounded, reviewable target sets. A model-expansion cap should produce explicit rejected/budget evidence rather than broad accepted facts.
- **D-09:** Reject model facts whose RHS exactly matches benchmark oracle expectations or answer-key labels. The validator should include a native fixture where an oracle-shaped RHS is present and deterministically rejected.
- **D-10:** The adaptation agent sandbox must not be able to read `research/evaluation-harness/repos/*/expected*`, `research/evaluation-harness/suites/*.toml` expected-label paths, suite-native answer keys, Jelly JSON edge oracles, Go x/tools `WANT` comments, or generated expected-label files. The prompt-sanitizer fixture must prove forbidden paths are not surfaced to the agent prompt or workspace.
- **D-11:** Allowed adaptation-agent inputs are source files, package/build/config/test files, baseline polint output, unresolved/unsupported call facts, unknown counts, accepted/rejected model facts, and non-oracle suite metadata needed to run the case. Forbidden inputs must be recorded explicitly in the adaptation record.
- **D-12:** If an adaptation attempt changes no files, it must record a no-change reason. Existing `eval::adaptation::AdaptationRecord` already enforces this shape; Phase 51 should reuse or extend it rather than inventing a second report model.

### Benchmark Adapted Mode Reporting

- **D-13:** Add or complete a `benchmark adapted` mode that records prompt hash, prompt path, changed model files, changed file digests, allowed/forbidden inputs, accepted/rejected model facts, unknown delta, precision/recall delta, runtime/cache delta, and held-out subset deltas.
- **D-14:** Reuse the existing internal eval structures where they fit: `eval::adaptation::AdaptationRecord`, `eval::delta::AdaptationDeltaReport`, `MetricSections::adaptation`, `EvaluationMode::PolintAgentAdapted`, and markdown rendering. Extend them only where Phase 51 acceptance fields are missing.
- **D-15:** Adapted reports compare a baseline run and an adapted run. Deltas must be deterministic and case-sorted. Improvements that add false positives must be visible through `new_false_positives` and precision deltas, not hidden behind recall gains.
- **D-16:** Held-out subset reporting is required. The planner may choose the minimal v1 shape, but the report must separate model-selection cases from held-out validation cases so adaptation cannot pass by overfitting the exact examples used to write the model.
- **D-17:** Runtime and cache deltas must be reported as Phase 51-local evidence. Phase 53 owns the milestone-wide cache/budget sweep, but Phase 51 must include adaptation model files in relevant digests and prove changed model files invalidate affected adapted output.

### Solver, Cache, And Budget Integration

- **D-18:** Accepted model facts feed the solver through semantic-graph constraints. Rejected facts stay visible to eval/reporting but must never emit `ModelEdge` constraints or derived call edges.
- **D-19:** Model expansion is budgeted. Required caps include at least max model files, max model facts, max expansions per model, max targets per source, and max model-derived edges per run. Budget hits must surface explicit evidence for later TAX-01 handling.
- **D-20:** Cache participation is mandatory. Model file paths, normalized model contents, accepted/rejected status, validator version, algorithm string such as `adaptation_model_v1`, solver model budget knobs, and prompt hash/report-affecting adaptation inputs must participate in deterministic digests where they affect behavior.
- **D-21:** Integrate without regressing Go RTA, TS tokens, or TS object-model behavior. The existing polyglot canary should be extended or mirrored only when it proves no cross-language model leakage.
- **D-22:** Keep `analysis::semantic_graph::constraints::ConstraintKind::ModelEdge` honest. It is currently reserved-empty; Phase 51 is the first real producer. Update tests that assert zero ModelEdge rows only in fixtures where no accepted adaptation model exists.
- **D-23:** Do not wire model facts into the public `RefinedCallEdgeFact` contract in this phase. Phase 52 owns refined-call projection over solver output.

### Verification And Acceptance

- **D-24:** Add native fixtures for: one accepted model edge, one rejected non-resolving target, one rejected wildcard/broad model, one rejected oracle-shaped RHS, one prompt-sanitizer forbidden-path case, one changed-model-file cache invalidation case, and one held-out subset delta report.
- **D-25:** Add at least one fixture where `ModelEdge` constraints are emitted only for accepted facts and absent for rejected facts.
- **D-26:** Add regression coverage that public-surface leak gate remains unchanged. Do not extend `ALLOWED_PRELUDE`.
- **D-27:** Keep external benchmark claims truthful. Phase 51 may report adapted-mode deltas and local held-out evidence, but Phase 54 owns hard corpus-level floors and final Go/Jelly recall numbers.

### Agent's Discretion

- Exact TOML file location and naming, provided it is repo-local, deterministic, and does not conflict with existing `.polint.toml` semantics.
- Exact field names for model facts, rejection reasons, budget structs, and stable keys.
- Whether the adaptation store is built as its own provider before semantic graph or as a semantic-graph input snapshot, provided provider order and digests are deterministic.
- Exact held-out subset selection strategy, provided adapted reports clearly separate selection cases from held-out validation cases.
- Natural plan slicing. A likely split is: (1) model schema/loader/store/validator, (2) semantic-graph `ModelEdge` lowering plus solver/cache/budget integration, (3) benchmark adapted mode + sandbox/prompt sanitizer + delta/held-out reporting, (4) fixtures/gates/roadmap closeout.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 51 goal, dependencies, ADAPT-01/ADAPT-02 success criteria, and Phase 52/53/54 boundaries.
- `.planning/REQUIREMENTS.md` - ADAPT-01 and ADAPT-02 requirement text, future ADAPT-FUT items, out-of-scope oracle/wildcard rules, and traceability mapping.
- `.planning/PROJECT.md` - v1.3 graph-engine precision goal, adaptation-model target feature, precision-first posture, and no-public-SDK-promotion discipline.
- `.planning/STATE.md` - current milestone state, prior decisions around solver/cache/model slots, and open repo-admin action for branch protection leak-gate checks.

### Immediate Upstream Phase Context

- `.planning/phases/50-js-ts-object-property-prototype-this-model-driver/50-CONTEXT.md` - Object-model boundaries, `ModelEdge` deferral, JS object-model budget/cache discipline, and Phase 54 benchmark-floor deferral.
- `.planning/phases/49-js-ts-function-token-propagation-driver/49-CONTEXT.md` - TS token policy, JS solver budget/config/cache patterns, and adaptation-model deferral.
- `.planning/phases/48-go-rta-driver/48-CONTEXT.md` - Go RTA policy, sub-budget pattern, budget-exceeded honesty, and polyglot no-cross-language canary precedent.
- `.planning/phases/47-unified-solver-core-derived-edge-provenance/47-CONTEXT.md` - Solver core, `SolverPolicy`, `PolicyOutcome`, `DerivedEdgeProvenance`, provider/cache/determinism/leak discipline.
- `.planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-CONTEXT.md` - Constraint vocabulary, reserved `ModelEdge`, semantic graph identity, provider slot discipline, and composition-over-duplication.
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Identity-vs-unsupported categories, `model_missing`, and public-surface-leak gate.

### Existing Research And Prompts

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - v1.3 graph-engine benchmark architecture and adaptation-model motivation.
- `research/evaluation-harness/STANDARD.md` - Evaluation terminology, adapted-run definition, allowed/forbidden adaptation-agent context, and reporting expectations.
- `research/evaluation-harness/prompts/graph-adaptation-agent.md` - Graph-specific adaptation-agent prompt and hard oracle-avoidance rules.
- `research/evaluation-harness/prompts/default-adaptation-agent.md` - Default adaptation-agent prompt, allowed/forbidden inputs, process, deliverables, and required record fields.
- `research/evaluation-harness/baselines/README.md` - Baseline/adapted-run records and warning not to commit expected labels into adaptation-agent context.
- `research/evaluation-harness/decisions/decision-log.md` - Benchmark architecture decisions and hidden/internal-first evaluation posture.
- `.planning/quick/260526-c36-capture-phase-40-benchmark-comparison-an/SUMMARY.md` - Prior quick-task guidance requiring prompt template, allowed/forbidden inputs, changed files, digests, eval deltas, and adaptation notes.
- `.planning/quick/260526-gtn-make-graph-benchmarks-the-main-benchmark/SUMMARY.md` - Graph adaptation-agent prompt addition and Go/Jelly benchmark counts.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/semantic_graph/constraints.rs` - `ConstraintKind::ModelEdge` reserved adaptation-model variant and stable kind behavior.
- `crates/polint/src/analysis/semantic_graph/build.rs` - Current honest zero-ModelEdge behavior that Phase 51 must update only for accepted models.
- `crates/polint/src/analysis/semantic_graph/{facts.rs,store.rs,provider.rs,cache_key.rs,validate.rs}` - Semantic graph fact/store/provider/cache/validation patterns and reserved adaptation-model cache comments.
- `crates/polint/src/analysis/solver/{policy.rs,engine.rs,provider.rs,budget.rs,cache_key.rs,facts.rs,store.rs,provenance.rs,validate.rs}` - Solver integration, budgets, cache participation, derived edges, provenance, and validation patterns.
- `crates/polint/src/eval/adaptation.rs` - Existing `AdaptationRecord`, prompt hash, allowed/forbidden inputs, changed artifact digests, no-change reason, and validation.
- `crates/polint/src/eval/delta.rs` - Existing `AdaptationDeltaReport`, case-sorted deltas, accepted/rejected fact deltas, runtime overhead, and cache invalidation scope.
- `crates/polint/src/eval/report.rs` - Existing `EvaluationRun.adaptation`, `adaptation_delta`, `MetricSections::adaptation`, `CategorizedFailureSection::model_missing`, and solver metrics.
- `crates/polint/src/eval/markdown.rs` - Existing markdown rendering for adaptation prompt path/hash and changed files.
- `crates/polint/src/eval/runner.rs` - External/native suite report construction and places to attach adapted-mode reports.
- `crates/polint/src/eval/observed.rs` - Existing observed item/status/category handling including model-missing category plumbing.
- `crates/polint/src/eval/suite.rs` - Suite manifests and scoring modes; use carefully because oracle expected paths are forbidden to adaptation agents.
- `tests/eval-fixtures/extension/adaptation-delta/` - Existing synthetic adaptation-delta fixture.
- `tests/eval-fixtures/extension/rejection-delta/` - Existing accepted/rejected extension-fact delta fixture.
- `tests/eval-fixtures/polyglot-canary/go-ts/` - Existing mixed-language canary to preserve no cross-language leakage.
- `crates/polint/tests/public_surface_leak.rs` - Public API leak gate; adaptation internals must stay unreachable from `polint::sdk::prelude::*`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `eval::adaptation::AdaptationRecord` already records schema version, suite id, case selection, agent kind/model/prompt path/prompt hash, allowed/forbidden inputs, changed artifacts, rule/extension digests, no-change reason, commands, and report paths.
- `eval::delta::AdaptationDeltaReport` already computes new true positives, removed false negatives, false-positive changes, unknown deltas, accepted/rejected extension fact deltas, changed graph/path keys, runtime overhead ratio, and cache invalidation scope.
- `EvaluationRun` already has optional `adaptation` and `adaptation_delta` fields; markdown rendering already displays adaptation prompt path/hash and changed file count.
- `MetricSections::adaptation` and `AdaptationMetricSection` already carry resolved unknowns, new false positives, removed false negatives, and accepted/rejected extension facts.
- `ConstraintKind::ModelEdge` already exists in the semantic graph vocabulary but currently has no producer.
- Existing extension adaptation fixtures provide accepted/rejected fact and delta-report precedents that can be mirrored for model facts.

### Established Patterns

- New graph-engine facts stay private and `pub(crate)`.
- Stable keys compose existing identities; dense IDs are assigned only after stable-key sorting.
- Provider/cache digests use frozen algorithm strings, upstream output digests, normalized row stable keys, status/precision/provenance fragments, and budget status.
- Budget exhaustion is explicit and never silently truncates precision.
- External benchmark reports are truthful: they separate baseline, adapted, and suite-native rows and do not claim final hard promotion floors before Phase 54.
- Adaptation-agent prompts already forbid expected labels and answer keys; Phase 51 must turn that policy into executable sandbox/prompt-sanitizer proof.

### Integration Points

- `analysis::semantic_graph::build` is the likely place where accepted adaptation model facts become `ModelEdge` constraints.
- `analysis::semantic_graph::cache_key` already mentions accepted adaptation models as a reserved input; Phase 51 should make that real for accepted model files/facts.
- `analysis::solver::provider` and `analysis::solver::cache_key` must include accepted models and model budgets where they affect derived edges.
- `eval::runner` and `eval::report` are the reporting path for `benchmark adapted` mode.
- `research/evaluation-harness/prompts/*adaptation-agent.md` are already committed prompt surfaces; prompt hashes should be computed from exact prompt text.

</code_context>

<specifics>
## Specific Ideas

- Model TOML examples should include at least one precise callback/model edge, one non-resolving target, one wildcard/broad-pattern rejection, and one oracle-shaped RHS rejection.
- A useful first fixture shape is a JS/TS source-evident framework/native callback that native token/object analysis still marks as unsupported, with a model file that maps the source callsite to a concrete target function.
- The sandbox fixture should create forbidden files matching `research/evaluation-harness/repos/*/expected*` and `research/evaluation-harness/suites/*.toml`, then assert the adaptation prompt/workspace does not include their contents or paths.
- Held-out reporting can start with deterministic fixture partitions before broad external-suite support. The important invariant is that the adapted report labels selection vs held-out cases separately.
- Existing `ChangedArtifactKind` may need a `Model` variant or equivalent. If added, update validation so changed model artifacts require model digests, not rule/extension digests.
- Keep `ModelEdge` payload design minimal. If `ConstraintKind::ModelEdge` needs payload fields, update remap/validation tests exhaustively and preserve the closed seven-variant vocabulary.

</specifics>

<deferred>
## Deferred Ideas

- Native-callable shim library for JS built-ins (`Array.prototype.map`, `Promise.then`, etc.) remains ADAPT-FUT-01.
- Reflection and dynamic-import auto-modeling remains ADAPT-FUT-02.
- Refined-call projection over solver output remains Phase 52.
- Unsupported/unknown taxonomy consolidation and `polint inspect unknowns --format json` remain Phase 52.
- Milestone-wide cache and solver-budget consolidation remains Phase 53.
- Hard benchmark promotion gates, F-score beta=0.5 tracking, and final Go/Jelly recall floors remain Phase 54.
- Public SDK views over adaptation/model facts are out of v1.3 and deferred to v1.4+.

</deferred>

---

*Phase: 51-Adaptation Model Layer*
*Context gathered: 2026-06-04*
