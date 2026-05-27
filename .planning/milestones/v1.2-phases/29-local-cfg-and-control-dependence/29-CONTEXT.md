# Phase 29: Local CFG and Control Dependence - Context

**Gathered:** 2026-05-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 29 delivers a private, validated local CFG layer over the existing Phase 28 semantic MIR for Go and TS/JS. It should add CFG functions, operation/block nodes, typed edges, reachability, dominance, postdominance, and control-dependence facts with deterministic snapshots and validation. Public CFG SDK/query promotion remains deferred unless this phase deliberately scopes a narrow, documented, tested surface.

</domain>

<decisions>
## Implementation Decisions

### Fact Shape And Provider Boundary
- **D-01:** Add CFG as a crate-private analysis layer that builds on `analysis::mir` and existing `AnalysisDb`/metadata patterns. Keep new CFG facts, provider wiring, debug JSON, eval observation, and validation test-facing/internal by default.
- **D-02:** Model both operation-level nodes and basic blocks. Operation nodes preserve MIR/source evidence for later path explanations, while basic blocks are the scalable graph substrate for reachability, dominators, postdominators, control dependence, and later abstract domains.
- **D-03:** Add an internal provider identity such as `polint.cfg` or an equivalent manifest-owned CFG provider after `polint.semantic_mir` and before metrics or future data-flow consumers. Provider manifests, run reports, precision ceilings, and output digests must be updated together.
- **D-04:** Use stable keys and run-local dense IDs separately, following Phase 28. Persistent identity should be derived from body stable key, node/block/edge kind, deterministic ordinal, MIR operation stable key where applicable, source evidence, and graph view where applicable.

### CFG Views And Edge Semantics
- **D-05:** Start with named internal graph views instead of one universal graph. The default implementation target should include at least a normal/control view and an abrupt-aware view; exceptional/cleanup/async details may be represented conservatively where language support is incomplete.
- **D-06:** Emit typed edges rather than unlabelled graph links. Minimum edge kinds should cover normal fallthrough, true/false branches, loop enter/back/exit, break, continue, return, throw/panic, short-circuit, switch/default, unreachable, unknown, and synthetic entry/exit edges.
- **D-07:** Treat Go `go` statements as spawn/control-boundary evidence, not intraprocedural successors into the spawned function. Direct call target facts and call graph behavior remain Phase 30.
- **D-08:** Treat TS/JS `await`, `yield`, promise scheduling, dynamic `eval`, dynamic import targets, getters/setters, optional-chain gaps, and complex `finally` behavior honestly. Use unsupported/unknown/control-flow uncertainty facts rather than exact edges when the implementation cannot model them precisely.
- **D-09:** Treat Go `defer`, `panic`, `recover`, `select`, `goto`, labels, and `fallthrough` as first-class CFG concerns where supported. If the first slice cannot model full SSA-grade semantics, it must record explicit precision/status limitations.

### Derived Analyses
- **D-10:** Compute reachability, dominators, postdominators, and control dependence as derived facts over validated CFG facts rather than baking them into language-specific CFG builders.
- **D-11:** Use simple deterministic algorithms first. Function-sized graphs can start with iterative reachability/dominator/postdominator computation, with optimization deferred until eval/runtime data shows a bottleneck.
- **D-12:** Postdominance must use an explicit synthetic unified-exit policy per graph view. Infinite loops, unsupported exits, exceptional exits, and cleanup edges should carry precision/status notes so control-dependence facts do not overstate certainty.
- **D-13:** Control-dependence facts should preserve the controlling edge kind and view/precision. Later rules and evidence layers should not have to traverse raw AST, parser nodes, or raw MIR operation sequences to ask guarded-by style questions.

### Validation, Cache, And Evaluation
- **D-14:** Extend metadata and validation for every new CFG family. Validation should reject dangling body/node/block/edge references, duplicate stable keys, invalid source spans, duplicate normalized edges, unreachable nodes mislabeled as reachable, malformed entry/exit structure, and precision-ceiling violations.
- **D-15:** Cache identity must include provider/schema version, source/config/lifecycle digests, semantic MIR output digest, relevant upstream syntax/semantic/topology digests, graph view/provider parameters, and absent extension/model/toolchain slots. Full persistent reuse may be deferred only if key vocabulary and output digests are still future-fit.
- **D-16:** Add deterministic eval snapshots for Go and TS/JS that cover branches, loops, returns, short-circuiting, panics/throws, switch/select/fallthrough where available, unreachable code, and unsupported constructs.
- **D-17:** Add public no-leak and compatibility proof. Public CLI JSON/help, `polint inspect`, `polint test`, SDK exports, docs, README, and temp-repo rule behavior must not advertise or expose private CFG internals unless a deliberate public promotion is included with docs and tests.

