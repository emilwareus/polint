# Phase 40: External Benchmark Adapters and Promotion Gates - Context

**Gathered:** 2026-05-26
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 40 --auto`

<domain>
## Phase Boundary

Phase 40 delivers the internal benchmark adapter, reporting, and promotion-gate layer that turns the existing v1.2 analysis substrate into measured evidence. It should extend the current crate-private eval harness so polint can compare scanner outcomes, graph/fact/path quality, runtime/cache behavior, and default-vs-agent-adapted deltas across native fixtures and selected external suites.

This phase does **not** promote stable public SDK query views, does not make `polint eval` a documented public CLI contract, does not vendor external benchmark corpora into the product repository, and does not add broad Java/Python language support. Phase 40 may implement adapter-only parsing/scoring for unsupported-language suites, but must label those as adapter/scoring validation rather than polint analysis results. Phase 41 owns public SDK/query ergonomics and any stable user-facing promotion of query views.

</domain>

<decisions>
## Implementation Decisions

### Benchmark Result Shape

- **D-01:** Benchmark reports must separate three claims: comparable other scanner/product results, `polint baseline` with no repo-specific adaptation, and `polint agent-adapted` after a dedicated adaptation agent writes repo-local rules, models, or provider extensions.
- **D-02:** Other-product results may be imported from pinned benchmark-published tables when local reproduction is impractical, but reports must label source, product, version, suite version, and whether the value was reproduced locally or copied from a published result.
- **D-03:** Locally reproduced competitor runs should record command, config, product version, suite version, artifact path, and environment notes. Initial comparison targets should include Semgrep, CodeQL, gosec, suite-native reference tools, and any suite-published product baselines where relevant.
- **D-04:** `polint baseline` means normal config and native/built-in analysis only. No benchmark-target-specific generated rule pack, model pack, or provider extension may be present.
- **D-05:** `polint agent-adapted` means a separate adaptation agent inspected the target codebase and wrote repo-local polint rules, model facts, or provider extensions before the benchmark was rerun. The adapted result must always be reported as a delta from the baseline result.

### Benchmark Families and First Targets

- **D-06:** Phase 40 must measure more than vulnerability findings. It should report three families: scanner outcome benchmarks, graph/fact/path benchmarks, and engine/adaptation benchmarks.
- **D-07:** Scanner outcome benchmarks answer whether the right finding was reported. Initial supported-language targets should prioritize a small SecBench.js TS/JS subset and Go-oriented gosec samples or CodeQL-inspired Go microcases, with OWASP Java/Python and RealVuln initially allowed as adapter/scoring-only where language support is missing.
- **D-08:** Graph/fact/path benchmarks answer whether internal analysis facts are correct. Native fixtures should cover CFG/control dependence, direct/refined call graphs, data-flow paths, evidence bundles, source/sink/sanitizer/barrier behavior, summaries, unknowns, budget truncation, and partial-truth graph matching.
- **D-09:** Engine/adaptation benchmarks answer whether repo-local adaptation improves scanner accuracy honestly. They must report default-vs-adapted deltas, resolved unknowns, new unknowns, accepted/rejected extension facts, new true positives, new false positives, removed false positives, runtime overhead, cache invalidation scope, and provenance.
- **D-10:** The first practical implementation slice should be native fixtures plus one supported-language external smoke suite plus OWASP expected-results adapter/scoring. For current polint, that likely means native fixture gates, SecBench.js smoke or gosec samples, and OWASP CSV parser/scorer marked adapter-only for Java/Python analysis.

### Agent Adaptation Protocol

- **D-11:** Every adapted benchmark run must include an adaptation record with suite id, case selection, agent/subagent kind, model if known, prompt path, wall-time/iteration budget, allowed inputs, forbidden inputs, changed rule/extension files, rule/extension digests, commands run, and adaptation notes.
- **D-12:** The adaptation agent must have access to `.claude/skills/polint/SKILL.md`, polint rule-authoring documentation, and extension/provider-sink research. It should be guided to model the target repository honestly, not to maximize the benchmark score directly.
- **D-13:** The adaptation prompt must forbid reading expected labels, answer keys, vulnerability truth, expected-result CSV truth mappings, or case-id-to-truth mappings before writing the adaptation.
- **D-14:** The adaptation prompt must forbid hardcoding benchmark case IDs, generated filenames, expected-result data, or suite naming conventions as detection logic.
- **D-15:** The adaptation prompt should direct the agent to inspect framework/lifecycle entrypoints, request/job/CLI/MCP trust boundaries, generated dispatch, summaries, call edges, source/sink/sanitizer/barrier models, unresolved calls, unknown flows, setup gaps, and noisy evidence.
- **D-16:** The adaptation agent should choose the narrowest repo-local mechanism: normal `#[polint::rule]` diagnostics for repository policy checks; provider/model extension facts for analysis semantics such as entrypoints, source/sink/sanitizer/barrier models, call edges, summaries, or trust boundaries.
- **D-17:** The eval report must embed or link the exact adaptation prompt text. If a future run changes the prompt, the changed prompt is part of the benchmark artifact and must be reported with the score.

