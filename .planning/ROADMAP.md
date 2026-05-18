# Roadmap: polint v1.2 Static Analysis Engine Implementation

## Current Milestone: v1.2 Static Analysis Engine Implementation

## Milestones

- [x] **v1.0 MVP** - repo-local static analysis framework for Go and TypeScript/JavaScript, shipped 2026-05-02.
- [x] **v1.1 Capability Fulfillment** - capability planning, resolved imports/module graph, and symbol/reference foundations for Go and TS/JS.
- [ ] **v1.2 Static Analysis Engine Implementation** - turn the completed static-analysis research roadmap into a private, validated, cache-aware, agent-extensible analysis engine.

## Current Status

v1.2 is active. Phase numbering continues after the v1.1 roadmap, so this milestone starts at Phase 20. The source of truth is `research/ROADMAP.md`, specifically "Implementation Roadmap: One PR Per Step". Each listed PR becomes exactly one independently reviewable phase in the same order.

## Milestone Goal

Build the internal analysis substrate needed for high-value repo-local static analysis: kernel scheduling, provenance, validation, evaluation, cache keys, semantic backbone, MIR, CFG, calls, summaries, extension sinks, framework models, type/value/alias facts, data flow, evidence, benchmark gates, and carefully promoted SDK/query ergonomics.

The milestone should preserve public API discipline. New internals stay private unless a phase intentionally promotes a supported CLI or SDK contract with tests, docs, and fixture evidence.

## Phase Summary

| Phase | Name | Goal | Requirements |
|-------|------|------|--------------|
| 20 | Private Analysis Kernel Facade | Move current analysis orchestration behind an internal kernel boundary and add provider manifests for existing providers. | SAE-FND-01 |
| 21 | 4/4 | Complete    | 2026-05-17 |
| 22 | 6/6 | Complete    | 2026-05-17 |
| 23 | 5/5 | Complete   | 2026-05-18 |
| 24 | 5/5 | Complete    | 2026-05-18 |
| 25 | 3/4 | In Progress|  |
| 26 | Semantic Index Deepening | Add scopes, richer imports, resolution facts, aliases, generated symbols, unknowns, and stable exports. | SAE-SEM-01 |
| 27 | Layered Module/Package/Topology Graph | Expand topology facts for workspaces, packages, source sets, declared deps, lockfiles, and overlays. | SAE-SEM-02 |
| 28 | Private Semantic MIR and Place Identity | Lower Go and TS/JS function bodies into private MIR with stable place identities. | SAE-SEM-03 |
| 29 | Local CFG and Control Dependence | Build local CFG, dominance, postdominance, and control-dependence facts over MIR. | SAE-SEM-04 |
| 30 | Direct Call Facts | Add direct call-site, target, unresolved-call, and index facts while keeping public call graphs unsupported. | SAE-SEM-05 |
| 31 | P0 Abstract-Domain Kernel | Add deterministic lattice/transfer infrastructure and first local domains. | SAE-INT-01 |
| 32 | Summary Kernel and Direct Summaries | Add typed summary storage, direct summaries, effects, TITO, and summary metadata. | SAE-INT-02 |
| 33 | Demand Queries and Summary SCC Cache | Add demand queries, summary SCC scheduling/cache, and extension-aware quarantine. | SAE-INT-03 |
| 34 | Rust Extension/Provider Sink | Add the first advanced repo-local Rust provider boundary with validation and cache participation. | SAE-INT-04 |
| 35 | Framework Entrypoints and Trust Boundaries | Add native and extension-overlay facts for entrypoints, lifecycle, dispatch, and trust boundaries. | SAE-INT-05 |
| 36 | P0 Type/Value/Place/Alias Substrate | Add normalized type, value, allocation, access-path, narrowing, and alias facts. | SAE-PREC-01 |
| 37 | Refined Call Graph Providers | Add opt-in refined call graph providers over direct calls, summaries, entrypoints, and type/value facts. | SAE-PREC-02 |
| 38 | Local Plus Summary-Projected Data Flow | Add local/interprocedural value flow, model sinks, unknowns, budgets, and query-scoped path search. | SAE-PREC-03 |
| 39 | Slicing, Paths, and Evidence Bundles | Add structured evidence, slices, chops, ranked paths, summary expansion, and evidence rendering. | SAE-PREC-04 |
| 40 | External Benchmark Adapters and Promotion Gates | Add benchmark adapters and promotion reports for precision, recall, runtime, cache, and extension impact. | SAE-PROM-01 |
| 41 | Public SDK Query Views and Agent Ergonomics | Promote only validated query views and agent workflows with stable docs and JSON contracts. | SAE-PROM-02 |

