# Phase 54: Benchmark Promotion Gate Extension - Context

**Gathered:** 2026-06-05
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 54 --auto`

<domain>
## Phase Boundary

Phase 54 delivers **BENCH-01** and closes the v1.3 milestone by converting the benchmark evidence built in Phases 42-53 into enforced promotion gates.

This phase should add:

1. Hard promotion-gate failures for per-suite precision floors, including Go >=60% and configurable Jelly floors.
2. Flooding protection that rejects synthetic fixtures or benchmark rows that improve recall by adding low-precision noise.
3. F-score beta=0.5 tracking alongside the existing F1/F2/F3 metrics.
4. Per-language and per-scoring-mode deltas, reported and enforced separately rather than averaged across Go, TS/JS, Jelly, or whole-repo suites.
5. A polyglot Go+TS canary in the promotion gate path, using the existing mixed-language fixture and solver non-interference tests.
6. Public-API leak CI enforcement proving no v1.3 solver or benchmark internals are reachable from `polint::sdk::prelude::*`.
7. A final v1.3 audit record with Go and Jelly precision/recall numbers against the baseline target, including limitations for any external suite that cannot run locally.

This phase explicitly does **not**:

- Add new solver algorithms, new graph facts, or new language adapters.
- Promote solver, benchmark, cache, or eval internals to the public SDK.
- Create a new public CLI or JSON contract unless an existing internal eval path already requires it.
- Claim external corpus recall or precision that was not actually measured during verification.
- Rework the cache/budget implementation from Phase 53 except where needed to consume its RSS/runtime/cache evidence in gates.

</domain>

<decisions>
## Implementation Decisions

### Promotion Gate Strictness

- **D-01:** Promotion precision floors are hard failures. Recall improvements, F1/F0.5 movement, or solver coverage gains must not compensate for precision below the configured floor.
- **D-02:** Go suite precision floor is fixed at **>=60%** for v1.3 promotion.
- **D-03:** Jelly precision floor is configurable per suite or gate configuration because the Jelly oracle coverage and fixture maturity differ from the Go RTA suite. The default must be conservative and documented in the gate config or fixture manifest.
- **D-04:** Floors are evaluated per suite, language, scoring mode, and precision tier where the data is available. Do not pass by averaging a strong suite with a weak one.
- **D-05:** Gate output should include deterministic rows for metric name, observed value, threshold, scope, and verdict so failures are reviewable in CI.

### Flooding And Recall Claims

- **D-06:** Flooding traps are explicit gate failures. Synthetic rows that increase observed edges or facts while reducing precision below floor should fail even if recall improves.
- **D-07:** Recall claims are evidence to report and audit, not a bypass for precision floors.
- **D-08:** Final audit language should stay truthful: if an external benchmark clone or oracle is unavailable, record the suite as skipped or limited rather than fabricating the v1.3 target numbers.

### F0.5 And Delta Reporting

- **D-09:** Add F-score beta=0.5 as a first-class internal metric alongside existing `precision`, `recall`, `f1`, `f2`, and `f3`.
- **D-10:** Preserve compatibility with existing metric summaries and report normalization. If `MetricSummary` layout locks make direct top-level expansion risky, put the new metric in `MetricSections` or another defaulted internal section rather than weakening schema tests.
- **D-11:** Per-language deltas must be separate rows keyed by language, suite, scoring mode, and precision tier. Do not report only a single milestone-wide delta.
- **D-12:** F0.5 is the precision-weighted promotion score for v1.3 tracking. F1 remains visible for continuity but must not be treated as the only headline score.

### Polyglot Canary

- **D-13:** Reuse `tests/eval-fixtures/polyglot-canary/go-ts/` as the canonical polyglot canary. Do not add a second mixed-language fixture unless a minimal extension is needed to expose gate status.
- **D-14:** The gate must prove all three existing canary lanes: Go RTA resolves Go edges, TS token propagation resolves intra-TS token edges, and TS object-model dispatch resolves property-backed intra-TS edges.
- **D-15:** The canary must also prove no solver-derived edge crosses the Go/TS language boundary.
- **D-16:** The canary should run on every solver or promotion-gate CI path. If CI split jobs exist, the promotion job should depend on or invoke the canary explicitly.

### Public API Leak Enforcement

- **D-17:** Use the existing `crates/polint/tests/public_surface_leak.rs` and `.github/workflows/ci.yml` leak-gate job as the canonical public-surface guard.
- **D-18:** Do not extend `ALLOWED_PRELUDE` for Phase 54 solver, benchmark, gate, cache, or eval internals.
- **D-19:** If the leak gate fails, fix visibility (`pub` to `pub(crate)` or narrower) rather than relaxing the test or CI job.
- **D-20:** Phase 54 verification should record that the leak gate runs in CI and locally, not just that the test file exists.

### Runtime, RSS, Cache, And Determinism Evidence

- **D-21:** Phase 53 RSS/runtime/cache fields are gate inputs where stable thresholds exist. Missing measurement must be explicit and deterministic.
- **D-22:** Absolute machine-specific data such as temp paths, timestamps, and transient RSS observations must not participate in deterministic output hashes unless normalized.
- **D-23:** Runtime budget failures, cache quarantines, rejected facts, unknown budgets, and deterministic output hash mismatches remain hard promotion-gate checks.

### Gate Configuration Surface

- **D-24:** Keep promotion-gate configuration internal to eval suite manifests, gate threshold structs, tests, or CI wiring. Do not expose a new public rule-author or CLI configuration surface for v1.3 promotion gates.
- **D-25:** Extend `eval::gates::{PromotionGateThresholds, SuiteGateConfig}` or adjacent internal types instead of creating a parallel gate framework.
- **D-26:** Gate config and defaults must be deterministic and visible in tests, especially for Jelly's configurable floor.

### Milestone Closeout

- **D-27:** Phase 54 should reconcile roadmap, requirement, state, and final audit artifacts as milestone closeout after implementation and verification.
- **D-28:** If stale requirement statuses from prior completed phases are discovered, update them as documentation/state hygiene only; do not expand product scope.
- **D-29:** The final audit should include exact commands run, final Go/Jelly precision and recall values, F0.5/F1 values, floor verdicts, polyglot canary verdict, leak-gate verdict, and any limitations.

### the agent's Discretion

- Exact representation for F0.5: top-level metric, scanner section field, promotion-gate row, or a defaulted metric subsection, provided serialization compatibility and schema tests remain honest.
- Exact gate output shape: Rust struct, markdown table, JSON test helper, or existing eval report section, provided CI failures are deterministic and actionable.
- Exact Jelly default floor, if no prior suite-specific value exists, provided it is conservative, documented, and configurable.
- Exact plan slicing. A likely split is: (1) metric/report extension, (2) promotion threshold/flooding gates, (3) polyglot/leak CI promotion wiring, (4) final audit and milestone state closeout.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 54 goal, dependencies, success criteria, and v1.3 milestone closeout boundary.
- `.planning/REQUIREMENTS.md` - BENCH-01 requirement plus upstream graph/adaptation/cache requirements whose evidence Phase 54 gates.
- `.planning/PROJECT.md` - Product constraints, performance/reliability/truthfulness posture, and public API discipline.
- `.planning/STATE.md` - Current workflow state and accumulated decisions from completed v1.3 phases.

### Upstream Phase Context

- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Benchmark identity, oracle rendering, categorized failures, and public-surface leak gate origins.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` - Per-suite scoring modes, determinism gates, and report schema stability.
- `.planning/phases/48-go-rta-driver/48-CONTEXT.md` - Go RTA oracle suite, Go precision/recall baseline path, and polyglot expectations.
- `.planning/phases/49-js-ts-function-token-propagation-driver/49-CONTEXT.md` - TS token propagation and polyglot token canary lane.
- `.planning/phases/50-js-ts-object-property-prototype-this-model-driver/50-CONTEXT.md` - TS object-model lane and property-backed canary expectations.
- `.planning/phases/51-adaptation-model-layer/51-CONTEXT.md` - Hard precision floor deferral, F-score beta=0.5 decision, held-out deltas, and final promotion gate boundary.
- `.planning/phases/52-refined-calls-rework-unknown-taxonomy-consolidation/52-CONTEXT.md` - Refined-call projection, consolidated unknown taxonomy, per-language reporting expectations, and final gate deferral.
- `.planning/phases/53-cache-solver-budgets-consolidation/53-CONTEXT.md` - Cache dependency ledger, solver budget evidence, RSS reporting, and final promotion-gate deferral.