### Adapter and Report Architecture

- **D-18:** Build on the existing crate-private `crates/polint/src/eval/` module, native fixture manifests, matchers, metrics, deterministic report hashing, observed-row bridge, and `KernelRunReport`. Do not create a parallel eval system.
- **D-19:** Keep the first `polint eval` command internal, hidden, or unstable. It may exist as an internal command or feature-gated path for CI/release gates, but Phase 40 should not document it as a stable public CLI contract.
- **D-20:** Add suite manifests that pin suite id, source URL, source commit, local clone path, license status, adapter kind, language support status, tier membership, case selector, expected-output paths, and whether the suite can run real polint analysis or only adapter/scoring validation.
- **D-21:** External benchmark source repositories must stay out of git history. Commit only adapter code, manifests, small expected/schema samples where license-reviewed, generated summaries, and pinned source metadata.
- **D-22:** Reports should produce deterministic JSON as the source of truth and generated Markdown/summary tables as derived artifacts. JSON should include schema version, suite metadata, cases, expected/observed rows, matches, metrics, provider stats, cache stats, adaptation record, competitor result records, output hash, and limitations.
- **D-23:** Metric reporting should include suite-native metrics where available and polint-unified metrics across suites. Preserve OWASP-native TP/FN/TN/FP, TPR/FPR, and score where relevant, but do not use one suite-native score as the only product metric.

### Promotion Gates

- **D-24:** Native fixtures remain the first promotion gate. External suite results do not replace engine invariants for provenance, precision, validation, extension merge, unknowns, cache invalidation, deterministic output, and public no-leak behavior.
- **D-25:** Add tiered gates: fast CI for native fixtures and small smoke subsets, nightly for broader supported-language suites, release for full suite reports, and research for expensive or unsupported-language adapter experiments.
- **D-26:** Gate failures should be configurable per suite and should cover determinism drift, recall/precision/F-score regression, false-positive trap increase, new high-severity false positives, runtime/provider-time regression, cache invalidation expansion, changed extension rejection count, and missing adaptation artifacts.
- **D-27:** Public precision claims are allowed only when tied to measured reports that identify suite version, polint version/commit, mode, competitor source, tier, case selection, limitations, and whether the result is baseline or adapted.
- **D-28:** Adapter-only validation for unsupported languages is valuable but cannot be used as a public claim that polint analyzed that language. Reports must label these cases explicitly.

### Public Boundary and Safety

- **D-29:** Phase 40 should keep eval schemas crate-private/internal unless a narrow output is intentionally stabilized. Stable public SDK/query views remain Phase 41.
- **D-30:** Public no-leak proof should cover normal `polint check` output, CLI help, SDK exports, runner behavior, README/docs/facts, and any hidden/unstable eval command gating so internal benchmark vocabulary does not become accidental public API.
- **D-31:** Adapted runs must never allow extension facts to bypass validation, precision ceilings, provenance requirements, cache-key participation, quarantine rules, or public rule-authoring constraints.
- **D-32:** A good adaptation is not just a higher score. The report must make cost and quality visible: new false positives, broad/heuristic facts, rejected facts, unknown changes, runtime/cache overhead, and path/evidence noise all count against the adapted result.

### The Agent's Discretion