## Phase Details

### Phase 20: Private Analysis Kernel Facade

**Goal:** Move current analysis orchestration behind an internal kernel boundary and add provider manifests for existing providers.
**Requirements:** SAE-FND-01
**Research:** `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md`, `research/implementation-bootstrap/RECOMMENDED_IMPLEMENTATION.md`
**Plans:** 2/2 plans complete

Plans:
- [x] 20-01-PLAN.md - Private kernel facade and runner/CLI delegation
- [x] 20-02-PLAN.md - Internal provider manifests and deterministic provider-order inspection

**Success criteria:**
1. Existing tests pass with current behavior preserved.
2. Runner orchestration delegates through the private kernel facade.
3. Existing source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics providers have internal manifests.
4. Provider order can be inspected through an internal/debug path without adding public SDK surface.

### Phase 21: Provenance, Precision, and Validation Metadata

**Goal:** Add shared internal metadata for fact origin, precision, confidence, validation, stable keys, and deterministic merge behavior.
**Requirements:** SAE-FND-02
**Research:** `research/analysis-kernel/FINAL-REPORT.md`, `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md`, `research/semantic-index/FINAL-REPORT.md`
**Plans:** 4/4 plans complete

Plans:
- [x] 21-01-PLAN.md - Metadata vocabulary and source/syntax sidecar attachment
- [x] 21-02-PLAN.md - Derived provider metadata coverage and missing-metadata detection
- [x] 21-03-PLAN.md - Stable-key merge validation and kernel diagnostics
- [x] 21-04-PLAN.md - Internal provenance debug JSON and compatibility proof

**Success criteria:**
1. Kernel-produced existing facts have internal metadata.
2. Duplicate or conflicting stable keys fail deterministically.
3. Debug JSON can show provenance for files, imports, symbols, and references.
4. Metadata remains internal unless deliberately promoted.

### Phase 22: Internal Evaluation Harness MVP

**Goal:** Add a hidden/internal evaluation model with deterministic expected/observed JSON, generic matchers, metrics, and native fixtures.
**Requirements:** SAE-FND-03
**Research:** `research/evaluation-harness/FINAL-REPORT.md`, `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md`, `research/evaluation-harness/VALIDATION.md`
**Plans:** 6/6 plans complete

Plans:
- [x] 22-01-PLAN.md - Internal evaluation model and deterministic report hashing
- [x] 22-02-PLAN.md - Generic matchers and unified metrics
- [x] 22-03-PLAN.md - Native fixture runner and real kernel observation
- [x] 22-04-PLAN.md - Provenance and current-cache native fixtures
- [x] 22-05-PLAN.md - Synthetic extension rejection and delta fixture
- [x] 22-06-PLAN.md - Required fixture coverage and public-boundary proof

**Success criteria:**
1. Native fixtures can assert facts, graph edges, paths, diagnostics, invariants, and runtime budgets.
2. Expected/observed output is deterministic.
3. Output hashes exclude timestamps and machine-local paths.
4. Harness fixtures cover at least kernel, provenance, cache, and extension invariants.

### Phase 23: Input Snapshots and Cache-Key Vocabulary

**Goal:** Add the typed snapshot and key vocabulary required for correct layered cache invalidation.
**Requirements:** SAE-FND-04
**Research:** `research/incremental-query-engine/FINAL-REPORT.md`, `research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md`, `research/module-graph/RECOMMENDED_IMPLEMENTATION.md`
**Plans:** 5/5 plans complete

Plans:
- [x] 23-01-PLAN.md — Internal digest, cache-key, cache-stat, and provider-output vocabulary
- [x] 23-02-PLAN.md — Deterministic input snapshots for source, config, lifecycle, rule, model, extension, provider, and tool inputs
- [x] 23-03-PLAN.md — Current Go/TS file-fact cache stats instrumentation without layer-cache reuse
- [x] 23-04-PLAN.md — Kernel run report with input snapshot and provider output metadata
- [x] 23-05-PLAN.md — Native eval fixture coverage and public-boundary proof

**Success criteria:**
1. `InputSnapshot`, `Digest`, `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey` exist internally.
2. Cache stats and provider output metadata are recorded.
3. Snapshot tests cover file text, config, Go lifecycle, TS/JS lifecycle, rule digests, model digests, and official tool invocation digests where present.

### Phase 24: Persistent Layer Cache for Existing Cheap Facts

**Goal:** Persist parse/syntax, imports, module facts, symbols/references, and metrics layers with conservative invalidation.
**Requirements:** SAE-FND-05
**Research:** `research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md`, `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md`, `research/semantic-index/RECOMMENDED_IMPLEMENTATION.md`

