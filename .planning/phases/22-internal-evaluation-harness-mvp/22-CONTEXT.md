# Phase 22: Internal Evaluation Harness MVP - Context

**Gathered:** 2026-05-17
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 22 adds an internal evaluation harness MVP for polint's analysis engine: deterministic expected/observed JSON, generic matchers, metrics, and native fixtures that prove kernel, provenance, cache, and extension invariants. The harness is evidence infrastructure for future precision work, not a public product surface yet.

This phase must not implement external benchmark adapters, typed cache-key vocabulary, persistent layer cache behavior, the repo-local extension/provider sink, MIR, CFG, call graph, data-flow, public SDK query promotion, or a stable public `polint eval` CLI contract. Those belong to later v1.2 phases.

</domain>

<decisions>
## Implementation Decisions

### Harness Surface
- **D-01:** Keep the Phase 22 harness internal, crate-private, and test-facing by default. Do not add a documented public SDK surface, crate-root export, or stable public CLI contract for evaluation in this phase.
- **D-02:** If planner research finds a command path useful, it must be hidden/internal or test-only and must not be documented as a supported `polint eval` contract. Public eval expansion belongs to later promotion phases.
- **D-03:** Prefer a module such as `crates/polint/src/eval/` or `crates/polint/src/analysis_kernel/eval/` with `pub(crate)` visibility. Exact module names are planner discretion.

### Evaluation Model
- **D-04:** Build a canonical expected/observed model that can represent diagnostics, facts, graph edges, paths, invariants, and runtime budgets from day one, matching the roadmap success criteria.
- **D-05:** The MVP schema should be stable enough for native fixtures and future adapters, but it is not a public JSON schema yet. Use deterministic serde output and internal tests rather than docs that imply public support.
- **D-06:** Expected items should support exact, tolerant, partial, and forbidden assertions where useful. False-positive traps should be first-class expected items, not comments or test-only conventions.
- **D-07:** Observed items should be normalized around polint-owned stable keys, relative paths, diagnostic fingerprints, fact family labels, provider/provenance metadata, and explicit unknown/setup/unsupported states where available.

### Native Fixtures
- **D-08:** Ship native polint fixtures first. Suggested layout is `tests/eval-fixtures/<area>/<case>/repo/` plus an `expected.polint-eval.toml` or equivalent manifest and a short README when helpful.
- **D-09:** Minimum fixture coverage must include kernel execution/provider-order invariants, provenance/metadata invariants from Phase 21, current cache behavior, and extension-style rejection/delta invariants.
- **D-10:** Extension fixtures in Phase 22 should model extension invariants through synthetic expected/observed items or controlled internal fixture data. Do not implement or activate the real extension/provider sink here; Phase 34 owns that.
- **D-11:** Cache fixtures may assert current cache determinism and no stale public behavior using existing cache surfaces, but must not introduce typed layer cache keys or persistent layer cache semantics. Phase 23 owns cache-key vocabulary and Phase 24 owns layer persistence.

### Matchers And Metrics
- **D-12:** Implement generic matchers for diagnostics, facts, graph edges, paths, invariants, and runtime budgets. The harness should compare normalized outputs, not embed analysis algorithms inside the scorer.
- **D-13:** Graph and path matching must support partial truth. Extra static edges or paths against partial ground truth should be classified as `unconfirmed` or equivalent, not automatically false positives.
- **D-14:** Include basic metric types such as true/false positives/negatives, precision, recall, F-score variants, false-positive trap hits, unknown counts, graph/path counts, and runtime budget pass/fail where the MVP data supports them.
- **D-15:** Keep suite-native scorecards, benchmark tiers, baselines, and regression gates future-shaped but out of scope unless needed for native fixture pass/fail. External benchmark scoring is a later phase.

### Determinism And Hashing
- **D-16:** Expected/observed JSON and reports must be byte-stable for the same inputs. Sort rows, maps, diagnostics, facts, graph edges, paths, and metric groups explicitly before serialization.
- **D-17:** Output hashes must exclude timestamps, absolute machine-local paths, temp roots, pointer-like values, nondeterministic map order, raw elapsed timestamps, and other transient runtime details.
- **D-18:** Runtime budgets may be asserted as coarse pass/fail thresholds in fixtures, but exact wall-clock durations should not participate in deterministic output hashes.

### Integration Boundaries
- **D-19:** Reuse the current private `AnalysisKernel`, provider manifests, Phase 21 metadata/debug helpers, diagnostics rendering/fingerprints, cache helpers, and temp-repo fixture patterns where practical.
- **D-20:** The harness should consume real kernel output for at least one native fixture rather than only comparing hand-built expected/observed structs.
- **D-21:** Preserve existing `polint check`, SDK fact views, examples, cache behavior, diagnostics rendering, ignore handling, and rule execution behavior. Phase 22 should add evidence infrastructure without changing user-facing behavior.
- **D-22:** Do not vendor external benchmark repositories or copied benchmark cases into the product tree in this phase. Keep external benchmark adapter work and license checks deferred.