- The planner may choose exact module names such as `eval::suite`, `eval::adapter`, `eval::external`, `eval::competitors`, `eval::adaptation`, `eval::gates`, `eval::tiers`, and `eval::markdown`, provided visibility remains crate-private.
- The planner may decide whether hidden eval execution lives under `crates/polint/src/cli` as an unstable command or under test/release tooling first, provided there is no public contract drift.
- The planner may split Phase 40 into slices such as eval schema extension, suite manifest/adapter trait, OWASP parser/scorer, native graph/fact/path promotion gates, supported-language smoke adapter, agent adaptation record/prompt, competitor baselines, and report/gate integration.
- The planner may defer full local reproduction of every competitor if published/pinned results are recorded honestly and at least one locally runnable supported-language comparison path exists.
- The planner may keep external benchmark subsets small in fast CI if release/nightly tiers can run broader suites outside the normal developer loop.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 40 goal, SAE-PROM-01 mapping, research references, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-PROM-01 requirement text, including scanner baselines, polint baseline, agent-adapted results, and prompt capture.
- `.planning/PROJECT.md` — Product boundary, private-analysis-first milestone intent, agent-extensible thesis, and public API discipline.
- `.planning/STATE.md` — Current milestone state, recent Phase 39 completion, and accumulated v1.2 decisions.

### Evaluation Harness Research

- `research/evaluation-harness/FINAL-REPORT.md` — External-benchmark-first strategy, suite ranking, measurement model, tiers, required native fixture layer, and wrong paths to avoid.
- `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md` — Internal eval architecture, canonical model, matchers/metrics, native fixture adapter, OWASP adapter, provider/cache stats, default-vs-extension delta, benchmark tiers, baselines, and reports.
- `research/evaluation-harness/STANDARD.md` — Suite/case/adapter vocabulary, expected/observed model, result classes, metric set, extension delta, and determinism requirements.
- `research/evaluation-harness/RESEARCH-ANALYSIS.md` — Benchmark family tradeoffs, extension delta metrics, matching policy, graph/path/evidence metrics, and adapted benchmark guardrails.
- `research/evaluation-harness/implementation/polint-eval-path.md` — Hidden `polint eval` implementation path, required comparison table shape, agent adaptation protocol, adaptation prompt template, and staged PR shape.
- `research/evaluation-harness/REPO-INDEX.md` — Pinned local clone metadata and no-vendoring policy for external benchmark repositories.
- `research/evaluation-harness/VALIDATION.md` — Verification notes, benchmark counts, schema volatility caveats, and prototype validation checklist.
- `research/evaluation-harness/oss/benchmark-comparison.md` — Benchmark priority ranking and suite-specific strengths/limitations.
- `research/evaluation-harness/algorithms/scoring-and-matching.md` — Diagnostic/fact/graph/path matching and extension delta algorithms.
- `research/evaluation-harness/algorithms/benchmark-scheduling.md` — Tiering, baseline comparison, and extension gate algorithms.

### Analysis Family Validation Research

- `research/data-flow/VALIDATION.md` — Data-flow validation expectations, path/source/sink/sanitizer/barrier coverage, and benchmark caveats.
- `research/call-graphs/VALIDATION.md` — Call graph benchmark truth, partial-truth caveats, dynamic-trace limitations, and unresolved-call accounting.
- `research/program-slicing-evidence/VALIDATION.md` — Evidence/path quality validation and renderer determinism expectations consumed by graph/path benchmark gates.

### Upstream Phase Decisions

- `.planning/phases/39-slicing-paths-and-evidence-bundles/39-CONTEXT.md` — Evidence/path contracts, extension evidence deltas, renderer determinism, and explicit deferral of external benchmark adapters to Phase 40.
- `.planning/phases/38-local-plus-summary-projected-data-flow/38-CONTEXT.md` — Data-flow facts, source/sink/sanitizer/barrier models, query-scoped path search, unknowns, budgets, and explicit deferral of benchmark promotion to Phase 40.
- `.planning/phases/37-refined-call-graph-providers/37-CONTEXT.md` — Refined call graph facts, direct-versus-refined deltas, partial-truth graph needs, and explicit deferral of benchmark adapters to Phase 40.
- `.planning/phases/36-p0-type-value-place-alias-substrate/36-CONTEXT.md` — Type/value/access-path/points-to/alias precision and budget facts consumed by graph/data-flow benchmark gates.
- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Entrypoints, trust boundaries, framework dispatch, and extension overlays that adaptation agents should model.
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Repo-local extension host, typed sinks, validation, precision ceilings, quarantine, and default-vs-extension eval evidence.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand query trace/cache/quarantine substrate for expensive benchmarked queries.