**Success criteria:**
1. Syntax cache is not invalidated by unrelated rule edits.
2. Module and symbol layers invalidate on import, lifecycle, and config changes.
3. Cache stats report hits and misses deterministically.
4. Stale reuse tests fail safely.

### Phase 25: Rule Manifest, Inspect, and Test Skeleton

**Goal:** Extend rule macro metadata into generated manifests and add the first supported inspect/test loop.
**Requirements:** SAE-FND-06
**Research:** `research/agent-rule-authoring/FINAL-REPORT.md`, `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md`, `research/agent-rule-authoring/VALIDATION.md`

**Success criteria:**
1. Generated `RuleManifest` includes derived fact views, capabilities, and options.
2. `polint inspect rule --format json` has stable JSON coverage.
3. `polint test` can run temp-repo fixtures and assert JSON diagnostics.
4. Temp-repo tests compile generated rules using only `polint::sdk::prelude::*`.

### Phase 26: Semantic Index Deepening

**Goal:** Deepen the semantic index with scopes, richer imports, resolution facts, aliases, generated symbols, unknowns, and stable export identities.
**Requirements:** SAE-SEM-01
**Research:** `research/semantic-index/FINAL-REPORT.md`, `research/semantic-index/RECOMMENDED_IMPLEMENTATION.md`, `research/semantic-index/VALIDATION.md`

**Success criteria:**
1. Fixtures cover resolved, ambiguous, unresolved, generated, alias, import/export, and cross-file references.
2. Unknowns are visible and precision-labeled.
3. Go and TS/JS providers own language-specific extraction behind normalized facts.
4. Stable IDs and export identities survive deterministic cache restore.

### Phase 27: Layered Module/Package/Topology Graph

**Goal:** Expand module topology into workspace roots, packages/projects, source sets, declared requirements, lockfile/tool edges, import-to-package facts, and overlays.
**Requirements:** SAE-SEM-02
**Research:** `research/module-graph/FINAL-REPORT.md`, `research/module-graph/RECOMMENDED_IMPLEMENTATION.md`, `research/module-graph/VALIDATION.md`

**Success criteria:**
1. Go monorepo module-root inference works.
2. TS/JS package and workspace facts are deterministic.
3. Import-to-package facts distinguish source, test, generated, vendor, and external where known.
4. Topology facts participate in relevant cache digests.

### Phase 28: Private Semantic MIR and Place Identity

**Goal:** Add private `analysis::mir` and `analysis::places` lowering for Go and TS/JS function bodies.
**Requirements:** SAE-SEM-03
**Research:** `research/implementation-bootstrap/FINAL-REPORT.md`, `research/implementation-bootstrap/RECOMMENDED_IMPLEMENTATION.md`, `research/abstract-interpretation/implementation/MIR-CONTRACT.md`

**Success criteria:**
1. MIR snapshots are deterministic.
2. Parser AST references do not escape lowering.
3. Places cover locals, parameters, globals, temporaries, fields/properties, indexes, call returns, and unknown roots.
4. Unsupported operations are explicit facts or diagnostics, not silent omissions.

### Phase 29: Local CFG and Control Dependence

**Goal:** Build local CFG nodes, blocks, edges, reachability, dominance, postdominance, and control dependence over MIR.
**Requirements:** SAE-SEM-04
**Research:** `research/cfg-control-flow/FINAL-REPORT.md`, `research/cfg-control-flow/RECOMMENDED_IMPLEMENTATION.md`, `research/cfg-control-flow/VALIDATION.md`

**Success criteria:**
1. Fixtures cover branches, loops, returns, short-circuiting, panics/throws, cleanup behavior where supported, unreachable code, and unsupported constructs.
2. CFG output is deterministic across runs.
3. Control-dependence facts do not require rule authors to traverse raw ASTs.
4. Public CFG promotion remains deferred unless the phase deliberately scopes it.

### Phase 30: Direct Call Facts

**Goal:** Add direct call-site, target, unresolved-call, direct/static resolution, call indexes, and debug snapshots.
**Requirements:** SAE-SEM-05
**Research:** `research/call-graphs/FINAL-REPORT.md`, `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md`, `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md`

**Success criteria:**
1. Fixtures cover direct functions, methods, constructors, member calls, function values as unresolved/unknown, unsupported dynamic calls, and precise statuses.
2. Direct call facts consume semantic references where available.
3. Public `CallGraph<'_>` remains unsupported until promotion gates justify it.
4. Debug snapshots are internal or preview-gated.