### the agent's Discretion
- The planner may choose exact module placement, such as `analysis/cfg/{ids,facts,graph,builder,derived,validate}.rs`, or a sibling `cfg/` module, as long as visibility remains crate-private and consistent with the existing `analysis::mir` layout.
- The planner may split the phase into multiple plans: schema/provider/storage, shared builder, Go CFG lowering, TS/JS CFG lowering, derived analyses, cache/debug/eval fixtures, and public-boundary proof.
- The planner may choose whether CFG facts are stored directly in `AnalysisDb` sidecars or a private semantic store/session wrapper, provided provider output metadata, validation, debug JSON, eval observation, and future cache restore stay straightforward.
- The planner may defer exact promise scheduling, fully precise TS/JS `finally`, full Go recover semantics, exact Go SSA parity, interprocedural call edges, extension overlay sinks, public `Cfg<'_>`, refined no-return summaries, and optimized dominator algorithms if Phase 29 success criteria are satisfied honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 29 goal, success criteria, and v1.2 milestone sequence.
- `.planning/REQUIREMENTS.md` - `SAE-SEM-04` requirement for local CFG, dominance, postdominance, and control-dependence facts over MIR.
- `.planning/PROJECT.md` - Product value, public API discipline, reliability, truthfulness, performance, and v1.2 substrate-first constraints.
- `.planning/STATE.md` - Current milestone state and accumulated Phase 20-28 decisions.
- `research/ROADMAP.md` - Source implementation sequence; Phase 29 maps to research PR 10 after private semantic MIR and before direct call facts.

### CFG And Control-Flow Research
- `research/cfg-control-flow/FINAL-REPORT.md` - Executive decision, language conclusions, layered architecture, algorithm conclusions, and product-specific extension guidance.
- `research/cfg-control-flow/RECOMMENDED_IMPLEMENTATION.md` - Recommended internal fact schema, builder/validator, Go and TS/JS provider scope, reachability/dominator algorithms, postdominance, control dependence, and validation path.
- `research/cfg-control-flow/STANDARD.md` - Normalized terminology, fact family shapes, precision labels, CFG views, invariants, and future SDK shape.
- `research/cfg-control-flow/VALIDATION.md` - Validation evidence, important corrections, confidence levels, and remaining open questions.
- `research/cfg-control-flow/REPO-INDEX.md` - Primary implementation references for Go, Oxc, CodeQL, Soot/WALA/OPAL, Checker Framework, Joern, and Semgrep.

### Upstream Phase Decisions
- `.planning/phases/28-private-semantic-mir-and-place-identity/28-CONTEXT.md` - Semantic MIR/place contracts, provider placement, unsupported semantics, cache identity, and public no-leak requirements that CFG must build on.
- `.planning/phases/27-layered-module-package-topology-graph/27-CONTEXT.md` - Internal topology and import-to-package context that can inform CFG body ownership and source-set/package context.
- `.planning/phases/26-semantic-index-deepening/26-CONTEXT.md` - Internal semantic rows, unknown/status vocabulary, stable semantic keys, and no broad public semantic API.
- `.planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-CONTEXT.md` - Layer cache stale-safety, dependency indexes, deterministic payload restore, and public cache no-leak proof.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Typed digest, layer/query/summary/diagnostic key vocabulary and provider output metadata.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Fact metadata sidecar, stable-key merge validation, provider precision ceilings, and deterministic debug JSON.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifest discipline, and behavior-preserving provider order.