### Rule and Extension Authoring

- `.claude/skills/polint/SKILL.md` — Polint skill that adaptation agents must be allowed to use when writing repo-local rules.
- `research/agent-extension-surface/FINAL-REPORT.md` — Repo-local Rust extension thesis, process isolation, validation, provenance, and extension delta reporting.
- `research/agent-extension-surface/RECOMMENDED_IMPLEMENTATION.md` — Extension lifecycle, typed sinks, manifest/protocol shape, and diff/test commands.
- `research/agent-extension-surface/VALIDATION.md` — Extension validation, security, provenance, cache, and deterministic diff requirements.
- `research/agent-rule-authoring/FINAL-REPORT.md` — Rule versus provider extension distinction and agent-authored rule/model workflow.
- `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md` — Public rule-authoring surface, `polint test`, inspect/diff tooling, and provider extension separation.

### Existing Implementation

- `crates/polint/src/eval/model.rs` — Current internal eval schema for expected/observed diagnostics, facts, graph edges, paths, invariants, runtime budgets, fixture areas, and observed statuses.
- `crates/polint/src/eval/report.rs` — Current deterministic run/case/metric report, output hash normalization, and sorting behavior.
- `crates/polint/src/eval/metrics.rs` — Current precision/recall/F-score, graph/path, unknown, fact, trap, and runtime-budget metrics.
- `crates/polint/src/eval/matcher.rs` — Current matching outcomes and fixture matcher logic to extend for external suite and competitor comparisons.
- `crates/polint/src/eval/fixtures.rs` — Native fixture loader, path safety checks, synthetic observed-row policy, and fixture execution helpers.
- `crates/polint/src/eval/observed.rs` — Kernel/evidence/data-flow/debug observed-row bridge and deterministic eval observation helpers.
- `tests/eval-fixtures/` — Native fixture suite and manifest shape. Phase 40 should add promotion gate fixtures here and keep external suite source out of git.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` — Provider output metadata, cache stats aggregation, demand-query stats, and provider manifest output digest helpers.
- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifest/order/schema vocabulary through `polint.evidence` and `polint.metrics`.
- `crates/polint/src/analysis/extensions/` — Extension discovery, host/protocol, typed sinks, validation, cache key, and provider integration.
- `crates/polint/src/analysis/data_flow/`, `crates/polint/src/analysis/cfg/`, `crates/polint/src/analysis/calls/`, `crates/polint/src/analysis/refined_calls/`, and `crates/polint/src/analysis/slicing/` — Graph/fact/path families that Phase 40 should benchmark through native fixtures and supported-language suites.
- `crates/polint/src/cli/mod.rs` and `crates/polint/src/runner/mod.rs` — Existing public CLI/runner boundaries that hidden eval work must not accidentally stabilize.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility discipline and supported rule-author surface boundaries.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/polint/src/eval/` already contains an internal evaluation model, matcher, metrics, report, native fixture loading, observed-row extraction, deterministic output hashing, and fixture tests. Phase 40 should extend this module rather than starting a new harness.
- `tests/eval-fixtures/` already contains native fixtures for cache, semantic index, CFG, direct calls, refined calls, abstract domains, framework entrypoints, data flow, and evidence. This is the right base for promotion gates on graph/fact/path correctness.
- `KernelRunReport` already carries provider outputs, cache stats, demand query trace, and input snapshots. Phase 40 can project those into provider/runtime/cache sections of eval reports.
- The provider manifest list already includes `polint.refined_calls`, `polint.data_flow`, and `polint.evidence`, so promotion gates can measure the analysis families added through Phase 39.
- The extension subsystem already has discovery, host/protocol, typed sinks, validation, store, provider, cache key, and manifest modules. Agent-adapted benchmark mode should use this path for analysis semantics instead of inventing hidden benchmark-only hooks.
- `.claude/skills/polint/SKILL.md` gives adaptation agents a concrete current workflow for repo-local rules and public typed fact views.