### Phase 31: P0 Abstract-Domain Kernel

**Goal:** Add deterministic abstract-domain infrastructure and first local domains over MIR/CFG.
**Requirements:** SAE-INT-01
**Research:** `research/abstract-interpretation/FINAL-REPORT.md`, `research/abstract-interpretation/RECOMMENDED_IMPLEMENTATION.md`, `research/abstract-interpretation/VALIDATION.md`

**Success criteria:**
1. Domain-law tests cover partial order, join, and widening behavior.
2. Transfer monotonicity tests exist.
3. Fixtures expose top, unknown, and budget states honestly.
4. First domains include reachability, nilness/nullishness, truthiness, constants, simple string facts, and cheap initializedness where practical.

### Phase 32: Summary Kernel and Direct Summaries

**Goal:** Add summary keys, stores, typed summary domains, local/direct summaries, effects, TITO, memory-touch approximations, and metadata.
**Requirements:** SAE-INT-02
**Research:** `research/effects-summaries/FINAL-REPORT.md`, `research/effects-summaries/RECOMMENDED_IMPLEMENTATION.md`, `research/effects-summaries/VALIDATION.md`

**Success criteria:**
1. Direct summary snapshots are deterministic.
2. Summary status, precision, and provenance are present.
3. Missing callees produce unknown/havoc summaries rather than silent certainty.
4. Summary cache inputs are explicit.

### Phase 33: Demand Queries and Summary SCC Cache

**Goal:** Add internal demand queries for expensive views, summary SCC scheduling/cache, extension-aware cache quarantine, and query trace output.
**Requirements:** SAE-INT-03
**Research:** `research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md`, `research/effects-summaries/RECOMMENDED_IMPLEMENTATION.md`, `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md`

**Success criteria:**
1. Body edits, public API edits, summary SCC edits, rule option edits, model edits, and extension code edits invalidate the correct layers.
2. Stale extension output is quarantined.
3. Expensive providers are not forced into eager whole-repo execution by default.
4. Query trace/debug output is internal or deliberately gated.

### Phase 34: Rust Extension/Provider Sink

**Goal:** Add the first advanced repo-local Rust model/provider extension boundary with typed sinks and validation.
**Requirements:** SAE-INT-04
**Research:** `research/agent-extension-surface/FINAL-REPORT.md`, `research/agent-extension-surface/RECOMMENDED_IMPLEMENTATION.md`, `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md`

**Success criteria:**
1. Invalid extension facts are rejected before merge.
2. Extension digests affect cache keys.
3. Extension facts carry provenance and precision ceilings.
4. Default-vs-extended eval reports changed facts and unknown reduction.

### Phase 35: Framework Entrypoints and Trust Boundaries

**Goal:** Model entrypoints, routes, handlers, callbacks, jobs, CLIs, MCP tools/resources/prompts, tests, dispatch, and trust boundaries.
**Requirements:** SAE-INT-05
**Research:** `research/framework-entrypoints/FINAL-REPORT.md`, `research/framework-entrypoints/RECOMMENDED_IMPLEMENTATION.md`, `research/framework-entrypoints/VALIDATION.md`

**Success criteria:**
1. Fixtures cover HTTP, CLI/env/stdin, jobs/queues where modeled, test entrypoints, unresolved framework dispatch, and extension-improved discovery.
2. Go and TS/JS have default recognizers for the scoped first tier.
3. Extension overlays can add or refine framework facts with validation.
4. Trust-boundary facts expose uncertainty explicitly.

### Phase 36: P0 Type/Value/Place/Alias Substrate

**Goal:** Add normalized type, value, allocation, access-path, narrowing, and alias facts with explicit alias statuses.
**Requirements:** SAE-PREC-01
**Research:** `research/type-alias-points-to/FINAL-REPORT.md`, `research/type-alias-points-to/RECOMMENDED_IMPLEMENTATION.md`, `research/type-alias-points-to/VALIDATION.md`

