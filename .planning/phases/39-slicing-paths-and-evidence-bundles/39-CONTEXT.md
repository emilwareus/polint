# Phase 39: Slicing, Paths, and Evidence Bundles - Context

**Gathered:** 2026-05-25
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 39 --auto`

<domain>
## Phase Boundary

Phase 39 delivers the internal evidence and query layer over the completed v1.2 analysis substrate. It should add internal evidence nodes/edges, evidence bundles, thin and full slice queries, source-to-sink/path queries, deterministic path ranking, summary expansion handles, provenance-rich diagnostic evidence, and deterministic JSON/SARIF evidence rendering.

This phase builds on Phase 38 data-flow facts, Phase 37 refined calls, Phase 36 type/value/access-path/alias facts, summaries, CFG/control dependence, entrypoints/trust boundaries, and extension facts. It does **not** promote public `Evidence<'_>`, `Paths<'_>`, `Slices<'_>`, or `DataFlow<'_>` SDK views, does not add external benchmark adapters, and does not make unbounded all-pairs path enumeration part of normal `polint check`. Phase 40 owns external benchmark adapters and promotion reports; Phase 41 owns public SDK/query ergonomics.

</domain>

<decisions>
## Implementation Decisions

### Evidence Bundle Contract

- **D-01:** Add a new private evidence/slicing layer rather than a standalone slicer. Evidence graph rows should reference existing stable facts from diagnostics, data flow, CFG/control dependence, calls/refined calls, summaries, models, extensions, places, symbols, references, and source spans.
- **D-02:** Preserve existing scalar `Diagnostic.evidence` behavior for compatibility. Phase 39 may add an internal `EvidenceBundleId`/`EvidenceBundle` association and render selected scalar evidence for existing consumers, but it must not remove or destabilize current human/JSON/SARIF diagnostics.
- **D-03:** Evidence nodes and edges need stable keys, run-local dense IDs, deterministic normalization, source span when user-rendered, status/precision/provenance/confidence/validation, optional expansion handles, compact labels, and cache/replay inputs.
- **D-04:** Evidence graph views should be query-specific materializations over normalized facts, not a second source of truth that duplicates every underlying analysis row.

### Slice Modes and Defaults

- **D-05:** Implement local slicing first. The first useful vertical is a local backward/forward slice inside one function over operation/place/data-flow/control-dependence evidence.
- **D-06:** Use thin slices as the default diagnostic explanation mode. Thin slices should prioritize value-producing data edges and selected summaries; full local/interprocedural slices remain internal debug/eval modes.
- **D-07:** Support explicit internal query modes: thin backward, full backward, forward impact, chop/source-to-sink, path, and expansion. The planner may choose exact enum names, but mode and edge-filter differences must be visible in cache keys, debug, and eval output.
- **D-08:** Slices must report omitted regions, unknowns, and budget truncation explicitly. A budgeted or filtered result must never look complete.

### Paths, Ranking, and Interprocedural Context

- **D-09:** Path search should be bounded and deterministic from the beginning. Use shortest path and bounded k-path extraction where appropriate, but never enumerate unbounded paths.
- **D-10:** Ranking is for display order only and must not affect solver truth. Prefer native/exact evidence, fewer unknowns, fewer unvalidated/model/heuristic edges, shorter paths, non-opaque summaries, and direct source/sink relevance.
- **D-11:** Interprocedural evidence must respect call/return context. Entering a callee pushes the call site; returning must pop the matching call site. Mismatched call/return paths are invalid and need regression tests.
- **D-12:** Unknown dynamic calls, unsupported edges, setup-missing summaries, and budgeted expansion must appear as explicit unknown/havoc/omitted evidence, not as missing paths.

### Summary Expansion and Rendering

- **D-13:** Summary-projected edges should render as compressed evidence steps by default, carrying summary id/stable key, subject/callable, domain, endpoint information, status, precision, provenance, and expansion status.
- **D-14:** Summary evidence should support three expansion states: expandable with an expansion key, opaque with a reason, and external/model-backed with model provenance. The planner may choose exact names.
- **D-15:** JSON evidence is the primary structured renderer. SARIF should be a lossy renderer from the same internal model via `codeFlows`, `threadFlows`, related locations, and messages; do not distort the internal model to match SARIF.
- **D-16:** Evidence rendering must be deterministic and bounded. JSON/SARIF must preserve enough status, precision, provenance, unknown, hidden-node, summary, replay-key, and omitted-region information that downstream agents can inspect why a diagnostic was produced.

### Extension Evidence Merge

- **D-17:** Extension evidence is additive and validation-gated. Repo-local extensions may add source/sink/barrier/sanitizer models, framework dispatch edges, call graph edges, summary edges, custom data-flow steps, labels, and grouping hints only through validated stable references.
- **D-18:** Lower-trust extension evidence cannot silently suppress native may edges, forge native provenance, claim exactness without validation, add unbounded expansion, or attach evidence to nonexistent spans.
- **D-19:** Use explicit merge verdicts such as accept, accept-with-precision-downgrade, candidate-only, and reject. Candidate or rejected evidence can be debug/eval-visible but must not strengthen public diagnostic claims.
- **D-20:** Default-vs-extension deltas should remain observable in eval/debug output so agents can see whether a repo-local model improved, downgraded, or failed evidence quality.

### Validation, Debug, Eval, and Public Boundary

- **D-21:** Validation must check dangling references, invalid spans, impossible edge kind/source-target pairs, missing provenance, unvalidated exactness, context-mismatched call/return paths, missing summary expansion status, missing omitted-region metadata for truncated results, and renderer loss.
- **D-22:** Debug output should include evidence node/edge counts by kind/status/precision/provenance, path counts, ranking inputs, unknowns, omitted regions, expansion handles, summary opacity, hidden node counts, and cache/replay keys. It must avoid raw source bodies, absolute paths, parser object IDs, timestamps, and nondeterministic ordering.
- **D-23:** Eval fixtures must cover local dependence, thin-versus-full slice differences, source-to-sink paths, sanitizer/barrier behavior, interprocedural direct-call context, summary compression/expansion, extension evidence, uncertainty markers, deterministic ranking, renderer determinism, and compact path limits.
- **D-24:** Public no-leak proof must cover normal `polint check --format json`, AI-friendly output, CLI help, SDK exports, runner surface, README, docs/facts, and SARIF/JSON evidence rendering. Internal provider ids, debug-only graph schemas, and unpromoted SDK view names must stay private unless intentionally exposed as a documented output contract for this phase.

### The Agent's Discretion

- The planner may choose exact module layout such as `analysis/evidence/{facts,store,graph,bundles,query,rank,render,validate,debug,cache_key}.rs` and `analysis/slicing/{query,local,paths,interprocedural}.rs`, provided visibility stays crate-private.
- The planner may split implementation into evidence contracts/store, local evidence graph, slice queries, ranked path queries, summary expansion, diagnostic/JSON/SARIF rendering, extension merge, and validation/debug/eval/no-leak proof.
- The planner may decide whether Phase 39 stores evidence bundles in `AnalysisDb` or materializes them per diagnostic/query, provided determinism, replay keys, and renderer tests prove stable output.
- The planner may keep interprocedural slicing narrow if direct-call context matching, summary compression, and unknown handling are proven with fixtures.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 39 goal, SAE-PREC-04 mapping, research references, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-PREC-04 requirement text and v1.2 boundaries.
- `.planning/PROJECT.md` — Product boundaries, private-analysis-first milestone intent, and public API discipline.
- `.planning/STATE.md` — Current milestone state and accumulated v1.2 decisions.

### Program Slicing and Evidence Research

- `research/program-slicing-evidence/FINAL-REPORT.md` — Practical conclusion, target evidence shape, query modes, provenance/trust model, ranking guidance, dependency order, and open questions.
- `research/program-slicing-evidence/RECOMMENDED_IMPLEMENTATION.md` — Evidence bundle, evidence node/edge store, local slicing, path queries, interprocedural context, summary expansion, JSON/SARIF rendering, extension merge, and future SDK view recommendations.
- `research/program-slicing-evidence/VALIDATION.md` — Validation levels, required fixture families, path quality metrics, determinism/cache tests, negative tests, and validation verdicts.

### Upstream Phase Decisions

- `.planning/phases/38-local-plus-summary-projected-data-flow/38-CONTEXT.md` — Data-flow fact contracts, local/interprocedural/summary-projected flow, source/sink/sanitizer/barrier models, unknowns, budgets, and explicit deferral of rich evidence to Phase 39.
- `.planning/phases/37-refined-call-graph-providers/37-CONTEXT.md` — Refined call edge facts, direct-versus-refined deltas, call graph tiers, extension/model validation, and explicit deferral of slicing/evidence to Phase 39.
- `.planning/phases/36-p0-type-value-place-alias-substrate/36-CONTEXT.md` — Type/value/access-path/points-to/alias facts, precision ceilings, explicit status rows, and future evidence/path consumers.
- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Entrypoints, trust boundaries, framework dispatch edges, unresolved framework facts, and extension overlays consumed by source/path evidence.
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Repo-local extension host, typed sinks, validation, precision ceilings, cache quarantine, and default-vs-extended eval evidence.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand query, summary SCC cache, trace, and quarantine substrate for bounded expensive evidence queries.
- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — Direct summaries, TITO, memory-touch, control/call effects, summary events, and unknown summary behavior.
- `.planning/phases/31-p0-abstract-domain-kernel/31-CONTEXT.md` — Abstract-domain statuses, local solver discipline, top/unknown events, and budget behavior.
- `.planning/phases/30-direct-call-facts/30-CONTEXT.md` — Direct call-site/target/unresolved fact model and call indexes.
- `.planning/phases/29-local-cfg-and-control-dependence/29-CONTEXT.md` — CFG, reachability, dominance, postdominance, control dependence, and unsupported control-flow facts.
- `.planning/phases/28-private-semantic-mir-and-place-identity/28-CONTEXT.md` — Semantic MIR operations, place identity, unknown/havoc conservative actions, and stable identity rules.

### Existing Implementation

- `crates/polint/src/diagnostics/mod.rs` — Existing diagnostic, evidence, label, suggestion/fix, fingerprint, and serialization contract.
- `crates/polint/src/reporting.rs` — Human, JSON, SARIF, and AI-friendly report rendering contract.
- `crates/polint/src/cli/mod.rs` and `crates/polint/src/runner/mod.rs` — CLI/runner rendering integration, AI-friendly persisted JSON, SARIF help mapping, and max-diagnostic behavior.
- `crates/polint/src/analysis/data_flow/` — Private data-flow nodes, edges, models, budgets, local/direct/summary projection, query search, validation, debug, and eval bridge.
- `crates/polint/src/analysis/cfg/` — CFG nodes/edges, reachability, dominance, postdominance, control dependence, validation, debug, and eval patterns.
- `crates/polint/src/analysis/calls/` and `crates/polint/src/analysis/refined_calls/` — Direct/refined call sites, targets, edge status/precision/provenance, call graph indexes, and unresolved evidence.
- `crates/polint/src/analysis/summaries/` — Summary domains, TITO flow roots/kinds, summary events, SCC closure, provider/cache, validation, and debug infrastructure.
- `crates/polint/src/analysis/types/`, `crates/polint/src/analysis/values/`, `crates/polint/src/analysis/access_paths/`, `crates/polint/src/analysis/points_to/`, and `crates/polint/src/analysis/aliases/` — Type/value/access-path/points-to/alias substrate for evidence precision and uncertainty.
- `crates/polint/src/analysis/entrypoints/` and `crates/polint/src/analysis/extensions/sinks.rs` — Trust boundaries, framework dispatch, extension fact validation, and model provenance inputs.
- `crates/polint/src/analysis_kernel/debug.rs`, `crates/polint/src/analysis_kernel/validation.rs`, and `crates/polint/src/eval/` — Debug, validation, eval observation, deterministic matching, fixture taxonomy, and no-leak proof patterns.
- `tests/eval-fixtures/` — Native fixture suite to extend with evidence/slicing fixtures.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility discipline and supported rule-author surface boundaries.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `Diagnostic` already carries scalar evidence, labels, help, suggestions/fixes, stable fingerprints, and deterministic serialization. Phase 39 should add structured evidence without destabilizing this public shape.
- Report rendering already supports human, JSON, SARIF, and AI-friendly output. Phase 39 can extend the existing rendering pipeline rather than adding a separate evidence command first.
- Phase 38 data-flow rows already contain local, direct-call, summary, model, unknown/havoc, and budget evidence with stable keys and debug counts. These are the primary input for source-to-sink and path evidence.
- CFG/control-dependence rows from Phase 29 provide control edges for full slices and branch-condition explanation.
- Refined calls from Phase 37 and direct calls from Phase 30 provide call-site identity needed for context-matched interprocedural paths.
- Summary facts from Phase 32 and data-flow summary-projected edges from Phase 38 provide the compression/expansion boundary for scalable evidence.
- Extension facts from Phase 34 and trust boundaries from Phase 35 already carry validation/provenance data needed for extension/model evidence.
- The eval harness already supports fact, path, invariant, runtime-budget, unknown, and deterministic output checks; Phase 39 should extend it rather than inventing a parallel harness.

### Established Patterns

- New analysis families stay crate-private until Phase 41 promotion.
- Provider output follows build -> normalize -> output digest -> store -> metadata refresh -> validate -> debug/eval.
- Stable keys are persistent identity; dense IDs are run-local handles assigned after sorting.
- Unknown, unsupported, setup-missing, rejected, havoc, omitted, and budget-exceeded states are first-class facts.
- Public no-leak tests protect normal CLI JSON/help, SDK exports, runner behavior, README, docs/facts, and public report schemas from private internal vocabulary.
- Extension/model contributions are additive, validation-gated, precision-ceiling gated, quarantine-aware, and observable as default-vs-extended deltas.

### Integration Points

- Add crate-private evidence/slicing modules under `crates/polint/src/analysis/` and register any provider manifest after `polint.data_flow` and before metrics unless planning chooses query-only materialization.
- Extend `AnalysisDb`, `FactFamily`, metadata assignment, validation, debug JSON, eval observation, and cache key vocabulary for evidence nodes, edges, bundles, slices, paths, omitted regions, unknowns, and replay keys.
- Extend `Diagnostic` or adjacent internal diagnostic state with optional evidence bundle references while preserving existing scalar evidence behavior.
- Extend report rendering in `reporting.rs`, CLI, and runner so JSON carries structured evidence first and SARIF maps selected paths to `codeFlows`/`threadFlows`.
- Add native eval fixtures under `tests/eval-fixtures/evidence/` or `tests/eval-fixtures/slicing/` covering local dependence, thin/full slices, paths, summaries, extensions, uncertainty, renderer determinism, and budgets.

</code_context>

<specifics>
## Specific Ideas

- Start with a local dependence fixture: variable assigned, reassigned/shadowed, branch-controlled sink, field/index access, and unreachable/dead branch markers.
- Add thin-versus-full slice tests asserting that thin slices are a subset of full slices, thin contains direct producers, full contains selected control dependencies, and omitted edges are reported when budget/filter removes them.
- Add a source-to-sink fixture covering direct local source to sink, sanitizer break, barrier block, multiple ranked paths, source through field/index, and budgeted truncation.
- Add an interprocedural fixture with two callers of one callee where only one source reaches the sink; assert context-matched mode rejects mismatched call/return paths.
- Add summary fixtures where a compressed summary edge appears in default evidence and an expansion key can reproduce the local callee path when available; opaque summaries carry an explicit reason.
- Add extension evidence fixtures where an extension adds a model edge or summary edge, fails to suppress a native may edge, and gets rejected when it references a nonexistent span or claims unsupported exactness.
- Add renderer tests ensuring JSON preserves paths, hidden-node counts, unknowns, replay keys, omitted regions, status/precision/provenance, and summary expansion keys; SARIF can be lossy but must not claim completeness when evidence is partial.

</specifics>

<deferred>
## Deferred Ideas

- Public `Evidence<'_>`, `Paths<'_>`, `Slices<'_>`, `DataFlow<'_>`, and stable rule-author query builders: Phase 41.
- External benchmark adapters, SliceBench/CodeQL/Semgrep/Joern benchmark integration, and precision/recall promotion reports: Phase 40.
- Full all-pairs path materialization, broad whole-program IFDS/IDE tabulation, high-k context sensitivity, and broad heap/object sensitivity: future work after native fixtures and benchmark gates prove the need.
- Public JSON/SARIF schema promises beyond the minimal diagnostic evidence contract intentionally added in Phase 39: defer until renderer fixtures and promotion gates stabilize the shape.

</deferred>

---

*Phase: 39-slicing-paths-and-evidence-bundles*
*Context gathered: 2026-05-25*