### Established Patterns

- New analysis/eval surfaces stay crate-private or hidden until deliberate promotion.
- Deterministic JSON output and output hashes exclude transient timestamps and runtime durations from hash identity.
- Native fixtures use relative paths and reject absolute paths or parent-directory traversal.
- External benchmark repositories stay under gitignored research clone paths or local manifests; benchmark source is not committed.
- Unknown, unsupported, setup-missing, ambiguous, rejected, budget-exceeded, and unconfirmed statuses are first-class metric inputs.
- Extension facts are additive, validation-gated, precision-ceiling gated, cache-keyed, quarantine-aware, and visible as default-vs-extension deltas.
- Public no-leak tests protect normal `polint check`, SDK exports, runner behavior, README/docs, and CLI help from private analysis vocabulary.

### Integration Points

- Extend `eval::model` with suite manifests, external suite cases, competitor result records, evaluation modes, adaptation records, suite language/support status, tier metadata, and provider/cache/performance summaries.
- Extend `eval::metrics` and `eval::report` with suite-native metric maps, unified metric groups, competitor comparison tables, graph/fact/path breakdowns, extension/adaptation deltas, and gate verdicts.
- Add adapter traits and implementations under `eval` or a crate-private sibling module for native fixtures, OWASP expected-results CSV, supported-language smoke suites, and future RealVuln/SecBench.js/gosec/Jelly adapters.
- Add hidden/unstable CLI or internal test/release entrypoints for running suites by manifest and tier, writing deterministic JSON, and generating Markdown summaries.
- Add gate logic for fast/nightly/release/research tiers with deterministic subset selection and baseline comparison.
- Add adaptation artifact support: prompt file, adaptation note, allowed/forbidden input record, changed files/digests, and default-vs-adapted report linking.

</code_context>

<specifics>
## Specific Ideas

- Use the first supported-language external benchmark as a smoke path rather than trying to run every researched suite in one pass. For current polint, prioritize either a small SecBench.js subset for TS/JS or gosec/CodeQL-inspired Go cases for Go.
- Keep OWASP Java/Python as the first adapter/scorer because the CSV shape is simple and externally recognized, but label Java/Python analysis as unsupported until language adapters exist.
- Add a report section named `comparison_table` with rows for suite/case selection and columns for other scanner/product, polint baseline, and polint agent-adapted.
- Add an `adaptation.prompt_path` and `adaptation.prompt_hash` field so reports can prove which subagent prompt produced the adapted result.
- Add fast CI gates over native fixtures for CFG/call/data-flow/evidence graph correctness before external suite gates, because external scanner benchmarks do not validate engine invariants.
- For graph/path benchmarks with partial truth, classify extra static edges as `unconfirmed` unless the suite proves they are false. Do not over-penalize static analysis for dynamic-trace partial truth.
- For adapted mode, require the delta table to name concrete changed cases/facts/edges/paths, not only aggregate score changes. Agents need case-level feedback to improve models.

</specifics>

<deferred>
## Deferred Ideas

- Stable public `polint eval` CLI contract, public eval JSON schema, public `CallGraph<'_>`/`DataFlow<'_>`/`Evidence<'_>` SDK views, and bounded public query builders: Phase 41 or later after promotion gates prove the contracts.
- Full Java/Python scanner execution on OWASP/RealVuln: future language-adapter work. Phase 40 may parse/score these suites but must label unsupported analysis honestly.
- Full local reproduction for every competitor on every suite: release/research tier after the first supported-language comparisons work.
- CrossCommitVuln temporal workflows, SecCodeBench agentic workflows, full DroidBench/CryptoAPI-Bench/SecuriBench Micro, and large real-repo release claims: future release/research tiers.
- In-process extension runtime optimizations for adapted mode: future work after process-isolated extension protocol and eval artifacts are stable.

</deferred>

---

*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Context gathered: 2026-05-26*