### Evaluation And Gate Implementation

- `crates/polint/src/eval/gates.rs` - Existing promotion gate threshold structs, suite gate config, gate report, and current hard checks to extend.
- `crates/polint/src/eval/metrics.rs` - Current precision/recall/F1/F2/F3 computation and `ComputedMetrics` conversion into report summaries.
- `crates/polint/src/eval/report.rs` - Internal evaluation schema, `MetricSummary`, `MetricSections`, scanner/performance/solver sections, normalization, and layout-lock tests.
- `crates/polint/src/eval/markdown.rs` - Deterministic markdown renderer for metrics, adaptation deltas, performance, and gate-adjacent reporting.
- `crates/polint/src/eval/performance.rs` - Runtime/cache/RSS performance report structures from Phase 53.
- `crates/polint/src/eval/runner.rs` - Native fixture runner, promotion fixture tests, deterministic JSON/markdown write path, and promotion report generation.
- `crates/polint/src/eval/suite.rs` - Suite manifests, languages, scoring mode, precision tiers, and manifest validation.
- `tests/eval-fixtures/promotion/cfg-call-flow-evidence/expected.polint-eval.toml` - Existing promotion fixture and synthetic observed rows that can seed flooding/floor tests.

### Polyglot Canary

- `tests/eval-fixtures/polyglot-canary/go-ts/expected.polint-eval.toml` - Canonical mixed Go+TS canary manifest and canary intent.
- `crates/polint/src/eval/go_rta.rs` - Go RTA canary test proving Go edges resolve without TS interference.
- `crates/polint/src/eval/ts_tokens.rs` - TS token canary test proving intra-TS token edge behavior and no cross-language edge.
- `crates/polint/src/eval/ts_object_model.rs` - TS object-model canary test proving property-backed intra-TS edge behavior and no cross-language edge.