### the agent's Discretion
- The planner may choose exact internal type names, fixture manifest format, and file layout.
- The planner may split implementation into model/serialization, matcher/metrics, fixture runner, and fixture coverage plans.
- The planner may decide whether native fixtures live under `tests/eval-fixtures/`, `crates/polint/tests/eval-fixtures/`, or another test-owned path, as long as no external benchmark content is committed without license review.
- The planner may use `insta` snapshots or direct JSON assertions where they improve determinism proof, but output hashes must still be tested directly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 22 goal, success criteria, research refs, and v1.2 phase order.
- `.planning/REQUIREMENTS.md` - `SAE-FND-03` acceptance requirement and milestone out-of-scope constraints.
- `.planning/PROJECT.md` - Current milestone target, public API discipline, truthfulness constraints, and behavior-preservation constraints.
- `research/ROADMAP.md` - Source-of-truth implementation sequence and Phase 22 row.

### Research
- `research/evaluation-harness/FINAL-REPORT.md` - External-benchmark-first strategy, native fixture purpose, metric model, tiers, and wrong paths to avoid.
- `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md` - Evaluation model, expected/observed item kinds, matcher/metric guidance, native fixture layout, report/hash expectations, and future adapter order.
- `research/evaluation-harness/VALIDATION.md` - Validated benchmark facts, license caveats, setup caveats, and pre-implementation validation notes.

### Prior Phase Decisions
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel ownership boundary, provider manifests, provider order, and no public provider surface.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Internal fact metadata, stable keys, validation, deterministic metadata debug JSON, and no public metadata surface.
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - Internal `AnalysisPlan`, capability diagnostics, deterministic plan/cache digest, and public support-view boundary.
- `.planning/phases/12-resolved-imports-and-module-relationships/12-CONTEXT.md` - Deterministic graph facts, setup-missing behavior, and typed SDK boundary.
- `.planning/phases/13-symbols-and-references/13-CONTEXT.md` - Stable symbol/reference IDs, precision/status fields, setup-missing behavior, cache/setup diagnostics, and external-consumer proof patterns.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/mod.rs` - Private kernel facade, provider manifests, and current `KernelOutput` shape; likely harness input for real observed output.
- `crates/polint/src/analysis_kernel/debug.rs` - Test-only deterministic metadata debug JSON for files/imports/symbols/references; useful provenance fixture source.
- `crates/polint/src/analysis_kernel/metadata.rs` and `validation.rs` - Fact families, `FactRef`, stable keys, confidence/precision/validation vocabulary, and deterministic validation diagnostics.
- `crates/polint/src/core/mod.rs` - `AnalysisDb`, typed fact vectors, metadata sidecar accessors, and current stable in-run IDs.
- `crates/polint/src/diagnostics/mod.rs` - Diagnostic model, fingerprints, deterministic ordering, and JSON/SARIF-like rendering.
- `crates/polint/src/cache/mod.rs` and `crates/polint/src/cache/keys.rs` - Current hash cache keys, cache layout, and cache status helpers that native cache fixtures can exercise without adding Phase 23 vocabulary.
- `crates/polint/tests/cli.rs` - Existing temp-repo helpers, cache determinism tests, kernel delegation checks, and public compatibility patterns.
- `tests/fixtures/` - Existing Go, TS, and mixed source fixtures that can inform small native eval repos.

### Established Patterns
- Public rule-author surfaces stay under `polint::sdk` and `polint::runner`; new analysis/evaluation internals stay `pub(crate)`.
- Machine-readable internal/test outputs must use relative paths, sorted rows, deterministic hashes, and no timestamps or temp roots.
- Current tests already use temp repos and external-consumer-style rule crates for public compatibility proof.
- Existing cache behavior is file/source/config/rule/plan-digest oriented; deeper layer cache semantics are intentionally later.
- Setup-sensitive and heuristic uncertainty should remain visible as data rather than being hidden by empty outputs or overconfident labels.

### Integration Points
- Add the harness model and matcher code behind an internal module.
- Build native fixture loading around small repo directories plus expected manifests.
- Use `AnalysisKernel::run` to produce observed kernel output for at least one fixture.
- Use Phase 21 metadata debug JSON or direct metadata reads to prove provenance fixtures.
- Reuse current cache tests or helpers to produce cache invariant observations without changing cache contracts.
- Add deterministic JSON/hash tests that run the same fixture twice and compare normalized output plus output hash.

</code_context>

<specifics>
## Specific Ideas

- Treat Phase 22 as the engine's evidence spine: small, boring fixtures that future call graph, data-flow, cache, and extension work can reuse.
- Keep native fixtures honest and narrow. They are not marketing benchmarks and should not replace external benchmark adapters later.
- Use external-benchmark research to shape the schema, but defer OWASP/SecBench/RealVuln/gosec/Jelly adapters until the external adapter/promotion phase.
- Extension invariants matter now because the schema should already know how to express accepted/rejected extension facts and default-vs-extension deltas, even though real extension activation is later.

</specifics>

<deferred>
## Deferred Ideas

- External benchmark adapters for OWASP, RealVuln, gosec, SecBench.js, Jelly, CodeQL/Pyre/Pysa-style microcases, DroidBench, CryptoAPI-Bench, SecuriBench Micro, CrossCommitVuln, and SecCodeBench.
- Public stable `polint eval` CLI, public eval JSON schemas, benchmark tiers, baselines, and regression gates.
- Provider stats, typed cache-key vocabulary, input snapshots, persistent layer cache, and cache hit/miss reporting by layer.
- Real repo-local extension/provider sink activation and extension fact merge APIs.
- Public SDK query promotion and agent-facing advanced analysis commands.

</deferred>

---

*Phase: 22-internal-evaluation-harness-mvp*
*Context gathered: 2026-05-17*