### Source Surfaces To Inspect
- `crates/polint/src/analysis/mod.rs` - Existing private analysis module boundary.
- `crates/polint/src/analysis/provider.rs` - `polint.semantic_mir` provider, output digest, merge behavior, and current recompute-only cache stats.
- `crates/polint/src/analysis/mir/body.rs` - MIR body, operation output, status, and normalization model.
- `crates/polint/src/analysis/mir/op.rs` - MIR operation kinds, branch/call/return/unsupported rows, and unsupported-domain vocabulary.
- `crates/polint/src/analysis/mir/lower_go.rs` - Go MIR lowering and current control-shape evidence.
- `crates/polint/src/analysis/mir/lower_ts.rs` - TS/JS MIR lowering and current control-shape evidence.
- `crates/polint/src/analysis/places.rs` - Place identity model used by MIR and later CFG/evidence.
- `crates/polint/src/core/mod.rs` - `AnalysisDb`, fact metadata attachment, semantic MIR storage, capability support, and public capability placeholders.
- `crates/polint/src/analysis_kernel/mod.rs` - Provider execution order, provider output metadata, validation, and run report integration.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifest schema, current `polint.semantic_mir` manifest, precision ceilings, and provider-order tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Fact family, precision, validation status, stable key, and metadata helpers.
- `crates/polint/src/analysis_kernel/validation.rs` - Provider precision-ceiling and missing-metadata validation to extend.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Layer key and digest patterns, especially semantic MIR cache-key inputs.
- `crates/polint/src/analysis_kernel/debug.rs` - Test-only metadata/debug JSON patterns for adding CFG rows.
- `crates/polint/src/eval/fixtures.rs`, `crates/polint/src/eval/model.rs`, and `crates/polint/src/eval/observed.rs` - Internal eval fixture categories, observed fact extraction, and semantic MIR snapshot precedent.
- `crates/polint/src/sdk/facts.rs` and `crates/polint/src/analysis_plan.rs` - Existing reserved `Cfg<'_>`/`cfg` capability behavior and unsupported capability diagnostics that should remain honest unless intentionally promoted.
- `crates/polint/src/graph/mod.rs` - Existing placeholder `cfg_to_dot` implementation that must not be mistaken for a supported CFG.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust best-practice usage, rule-authoring platform contract, Go lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.
- `docs/facts/symbols-and-references.md`, `docs/facts/imports.md`, and `docs/facts/module-graph.md` - Existing supported fact docs to keep aligned if public behavior is touched; Phase 29 should not add public CFG docs unless promotion is intentional.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `analysis::mir` already exists as a crate-private module with `MirBody`, `MirOperation`, `PlaceFact`, `UnsupportedSemanticFact`, stable-key normalization, and language lowerers for Go and TS/JS.
- `analysis::provider::derive_semantic_mir_with_cache_stats` already merges Go/TS MIR outputs, computes deterministic output digests, records recompute stats, and stores rows through `AnalysisDb::replace_semantic_mir`.
- `AnalysisKernel::run` already executes `polint.semantic_mir` after module topology and symbol graph, then records provider output metadata and validates fact metadata before returning.
- `ProviderManifest` already includes `polint.semantic_mir`; Phase 29 can follow the same manifest/schema/precision-ceiling pattern for `polint.cfg`.
- `AnalysisDb` already stores semantic MIR rows and metadata for `MirOperation`, `Place`, and `UnsupportedSemantic` families; CFG can follow `replace_*` style storage and metadata refresh patterns.
- `analysis_kernel::metadata`, `validation`, and `incremental::keys` already provide stable-key, precision/status, provider-output digest, input snapshot, and cache-key vocabulary.
- `analysis_kernel::debug` and `eval::observed` already have semantic MIR debug/eval row extraction that can be extended for CFG snapshots.
- Reserved public `cfg` capability handling already exists and currently reports unsupported capability diagnostics; this should remain honest unless Phase 29 deliberately promotes support.

### Established Patterns
- New analysis internals stay `pub(crate)` and test/eval-facing until deliberately promoted.
- Provider outputs are deterministic, sorted, metadata-backed, and validated before rules run.
- Unknown, unsupported, partial, setup-missing, and heuristic behavior is represented explicitly and does not claim exactness.
- Public no-leak proof is done through public JSON/help/docs/source-surface checks and temp-repo external rule behavior using only `polint::sdk::prelude::*` and `polint::runner::run_cli`.
- Cache and output digests avoid raw source text, raw ASTs, absolute paths, timestamps, parser allocation IDs, or run-local dense IDs as identity.
- Go lifecycle constraints remain in `.polint.toml` and sidecar/temporary workspace behavior; CFG must not write repository lifecycle files.

### Integration Points
- Add CFG fact IDs/types and storage adjacent to the existing private `analysis` module.
- Extend provider manifests, kernel execution, provider output reports, metadata families, and validation for `polint.cfg`.
- Build CFG from stored MIR bodies and operations rather than raw parser ASTs wherever practical; language-specific lowerers may add edge precision/status based on Go/TS semantics.
- Extend input snapshot/cache key vocabulary for CFG provider parameters and semantic MIR output dependency.
- Extend metadata/debug JSON and eval observed extraction with CFG node, block, edge, reachability, dominator, postdominator, and control-dependence rows.
- Preserve current public capability behavior until the phase intentionally adds a supported SDK/query surface.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: start private and internal, not public.
- Auto-selected default: include both operation-level and block-level graphs because later data flow and evidence need both source anchors and scalable graph queries.
- Auto-selected default: derive reachability, dominance, postdominance, and control dependence after CFG validation, not inside language-specific builders.
- Auto-selected default: prefer truthful partial/unknown/unsupported rows over pretending exact CFG coverage for `finally`, `defer`, panic/recover, async scheduling, dynamic JS, and unsupported language constructs.
- Auto-selected default: plan fixtures around the roadmap success criteria: branches, loops, returns, short-circuiting, panics/throws, cleanup behavior where supported, unreachable code, unsupported constructs, and deterministic output.

</specifics>

<deferred>
## Deferred Ideas

- Public `Cfg<'_>` SDK view and supported `cfg` capability promotion remain deferred until fixture and documentation evidence justify it.
- Direct call-site/target/unresolved-call facts remain Phase 30.
- P0 abstract-domain transfer functions over CFG remain Phase 31.
- Summary/effects/control-effect summaries remain Phase 32.
- Extension overlay sinks for repo-local Rust providers remain Phase 34 unless a minimal placeholder is needed for future-fit validation.
- Exact promise scheduling, goroutine interleavings, full Go recover semantics, and fully precise TS/JS `finally` paths are deferred unless needed to satisfy Phase 29 honestly.

</deferred>

---

*Phase: 29-local-cfg-and-control-dependence*
*Context gathered: 2026-05-20*