**Success criteria:**
1. Fixtures cover receiver narrowing, function values, object/property allocations, field sensitivity limits, unresolved aliases, and official-tool input digests where used.
2. Alias status can distinguish `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, and `Unknown`.
3. Official tooling outputs are normalized into polint-owned facts.
4. Whole-repo points-to remains optional, not mandatory for baseline behavior.

### Phase 37: Refined Call Graph Providers

**Goal:** Add opt-in refined call providers over direct calls, entrypoints, summaries, type/value facts, function tokens, receiver types, and bounded points-to constraints.
**Requirements:** SAE-PREC-02
**Research:** `research/call-graphs/FINAL-REPORT.md`, `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md`, `research/call-graphs/VALIDATION.md`

**Success criteria:**
1. Native fixtures and eval suites measure direct versus refined edges.
2. Precision and status are attached to every edge.
3. Dynamic dispatch and framework edges retain provenance.
4. Unresolved and budget-exceeded statuses remain explicit.

### Phase 38: Local Plus Summary-Projected Data Flow

**Goal:** Add local value-flow graph, direct-call interprocedural edges, summary-projected edges, model sinks, budgets, unknowns, and query-scoped path search.
**Requirements:** SAE-PREC-03
**Research:** `research/data-flow/FINAL-REPORT.md`, `research/data-flow/RECOMMENDED_IMPLEMENTATION.md`, `research/data-flow/implementation/BOOTSTRAP-INTEGRATION.md`

**Success criteria:**
1. Fixtures cover local flow, parameter/return flow, sanitizer/barrier behavior, missing summaries, extension-added flows, false-positive traps, and deterministic budget handling.
2. Source, sink, sanitizer, and barrier model facts have provenance.
3. Unknown/havoc facts are visible.
4. Query-scoped path search has explicit limits.

### Phase 39: Slicing, Paths, and Evidence Bundles

**Goal:** Add internal evidence nodes/edges, thin/full slices, chops, ranked paths, summary expansion handles, and evidence rendering.
**Requirements:** SAE-PREC-04
**Research:** `research/program-slicing-evidence/FINAL-REPORT.md`, `research/program-slicing-evidence/RECOMMENDED_IMPLEMENTATION.md`, `research/program-slicing-evidence/VALIDATION.md`

**Success criteria:**
1. Fixtures cover local dependence, interprocedural direct-call evidence, summary expansion, extension evidence, uncertainty markers, deterministic ranking, and compact path limits.
2. Diagnostic evidence includes provenance and uncertainty.
3. JSON/SARIF evidence rendering is deterministic.
4. Evidence bundles remain bounded for large findings.

### Phase 40: External Benchmark Adapters and Promotion Gates

**Goal:** Add benchmark adapters and reports that support public precision claims.
**Requirements:** SAE-PROM-01
**Research:** `research/evaluation-harness/FINAL-REPORT.md`, `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md`, `research/data-flow/VALIDATION.md`, `research/call-graphs/VALIDATION.md`

**Success criteria:**
1. Reports show TP, FP, FN, precision, recall, F-score, unknown counts, graph/path metrics, runtime, memory, cache reuse, extension overhead, and accepted/rejected extension facts.
2. Native fixtures remain the first promotion gate before external suites.
3. External adapters are added in the order justified by the harness research.
4. Public claims are tied to measured reports.

### Phase 41: Public SDK Query Views and Agent Ergonomics

**Goal:** Promote only validated typed views, bounded query builders, and agent authoring commands whose contracts are ready.
**Requirements:** SAE-PROM-02
**Research:** `research/agent-rule-authoring/FINAL-REPORT.md`, `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md`, `research/ROADMAP.md`

**Success criteria:**
1. Temp-repo rule tests consume public SDK views only.
2. Docs explain limits, precision tiers, and heuristic behavior.
3. Accepted public commands have stable JSON contracts.
4. Expensive queries require limits or explicit unbounded mode.

## Release Gate For Each Phase

Each phase should ship with:

- Compiling code and existing behavior preserved.
- Tests or fixtures proving the new contract.
- Deterministic outputs where facts, diagnostics, cache keys, or reports are involved.
- Public API discipline: new internals stay private unless intentionally promoted.
- External-consumer tests using `polint::sdk::prelude::*` when public rule-author behavior changes.
- Docs under `docs/facts/` or relevant user docs when public facts or commands are promoted.
- Honest unsupported, setup-missing, unknown, ambiguous, or budget-exceeded states instead of silent empty facts.

## Traceability

All 22 v1.2 requirements are mapped in [`REQUIREMENTS.md`](REQUIREMENTS.md#traceability). No v1.2 requirements are currently unmapped.

## Source Material

- `research/ROADMAP.md`
- `research/analysis-kernel/`
- `research/evaluation-harness/`
- `research/incremental-query-engine/`
- `research/agent-rule-authoring/`
- `research/semantic-index/`
- `research/module-graph/`
- `research/implementation-bootstrap/`
- `research/cfg-control-flow/`
- `research/call-graphs/`
- `research/abstract-interpretation/`
- `research/effects-summaries/`
- `research/agent-extension-surface/`
- `research/framework-entrypoints/`
- `research/type-alias-points-to/`
- `research/data-flow/`
- `research/program-slicing-evidence/`