### External Benchmarks And Research

- `crates/polint/src/eval/external/go_x_tools_callgraph.rs` - Go x/tools/RTA external adapter and Go recall/precision evidence source.
- `crates/polint/src/eval/external/jelly_callgraph.rs` - Jelly external adapter and Jelly oracle evidence source.
- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - v1.3 benchmark motivation, target recall movement, and graph-engine precision context.
- `research/evaluation-harness/STANDARD.md` - Native fixture and observed-output conventions.
- `research/evaluation-harness/baselines/README.md` - Baseline measurement context for final audit.
- `research/evaluation-harness/decisions/decision-log.md` - Historical benchmark and promotion decisions.

### Public API And CI Guardrails

- `crates/polint/tests/public_surface_leak.rs` - Frozen prelude/public-surface leak gate and expected allowed prelude.
- `.github/workflows/ci.yml` - CI leak-gate and determinism-gate jobs that Phase 54 must preserve and include in verification.
- `docs/API-VISIBILITY-PLAN.md` - Public/internal visibility rules for keeping v1.3 internals out of `polint::sdk::prelude::*`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `evaluate_promotion_gates` already evaluates native pass rate, graph/path misses, unknown budgets, rejected facts, runtime budget failures, cache quarantines, deterministic output hashes, and warning thresholds.
- `PromotionGateThresholds` and `SuiteGateConfig` are the natural extension points for precision floors, F0.5 thresholds, per-language deltas, and canary requirements.
- `ComputedMetrics` currently computes `precision`, `recall`, `f1`, `f2`, and `f3`; adding beta=0.5 should reuse the existing `f_score` formula.
- `MetricSummary` is layout-tested. `MetricSections` has defaulted subsections that can absorb compatible internal metric extensions.
- The promotion fixture already contains synthetic observed rows and a false-positive-prone extra graph edge, making it a good seed for flooding rejection tests.
- The polyglot fixture and three existing canary test modules already prove the mixed-language solver lanes.
- The public-surface leak gate and CI job already exist; Phase 54 should wire them into promotion verification and keep them frozen.

### Established Patterns

- Eval schema changes use defaulted sections and normalization tests to keep older report JSON deserializable.
- Promotion gate reports carry deterministic pass/fail/warn entries rather than panicking or relying on ad hoc assertion messages.
- Suite manifests must declare languages, precision tiers, language support, and scoring mode.
- Determinism gates avoid transient paths and machine-specific data in stable hashes.
- Public API discipline is enforced by tests and visibility, not by comments alone.

### Integration Points

- New precision/F0.5 gates should connect to `MetricSummary`, suite manifest metadata, and `evaluate_promotion_gates`.
- Per-language deltas likely connect through `EvaluationRun.suite_manifest`, `BenchmarkComparisonRow`, or a new defaulted internal gate/report section.
- Polyglot promotion status should connect existing `eval::go_rta`, `eval::ts_tokens`, and `eval::ts_object_model` tests to the promotion gate or CI path.
- Leak-gate promotion status should connect `.github/workflows/ci.yml` and `public_surface_leak.rs` into final verification/audit artifacts.
- Final closeout should update `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md`, `.planning/STATE.md`, and a phase verification/audit artifact after code passes.

</code_context>

<specifics>
## Specific Ideas

- Add a failing unit test where precision is below 0.60 but recall improves, and assert the gate fails with a precision-floor reason.
- Add a flooding fixture/report with many observed false positives and enough true positives to move recall, then assert it fails independently from recall/F1.
- Add a `f0_5` or `f_beta_0_5` report field and verify `precision=0.75`, `recall=0.60` computes the expected beta=0.5 value.
- Add a gate test proving a weak Go row cannot be hidden by a strong Jelly row, and the reverse if Jelly is configured.
- Treat missing external benchmark data as a limitation row in the final audit rather than a passing metric.
- Add or update CI comments/job names only if they reflect executable checks that really run.

</specifics>

<deferred>
## Deferred Ideas

- Public SDK views for solver, call graph, dataflow, semantic graph, benchmark evidence, or promotion gates remain out of v1.3.
- New external benchmark corpora and long-term benchmark infrastructure belong after v1.3 unless required to run the existing Go/Jelly adapters.
- Rich public benchmark dashboards can follow later; Phase 54 needs deterministic CI/audit evidence first.

</deferred>

---

*Phase: 54-Benchmark Promotion Gate Extension*
*Context gathered: 2026-06-05*
