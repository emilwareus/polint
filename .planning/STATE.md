---
gsd_state_version: 1.0
milestone: v1.2
milestone_name: Static Analysis Engine Implementation
status: executing
last_updated: "2026-05-24T11:02:48.781Z"
last_activity: 2026-05-24 -- Phase 36 planning complete
progress:
  total_phases: 22
  completed_phases: 16
  total_plans: 100
  completed_plans: 93
  percent: 73
---

# State: polint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-21)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** Phase 36 — p0-type-value-place-alias-substrate

## Current Status

- **GitHub:** `emilwareus/polint` (public repository name).
- Active branch policy: do not push directly to `main` unless explicitly instructed; create a feature/fix branch before sharing remote work.
- v1.0 MVP was audited, archived, tagged, and closed on 2026-05-02.
- v1.1 Capability Fulfillment completed the capability plan, resolved imports/module graph, and symbols/references foundations.
- Static-analysis engine research completed on 2026-05-16 in `research/ROADMAP.md`.
- v1.2 requirements are defined in `.planning/REQUIREMENTS.md`.
- v1.2 roadmap is defined in `.planning/ROADMAP.md`.
- Phase 22 has been shipped for review in PR #22: https://github.com/emilwareus/polint/pull/22.
- Phase 24 has been shipped for review in PR #25: https://github.com/emilwareus/polint/pull/25.
- Phase 29 has been shipped for review in PR #34: https://github.com/emilwareus/polint/pull/34.
- Each v1.2 research PR maps to one GSD phase, in order, from Phase 20 through Phase 41.
- New broad research is not needed by default. Use the relevant research documents referenced by each phase; do additional research only for a concrete implementation gap.

## Current Position

Milestone: v1.2 Static Analysis Engine Implementation
Status: Ready to execute
Phase: 36 (p0-type-value-place-alias-substrate) — NOT STARTED
Plan: 0 of unknown
Last activity: 2026-05-24 -- Phase 36 planning complete

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 20 | Complete | 2/2 plans complete; private kernel facade/delegation plus internal provider manifests/order inspection done |
| 21 | Complete | 4/4 plans complete; provenance, precision, validation metadata, deterministic debug JSON, and public compatibility proof done; requirement SAE-FND-02 |
| 22 | Complete | 6/6 plans complete; evaluation model/report hashing, generic matchers/metrics, native fixture runner, provenance/cache/extension fixtures, fixture category coverage, and public-boundary proof done; requirement SAE-FND-03 |
| 23 | Pending | Input snapshots and cache-key vocabulary; requirement SAE-FND-04 |
| 24 | Complete | 5/5 plans complete; persistent layer cache proof, stale-safety, public-boundary coverage, and full verification done; requirement SAE-FND-05 |
| 25 | Pending | Rule manifest, inspect, and test skeleton; requirement SAE-FND-06 |
| 26 | Complete | 6/6 plans complete; semantic index contracts, TS/JS and Go semantic rows, validation/debug output, cache persistence, eval fixtures, and public-boundary proof done; requirement SAE-SEM-01 |
| 27 | Complete | 7/7 plans complete; topology contracts, Go/TS topology collectors, provider/cache wiring, module topology provider, eval fixtures, public-boundary proof, and docs alignment done; requirement SAE-SEM-02 |
| 28 | Complete | 7/7 plans complete; private MIR/place contracts, semantic store, Go and TS/JS lowering, provider/cache/debug wiring, semantic-MIR eval snapshots, and public-boundary proof done; requirement SAE-SEM-03 |
| 29 | Complete | 6/6 plans complete; private CFG contracts/storage, shared builder/derived analyses, provider/cache/validation/debug wiring, Go CFG lowering, TS/JS CFG lowering, eval fixtures, and public-boundary proof done; requirement SAE-SEM-04 |
| 30 | Complete | 8/8 plans complete; direct call contracts, provider/cache identity, validation/debug snapshots, MIR call-site extraction, direct targets, unresolved evidence, eval observation/fixtures, and public-boundary proof done; requirement SAE-SEM-05 |
| 31 | Complete | 5/5 plans complete; private domain contracts, deterministic local solver, stored domain facts, provider/cache identity, validation, debug JSON, abstract-domain eval fixtures, public-boundary proof, review fixes, and final verification done; requirement SAE-INT-01 |
| 32 | Complete | 7/7 plans complete; summary kernel contracts, store, builder, provider, cache identity, validation, debug, eval fixtures, and public-boundary proof done; requirement SAE-INT-02 |
| 33 | Complete | 7/7 plans complete; demand queries, summary SCC cache, extension-aware quarantine, eval fixtures, public-boundary proof, review fixes, and final verification done; requirement SAE-INT-03 |
| 34 | Complete | 6/6 plans complete; Rust extension discovery/host/protocol, sink validation, kernel integration, cache identity/quarantine, real extension eval, review fixes, and final verification done; requirement SAE-INT-04 |
| 35 | Complete | 8/8 plans complete; framework fact contracts, provider wiring, Go/TS recognizers, trust boundaries, dispatch, validation, eval fixtures, public no-leak proof, and clippy cleanup done; requirement SAE-INT-05 |
| 36 | Pending | P0 type/value/place/alias substrate; requirement SAE-PREC-01 |
| 37 | Pending | Refined call graph providers; requirement SAE-PREC-02 |
| 38 | Pending | Local plus summary-projected data flow; requirement SAE-PREC-03 |
| 39 | Pending | Slicing, paths, and evidence bundles; requirement SAE-PREC-04 |
| 40 | Pending | External benchmark adapters and promotion gates; requirement SAE-PROM-01 |
| 41 | Pending | Public SDK query views and agent ergonomics; requirement SAE-PROM-02 |

## Accumulated Context

- Product code and GSD planning documents live together in the repository root on `main`.
- Public API discipline is strict: use `pub(crate)` for internals, promote only curated SDK/runner surfaces, and fix `unreachable_pub` by tightening visibility.
- Rule-author examples and temp-repo tests must consume `polint::sdk::prelude::*` and `polint::runner::run_cli`, not internal modules.
- Capability names must stay honest: unsupported or setup-missing hard capabilities produce capability diagnostics rather than placeholder facts.
- Comment ignores are an engine/reporting concern; individual rules should report the diagnostics they find.
- Go semantic lifecycle must support monorepos without requiring a root `go.mod`; module roots are inferred or configured in `.polint.toml`.
- New analysis modules for v1.2 should stay private until validation and promotion gates justify public SDK or CLI exposure.
- Every new fact family should carry stable IDs, precision/status/provenance, deterministic ordering, cache inputs, validation fixtures, and explicit unknown states.
- Phase 20 Plan 01 added a crate-private `AnalysisKernel` facade that owns the existing source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics execution order.
- Runner and parent CLI analysis paths delegate provider execution through `AnalysisKernel::run`; rule selection, rule options, ignores, report filtering/rendering, exit behavior, and rule execution remain outside the kernel.
- Phase 20 Plan 02 added deterministic crate-private provider manifests for the six current providers and test-only provider order/report helpers.
- Provider manifests are consumed by production kernel code only for metadata consistency; they do not drive scheduling, diagnostics, or cache identity.

## Decisions

- Keep `AnalysisKernel`, `KernelInput`, and `KernelOutput` crate-private with no new SDK, crate-root public, or CLI surface.
- Preserve the existing eager provider order inside the kernel until provider manifests and order inspection land in Plan 20-02.
- Merge module graph support over the static plan support view, then symbol graph support over module support, before rules run.
- [Phase 20-private-analysis-kernel-facade]: Keep provider manifests crate-private and consume them only for behavior-preserving metadata consistency in this phase.
- [Phase 20-private-analysis-kernel-facade]: Keep provider execution order as explicit AnalysisKernel::run calls; manifest dependency data remains deterministic test metadata only.
- [Phase 20-private-analysis-kernel-facade]: Expose provider order inspection only through #[cfg(test)] crate-private helpers, with no SDK, runner, or CLI contract.
- [Phase 21-provenance-precision-and-validation-metadata]: Metadata stays in an AnalysisDb sidecar rather than widening public fact structs.
- [Phase 21-provenance-precision-and-validation-metadata]: Provider IDs polint.source, polint.go.syntax, and polint.ts.syntax are reused as producer and layer IDs for current source/syntax facts.
- [Phase 21-provenance-precision-and-validation-metadata]: Stable keys are deterministic strings built from sorted, normalized, length-prefixed labeled parts while run-local FactRef IDs remain separate.
- [Phase 21-provenance-precision-and-validation-metadata]: Derived provider metadata uses hard-coded manifest IDs polint.module_graph, polint.symbol_graph, and polint.metrics.
- [Phase 21-provenance-precision-and-validation-metadata]: Symbol, definition, and reference metadata stable keys reuse the existing symbol graph stable_key fields exactly.
- [Phase 21-provenance-precision-and-validation-metadata]: The missing metadata report stays crate-private and test-facing, with a debug assertion keeping the invariant live inside the kernel.
- [Phase 21-provenance-precision-and-validation-metadata]: Stable-key ownership is keyed by (FactFamily, stable_key); conflicting payloads keep existing fact rows but become deterministic validation diagnostics.
- [Phase 21-provenance-precision-and-validation-metadata]: Metadata validation runs after metrics derivation and before KernelOutput is returned to rule execution.
- [Phase 21-provenance-precision-and-validation-metadata]: Provider precision ceilings allow lower-confidence precision labels while flagging syntax providers that claim Exact or SetupAware output.
- [Phase 21-provenance-precision-and-validation-metadata]: Metadata debug JSON remains behind cfg(test) and crate-private AnalysisKernel helpers, with no SDK, runner, or public CLI surface.
- [Phase 21-provenance-precision-and-validation-metadata]: Debug rows use SourceFile.relative_path and explicit row sorting by path/span/name/stable key/run id to avoid machine-local or transient details.
- [Phase 21-provenance-precision-and-validation-metadata]: Public compatibility is proven through a temp-repo external rule importing only polint::sdk::prelude::* and checking metadata-only keys stay out of public JSON.
- [Phase 22-internal-evaluation-harness-mvp]: Keep eval crate-private and internal; no public SDK, runner, crate-root public, or CLI contract was introduced.
- [Phase 22-internal-evaluation-harness-mvp]: Normalize reports by sorting cases, expected items, observed items, and matches before serialization and hashing.
- [Phase 22-internal-evaluation-harness-mvp]: Compute output hashes from canonical JSON with output_hash cleared and runtime durations removed, while preserving runtime pass/fail semantics.
- [Phase 22-internal-evaluation-harness-mvp]: Use a scoped dead_code lint expectation on the eval module until later Phase 22 plans consume the foundation types.
- [Phase 22-internal-evaluation-harness-mvp]: Keep matcher and metric logic crate-private and pure over normalized in-memory eval rows.
- [Phase 22-internal-evaluation-harness-mvp]: Represent matcher outcomes as typed report data instead of outcome strings so metrics can aggregate deterministically.
- [Phase 22-internal-evaluation-harness-mvp]: Clear observed runtime durations from match summaries before deterministic output hashing, preserving pass/fail semantics without wall-clock hash input.
- [Phase 22-internal-evaluation-harness-mvp]: Extend the existing MetricSummary report type from Plan 22-01 instead of adding a duplicate metric report shape.
- [Phase 22-internal-evaluation-harness-mvp]: 22-03 kept native fixture loading, observation, and execution crate-private/test-facing under eval with no public CLI or SDK surface.
- [Phase 22-internal-evaluation-harness-mvp]: 22-03 copies fixture repos into temporary directories before AnalysisKernel::run and rejects symlink escape during fixture copy.
- [Phase 22-internal-evaluation-harness-mvp]: 22-03 sources provider-order observations from AnalysisKernel::provider_manifests() and keeps exact runtime durations out of deterministic output hashes.
- [Phase 22-internal-evaluation-harness-mvp]: 22-04 keeps provenance and cache fixtures crate-private/test-facing with no public CLI, SDK, runner, or crate-root surface.
- [Phase 22-internal-evaluation-harness-mvp]: 22-04 expected fact matching honors producer_id, precision, and status when manifests specify them, with partial stable-key matching for content-hash-bearing metadata rows.
- [Phase 22-internal-evaluation-harness-mvp]: 22-04 derives cache.current_determinism only after cold, warm, and no-cache fixture runs have matching normalized JSON and output_hash values.
- [Phase 22-internal-evaluation-harness-mvp]: 22-05 keeps synthetic observed rows manifest-owned, test-facing, and rejected outside extension fixtures.
- [Phase 22-internal-evaluation-harness-mvp]: 22-05 counts present, accepted, and rejected observed fact statuses separately in eval metrics.
- [Phase 22-internal-evaluation-harness-mvp]: 22-05 represents extension delta evidence with normalized invariant rows and extension.real_sink_active = false.
- [Phase 22-internal-evaluation-harness-mvp]: 22-05 adds no real extension provider activation, merge surface, CLI, SDK, or runner contract.
- [Phase 22-internal-evaluation-harness-mvp]: 22-06 keeps Phase 22 eval proof entirely test-facing with no public eval CLI, SDK export, runner entrypoint, or documented schema.
- [Phase 22-internal-evaluation-harness-mvp]: 22-06 proves suite category coverage by executing every native fixture manifest and requiring passing kernel, provenance, cache, and extension areas.
- [Phase 22-internal-evaluation-harness-mvp]: 22-06 uses repeated minimal public check JSON output as the no-leak and determinism guard.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Layer-cache persistence remains crate-private under analysis_kernel::incremental with no SDK, runner, CLI, or public JSON surface.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Layer payloads use digest-derived blob paths and manifests are published last under .polint/cache/layers.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Invalidation planning fails closed for unknown, schema, provider, lifecycle, toolchain, model, extension, and missing dependency cases.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Existing key structs derive ordering so CacheNode can support deterministic BTreeMap indexes.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Syntax layer identity excludes rule code, rule options, and downstream diagnostic identity; parser reuse is keyed by parser/source/config/lifecycle/provider inputs.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Go and TS/JS syntax layer payloads store normalized facts and parser diagnostics, not raw source bodies or absolute temp roots.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Adapter provider-output metadata reuses validated layer read output digests on hits and computes output digests after recompute misses.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Cache hit/miss/reuse counters remain internal; CLI compatibility is guarded by public PolintReport parsing and no-leak assertions.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Module graph cache identity includes provider/schema, import shape, source/package, config, Go lifecycle, TS/JS lifecycle, absent toolchain/extension slots, and upstream Go/TS syntax output digests.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Module graph cache hits restore normalized facts through AnalysisDb::replace_module_graph_facts instead of bypassing metadata normalization.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Disabled module graph caching records bypasses_disabled and recomputes without reading or writing layer-cache files.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Module graph cache stats remain internal to KernelRunReport and do not change public check JSON.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Symbol graph cache identity includes source/function/package/import inputs, lifecycle/config digests, module graph output digest, syntax output digests, provider/schema identity, and absent extension/toolchain slots.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Symbol graph and metrics cache hits restore normalized facts through existing AnalysisDb::replace_* paths so metadata and public SDK behavior stay compatible.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Metrics cache identity includes source/function inputs, upstream syntax output digests, provider/schema identity, config digest, and absent extension/toolchain slots.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Derived provider cache stats remain internal to KernelRunReport; public check JSON and SDK surfaces are unchanged.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Layer-cache eval uses an explicit capability-requesting AnalysisPlan so all Phase 24 providers run through real cache paths.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: LayerCacheStore rejects invalid manifests before payload reads, including dependency-index schema drift and derived-layer manifests without dependency rows.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: Layer-cache internals remain test/eval-facing only; public JSON, CLI help, SDK, runner, and crate-root surfaces are guarded by integration tests.
- [Phase 24-persistent-layer-cache-for-existing-cheap-facts]: The public cache status contract includes the managed layers category but still does not expose layer-cache internals or provider stats.
- [Phase 26]: Phase 26 context gathered at .planning/phases/26-semantic-index-deepening/26-CONTEXT.md
- [Phase 26-semantic-index-deepening]: Keep semantic index rows crate-private under symbol_graph::semantic with no SDK, runner, CLI, or crate-root public surface.
- [Phase 26-semantic-index-deepening]: Use polint.symbol_graph as producer/layer id for semantic metadata rows.
- [Phase 26-semantic-index-deepening]: Assign semantic run-local IDs by sorted stable keys while keeping stable keys separate from IDs.
- [Phase 26-semantic-index-deepening]: Keep TS/JS semantic rows crate-private under symbol_graph::semantic with no SDK, runner, CLI, or crate-root public surface.
- [Phase 26-semantic-index-deepening]: Use Oxc scopes and references as the TS/JS semantic source, with conservative rows for unresolved, dynamic, external, and unsupported forms.
- [Phase 26-semantic-index-deepening]: Represent TS/JS stable export identities with a native generated discriminator while future plans decide DB/cache publication.
- [Phase 26-semantic-index-deepening]: Use the existing Go lifecycle and sidecar path, adding semantic rows without writing repository lifecycle files.
- [Phase 26-semantic-index-deepening]: Keep Go semantic rows crate-private under symbol_graph::semantic with no SDK, runner, CLI, or crate-root public surface.
- [Phase 26-semantic-index-deepening]: Represent Go setup gaps and unresolved sidecar references as UnknownFallback semantic rows while preserving polint/capability diagnostics.
- [Phase 26-semantic-index-deepening]: Keep semantic closure, generated hooks, validation, and debug JSON crate-private/test-only for plan 26-04.
- [Phase 26-semantic-index-deepening]: Semantic metadata from polint.symbol_graph must not claim FactPrecision::Exact; setup-aware precision is enforced by validation.
- [Phase 26-semantic-index-deepening]: Native generated hooks are polint.symbol_graph rows with source_stable_key, generated_discriminator, and GeneratedHintLookup provenance.
- [Phase 26-semantic-index-deepening]: Keep semantic cache identity and payload restore crate-private under the existing symbol graph provider.
- [Phase 26-semantic-index-deepening]: Use schema symbol-graph-facts-2 for symbol graph layer payloads that include semantic_index rows.
- [Phase 26-semantic-index-deepening]: Reject malformed semantic cache payloads before reuse instead of restoring partial or placeholder semantic facts.
- [Phase 26-semantic-index-deepening]: Keep semantic eval support crate-private/test-facing; no public eval CLI, SDK view, or generic semantic graph API was added.
- [Phase 26-semantic-index-deepening]: Represent semantic unknown statuses explicitly in eval reports so ambiguous, unresolved, dynamic, external, cycle, generated, setup-missing, and unsupported rows count as unknown evidence.
- [Phase 26-semantic-index-deepening]: Document only existing Symbols<'_> and References<'_> behavior; scopes/import closure/resolution-step rows remain internal.
- [Phase 27-layered-module-package-topology-graph]: Keep topology contracts crate-private under module_graph::topology with no SDK, runner, CLI, crate-root, or public docs promotion.
- [Phase 27-layered-module-package-topology-graph]: Use polint.module_graph for base topology metadata and polint.module_topology for import-to-package metadata.
- [Phase 27-layered-module-package-topology-graph]: Advertise only base topology outputs on the existing polint.module_graph provider; import_to_package_edges remains deferred to the later semantic-aware module topology pass.
- [Phase 27-layered-module-package-topology-graph]: Go module topology reuses GoAnalysisConfig::from_loaded so configured module_roots take precedence and nearest go.mod discovery remains centralized.
- [Phase 27-layered-module-package-topology-graph]: go.mod requirements, replace/exclude directives, and go.sum checksum rows remain separate topology facts rather than import or DependsOn edges.
- [Phase 27-layered-module-package-topology-graph]: Missing go.sum evidence for external requirements is represented as explicit MissingLockfile topology uncertainty.
- [Phase 27-layered-module-package-topology-graph]: Represent package-manager and tsconfig evidence as internal repo topology overlay rows until a dedicated manager-evidence fact family is introduced.
- [Phase 27-layered-module-package-topology-graph]: Treat package-lock.json packages as exact lockfile-selected rows while marking pnpm, Yarn, and Bun lockfile presence as unsupported evidence.
- [Phase 27-layered-module-package-topology-graph]: Use workspace: dependency ranges to override the dependency-section kind with RequirementKind::Workspace.
- [Phase 27-layered-module-package-topology-graph]: Base topology is stored by the existing polint.module_graph provider immediately after resolved imports, module nodes, and module edges are replaced.
- [Phase 27-layered-module-package-topology-graph]: Module graph layer payload schema v2 persists base topology rows but keeps import_to_package_edges out for the later semantic-aware topology pass.
- [Phase 27-layered-module-package-topology-graph]: Topology cache identity hashes checked-in manifest, lockfile, workspace, and tsconfig files under topology-relevant roots while preserving absent-only extension handling.
- [Phase 27-layered-module-package-topology-graph]: Add semantic-aware import-to-package facts in crate-private polint.module_topology instead of widening public module graph contracts.
- [Phase 27-layered-module-package-topology-graph]: Run module topology after polint.symbol_graph so semantic import rows are available without creating a provider cycle.
- [Phase 27-layered-module-package-topology-graph]: Reject duplicate cached import-to-package stable keys before restore so stale or conflicting topology payloads are recomputed.
- [Phase 27-layered-module-package-topology-graph]: Kept topology eval observation crate-private and test-facing, with no SDK, runner, CLI, or public crate-root topology API.
- [Phase 27-layered-module-package-topology-graph]: Represented topology expected rows through stable keys, status labels, precision labels, and compact payload fragments instead of raw source or absolute paths.
- [Phase 27-layered-module-package-topology-graph]: Updated existing layer-cache expectations so polint.module_topology is part of the managed provider cache proof.
- [Phase 27-layered-module-package-topology-graph]: Keep Phase 27 topology internals private and prove the boundary with public CLI JSON, help text, and source-surface assertions rather than adding any SDK topology view.
- [Phase 27-layered-module-package-topology-graph]: Document ResolvedImports<'_> and ModuleGraphFacts<'_> as the supported relationship surfaces while explicitly leaving richer package/workspace topology internals outside SDK facts.
- [Phase 28-private-semantic-mir-and-place-identity]: Keep the new analysis module crate-private and expose no SDK, runner, CLI, or public docs surface.
- [Phase 28-private-semantic-mir-and-place-identity]: Use run-local dense IDs only as handles; persistent place and MIR identity is carried by stable keys.
- [Phase 28-private-semantic-mir-and-place-identity]: Represent unsupported semantics as structured rows with source evidence and conservative action labels.
- [Phase 28-private-semantic-mir-and-place-identity]: Keep stored semantic MIR artifacts behind AnalysisDb crate-private accessors and SemanticStore rather than adding SDK or RuleCtx views.
- [Phase 28-private-semantic-mir-and-place-identity]: Use polint.semantic_mir as the internal producer/layer id and map stored MIR precision conservatively, never Exact.
- [Phase 28-private-semantic-mir-and-place-identity]: Treat public-boundary proof as source-surface tests over SDK, runner, docs, README, and _bench.
- [Phase 28-private-semantic-mir-and-place-identity]: Keep Go MIR lowering crate-private under analysis::mir::lower_go with no SDK, runner, CLI, docs, or public JSON surface.
- [Phase 28-private-semantic-mir-and-place-identity]: Draft MIR operations against stable place keys, then resolve to run-local PlaceId values only after PlaceTableBuilder assigns deterministic dense IDs.
- [Phase 28-private-semantic-mir-and-place-identity]: Represent Go calls only as MirOperationKind::Call shape evidence and emit UnsupportedSemanticFact rows for dynamic/control constructs instead of direct-call facts.
- [Phase 28-private-semantic-mir-and-place-identity]: Keep TS/JS MIR lowering crate-private under analysis::mir::lower_ts with no SDK, runner, CLI, docs, or public JSON surface.
- [Phase 28-private-semantic-mir-and-place-identity]: Use Oxc AST nodes only inside the lowering pass; emitted MIR/place rows contain polint-owned IDs, spans, stable keys, roots, projections, operations, and unsupported facts.
- [Phase 28-private-semantic-mir-and-place-identity]: Represent TS/JS calls only as MirOperationKind::Call shape evidence with call-return places; no direct target facts or call graph surface was added.
- [Phase 28-private-semantic-mir-and-place-identity]: Semantic MIR remains private and crate-internal; no SDK, runner, CLI, or public JSON surface was promoted.
- [Phase 28-private-semantic-mir-and-place-identity]: Malformed unsupported semantic rows are stored and rejected by validation so diagnostics carry stable family/stable_key/field/reason evidence.
- [Phase 28-private-semantic-mir-and-place-identity]: Semantic MIR cache identity includes absent extension, model, and toolchain slots even before those inputs exist.
- [Phase 28-private-semantic-mir-and-place-identity]: Keep semantic-MIR eval observation crate-private and test-facing, sourced only from metadata_debug_json_for_test.
- [Phase 28-private-semantic-mir-and-place-identity]: Use compact semicolon payload fragments for MIR eval evidence instead of raw source, AST dumps, absolute paths, or dense IDs as identity.
- [Phase 28-private-semantic-mir-and-place-identity]: Treat Partial semantic-MIR rows as unknown-like evidence in matcher outcomes and metrics.
- [Phase 28-private-semantic-mir-and-place-identity]: Keep semantic MIR/place internals out of public check JSON, inspect JSON, polint test JSON, CLI help, SDK, runner, crate-root public exports, README, and docs.
- [Phase 28-private-semantic-mir-and-place-identity]: Use an external temp-repo rule that requests only supported public fact views to prove existing rule-author workflows remain compatible.
- [Phase 28-private-semantic-mir-and-place-identity]: Offset private MIR/place/unsupported IDs per language output before merge so validation does not cross-wire Go and TS/JS run-local IDs.
- [Phase 29-local-cfg-and-control-dependence]: Keep CFG contracts crate-private with no SDK, runner, CLI, or docs promotion.
- [Phase 29-local-cfg-and-control-dependence]: Use run-local dense IDs only as handles; persistent CFG identity is carried by stable keys.
- [Phase 29-local-cfg-and-control-dependence]: Preserve duplicate CFG rows during normalization so later validation can report conflicts deterministically.
- [Phase 29-local-cfg-and-control-dependence]: Drive language CFG lowering through one shared builder rather than duplicating graph construction per language.
- [Phase 29-local-cfg-and-control-dependence]: Derive reachability, dominators, postdominators, and control dependence from selected graph views instead of storing language-authored derived rows.
- [Phase 29-local-cfg-and-control-dependence]: Use a synthetic unified exit for postdominance and preserve controlling edge evidence on control-dependence facts.
- [Phase 29-local-cfg-and-control-dependence]: Run polint.cfg after polint.semantic_mir and before polint.metrics.
- [Phase 29-local-cfg-and-control-dependence]: Accept an empty CFG provider output until language lowering plans populate real graph rows.
- [Phase 29-local-cfg-and-control-dependence]: Keep CFG validation and debug output crate-private/test-facing with no SDK, runner, CLI, or public JSON surface.
- [Phase 29-local-cfg-and-control-dependence]: Lower Go CFG from private semantic MIR rows and keep raw tree-sitter AST objects out of CFG facts.
- [Phase 29-local-cfg-and-control-dependence]: Keep language CFG lowerers responsible for base nodes/edges only; shared provider code derives reachability, dominance, postdominance, and control dependence.
- [Phase 29-local-cfg-and-control-dependence]: Represent Go spawn, defer, panic, select, goto, fallthrough, and unsupported semantics with typed CFG edges or unsupported control-flow rows instead of exact claims.
- [Phase 29-local-cfg-and-control-dependence]: Lower TS/JS CFG from private semantic MIR rows and keep Oxc AST/span objects out of CFG facts.
- [Phase 29-local-cfg-and-control-dependence]: Merge language base CFG outputs with deterministic run-local ID offsets before deriving shared CFG analyses.
- [Phase 29-local-cfg-and-control-dependence]: Represent TS/JS dynamic, async, cleanup, optional/nullish, throw, and unsupported semantics with typed CFG edges or unsupported control-flow rows instead of exact scheduler/runtime claims.
- [Phase 29-local-cfg-and-control-dependence]: Keep CFG eval support crate-private and test-facing, sourced only from metadata_debug_json_for_test.
- [Phase 29-local-cfg-and-control-dependence]: Use the existing TOML eval fixture manifest format instead of adding JSON fixture files.
- [Phase 29-local-cfg-and-control-dependence]: CFG stable keys must use MIR/body stable identity, not run-local CFG IDs, to avoid cross-language and cross-function collisions.
- [Phase 29-local-cfg-and-control-dependence]: Keep reserved public cfg capability unsupported until a later intentional promotion phase.
- [Phase 30-direct-call-facts]: Call facts remain crate-private under analysis::calls with no SDK, runner, CLI, or docs promotion.
- [Phase 30-direct-call-facts]: CallStore validates target and unresolved site references before publishing indexes.
- [Phase 30-direct-call-facts]: CALLS_PROVIDER_ID is polint.calls and call metadata uses compact status/kind/algorithm/reason/stable-key payload fragments.
- [Phase 30-direct-call-facts]: polint.calls remains crate-private and manifest-owned, with no SDK, runner, CLI, or public call graph promotion.
- [Phase 30-direct-call-facts]: The calls provider runs after polint.cfg and before polint.metrics so direct calls can consume CFG/MIR context before metrics remain unchanged.
- [Phase 30-direct-call-facts]: Calls cache identity includes semantic MIR, CFG, symbol graph, module topology, syntax, lifecycle, config, parameters, and absent extension/model/toolchain slots.
- [Phase 30-direct-call-facts]: Call validation remains crate-private under analysis::calls and is invoked from metadata validation after CFG validation.
- [Phase 30-direct-call-facts]: Calls debug snapshots stay behind cfg(test) and expose relative paths, stable keys, spans, statuses, precision, compact payload labels, counts, and index evidence only.
- [Phase 30-direct-call-facts]: Exact metadata precision from polint.calls is rejected because call facts are setup-aware/conservative internal rows, not public exact facts.
- [Phase 30-direct-call-facts]: Call-site extraction consumes semantic MIR and place rows only; no parser AST or source reparsing dependency was added.
- [Phase 30-direct-call-facts]: Direct targets remain empty in this plan; function-value, dynamic, unknown, setup-missing, and unsupported call evidence is published as unresolved rows.
- [Phase 30-direct-call-facts]: Call output digest proof now covers provider-derived populated sites and unresolved rows, while direct target coverage remains in the later direct-target plan.
- [Phase 30-direct-call-facts]: Direct targets are emitted only from precise resolved ReferenceFact evidence; dynamic/interface/function-token/framework/value-flow cases remain unresolved or unsupported.
- [Phase 30-direct-call-facts]: Native direct target rows use NativeDirect provenance and SetupAware precision under the private polint.calls provider.
- [Phase 30-direct-call-facts]: Provider-derived unresolved rows are filtered off call sites that have a resolved direct target, so precise evidence wins over dynamic-shape uncertainty.
- [Phase 30-direct-call-facts]: Eval call observation stays crate-private/test-facing; no public SDK, runner, CLI, docs, or call graph API was promoted.
- [Phase 30-direct-call-facts]: Call eval payloads use relative path, source span, status/kind/algorithm/reason/provider, and stable-key target identity only.
- [Phase 30-direct-call-facts]: Existing matcher/metrics/report unknown-like status accounting already covered unresolved, unsupported, and setup_missing; plan-specific tests now prove it for call rows.
- [Phase 30-direct-call-facts]: Plan 30-07 kept direct-call fixture coverage internal and test-facing; no public CallGraph API was exposed.
- [Phase 30-direct-call-facts]: Plan 30-07 uses nonzero eval invariants for direct-call debug count and D-10 index coverage instead of fragile exact counts.
- [Phase 30-direct-call-facts]: Plan 30-07 derives missing call-site owner symbols from existing function/symbol facts before call-store indexing.
- [Phase 30-direct-call-facts]: Plan 30-08 kept direct-call internals private and test-facing; no SDK, runner, CLI, README, or docs/facts call surface was promoted.
- [Phase 30-direct-call-facts]: Plan 30-08 kept CallGraph as an inert reserved SDK view whose call_graph capability remains unsupported.
- [Phase 30-direct-call-facts]: Plan 30-08 recorded the verification-only regression task as an empty test commit to preserve the per-task commit contract.
- [Phase 31-p0-abstract-domain-kernel]: Keep abstract-domain contracts and P0 slots crate-private under analysis::domains with no public SDK, runner, CLI, README, or docs/facts promotion.
- [Phase 31-p0-abstract-domain-kernel]: Represent top and unknown causes as private TopReason labels that participate in stable digest parts.
- [Phase 31-p0-abstract-domain-kernel]: Use BTreeMap and BTreeSet ordering for deterministic product state and literal-set digest behavior.
- [Phase 31-p0-abstract-domain-kernel]: Keep solver, transfer, and result cursor APIs crate-private under analysis::domains with no SDK, runner, CLI, README, or docs/facts promotion.
- [Phase 31-p0-abstract-domain-kernel]: Materialize result identity and iteration through stable keys while using run-local IDs only for cursor lookup within a run.
- [Phase 31-p0-abstract-domain-kernel]: Treat calls, unsupported operations, dynamic writes, widening, and iteration budgets as explicit top/unknown events or states rather than silent certainty.
- [Phase 31-p0-abstract-domain-kernel]: Keep domain facts, provider, store, and cache identity crate-private with no SDK, runner, CLI, README, or docs/facts promotion.
- [Phase 31-p0-abstract-domain-kernel]: Normalize domain facts into observation rows and event rows with explicit status and precision labels, including top, unknown, setup, and budget cases.
- [Phase 31-p0-abstract-domain-kernel]: Make abstract-domain cache identity include provider policy, MIR, CFG, calls, symbol graph, module topology, syntax, lifecycle/config, and absent extension/model/toolchain slots.
- [Phase 31-p0-abstract-domain-kernel]: Represent domain bottom/no-info rows as explicit unknown top reasons before validation so malformed unknown rows fail closed.
- [Phase 31-p0-abstract-domain-kernel]: Record compact eval provider-output schema evidence for polint.abstract_domains without exposing a public provider surface.
- [Phase 31-p0-abstract-domain-kernel]: Abstract-domain facts remain internal eval/debug evidence, not SDK or CLI contract.
- [Phase 31-p0-abstract-domain-kernel]: Deterministic top and budget fixture rows use private test-only solver policies rather than changing production solver defaults.
- [Phase 31-p0-abstract-domain-kernel]: Transient domain place IDs are retained in stable keys but not exposed as invalid indexed references.
- [Phase 32-summary-kernel-and-direct-summaries]: Use max instead of saturating_add for CallEffects unresolved_count join to preserve lattice idempotence.
- [Phase 32-summary-kernel-and-direct-summaries]: Re-declare Changed enum locally in summaries::domain rather than importing from domains::lattice to keep module boundaries clean.
- [Phase 32-summary-kernel-and-direct-summaries]: Place AccessKind::join impl in core.rs since it is specific to summary domain join behavior.
- [Phase 32-summary-kernel-and-direct-summaries]: SummaryOutput normalized() sorts by (stable_key, id) then reassigns IDs sequentially, matching CallOutput pattern.
- [Phase 32-summary-kernel-and-direct-summaries]: Each SummaryDomainKind maps to a separate FactFamily variant for independent metadata tracking and removal.
- [Phase 32-summary-kernel-and-direct-summaries]: SummaryPrecision::Local and SetupAware both map to FactPrecision::SetupAware since summary facts are never Exact.
- [Phase 32-summary-kernel-and-direct-summaries]: Use polint.direct_summaries as the producer_id and layer_id for all summary metadata.
- [Phase 32-summary-kernel-and-direct-summaries]: Implement all four domain builders in a single DirectSummaryBuilder::build pass for deterministic output.
- [Phase 32-summary-kernel-and-direct-summaries]: TITO uses simple copy-chain tracing without field-level access paths per D-07/D-10.
- [Phase 32-summary-kernel-and-direct-summaries]: Memory effects treat all PlaceRoot::Parameter variants uniformly as Param(index) since the place model has no separate Receiver root.
- [Phase 32-summary-kernel-and-direct-summaries]: Output digest includes abstract_domains_output_digest as upstream input for cache invalidation when domain results change.
- [Phase 32-summary-kernel-and-direct-summaries]: Provider parameter digest includes all four summary domain IDs and versions for cache identity.
- [Phase 32-summary-kernel-and-direct-summaries]: LayerKind::DirectSummaries and direct_summaries_layer_key include absent extension/model/toolchain slots per D-14.
- [Phase 32-summary-kernel-and-direct-summaries]: Summary validation runs after validate_abstract_domains in the kernel validation sequence.
- [Phase 32-summary-kernel-and-direct-summaries]: Precision ceiling check rejects FactPrecision::Exact from polint.direct_summaries metadata rows.
- [Phase 32-summary-kernel-and-direct-summaries]: Summary debug rows use as_str labels for domain, status, precision, and provenance instead of dense IDs.
- [Phase 32-summary-kernel-and-direct-summaries]: Eval observation maps summary domain names to fact families: control_effects -> summary_control, call_effects -> summary_call, memory_effects -> summary_memory, data_flow_tito -> summary_tito.
- [Phase 32-summary-kernel-and-direct-summaries]: Summary event facts use a single summary_event family rather than per-domain event families.
- [Phase 32-summary-kernel-and-direct-summaries]: Direct-summary eval payload uses semicolon-delimited compact fragments: domain;status;precision;provenance;payload_digest_prefix.
- [Phase 32-summary-kernel-and-direct-summaries]: Direct-summary determinism comparison uses cold/warm/no-cache three-way equality matching the established direct-calls and abstract-domains patterns.
- [Phase 32-summary-kernel-and-direct-summaries]: Direct-summary public-boundary proof uses 21 specific internal markers (provider IDs, domain names, type names, fact families) rather than generic substring markers that would match test naming.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: EntrypointOutput normalized() sorts by stable_key then reassigns sequential IDs from 0, matching the CallOutput pattern.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: EntrypointStore validates referential integrity: trust boundaries and dispatch edges must reference existing entrypoint stable keys via from_output.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Four new FactFamily variants placed after ExtensionFact: Entrypoint, TrustBoundary, DispatchEdge, UnresolvedFramework.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: TriggerMetadata is a struct with optional fields (method, path, tool_name, event_name, test_name) rather than an enum.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: polint.entrypoints runs after polint.direct_summaries and SCC closure, before polint.extensions in the kernel run sequence.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Direct summaries provider output uses provider-computed digest via provider_output_for_with_optional_digest, not metadata fallback.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Upstream dependency digests are cloned before direct_summaries consumes them so entrypoints can reuse them.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: TS/JS test entrypoints use SetupAware precision (not ResolvedStatic) because they depend on test runner configuration being present.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: MCP SDK detection uses @modelcontextprotocol/ prefix matching to cover all possible subpath imports.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Trust boundaries are per-entrypoint per-source-kind facts derived from EntrypointKind rules per D-19/D-20/D-21.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: HTTP routes produce PathParam (if path has /:id or /{id}), QueryString, RequestBody (POST/PUT/PATCH/DELETE), RequestHeader boundaries.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Dispatch edges map EntrypointKind to DispatchEdgeKind following D-04 specification.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Unresolved merge uses BTreeMap by stable key for dedup (first occurrence wins) and deterministic sort.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Entrypoint fact accessors promoted from #[cfg(test)] to production visibility for validation pipeline access.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Extension framework facts use FrameworkPrecisionCeiling rejection reason separate from MissingProvenance for Exact precision violations.
- [Phase 35-framework-entrypoints-and-trust-boundaries]: Conflicting entrypoint registrations detected by same target_function with different framework_ids produce warning diagnostics.

## Execution Metrics

| Phase | Plan | Duration | Tasks | Files |
|-------|------|----------|-------|-------|
| 20-private-analysis-kernel-facade | 01 | 9 min | 2 | 5 |
| 20-private-analysis-kernel-facade | 02 | 9 min | 2 | 2 |
| 21-provenance-precision-and-validation-metadata | 01 | 9h 8m | 2 | 3 |
| 21-provenance-precision-and-validation-metadata | 02 | 14m | 3 | 6 |
| 21-provenance-precision-and-validation-metadata | 03 | 14m | 2 | 4 |
| 21-provenance-precision-and-validation-metadata | 04 | 11m | 2 | 3 |
| 22-internal-evaluation-harness-mvp | 02 | 15 min | 2 | 5 |
| 22-internal-evaluation-harness-mvp | 03 | 12 min | 3 | 7 |
| 22-internal-evaluation-harness-mvp | 04 | 11 min | 2 | 12 |
| 22-internal-evaluation-harness-mvp | 05 | 8 min | 1 | 9 |
| 22-internal-evaluation-harness-mvp | 06 | 9 min | 1 | 4 |
| 24-persistent-layer-cache-for-existing-cheap-facts | 01 | 13 min | 2 | 7 |
| 24-persistent-layer-cache-for-existing-cheap-facts | 02 | 20 min | 3 | 10 |
| 24-persistent-layer-cache-for-existing-cheap-facts | 03 | 16 min | 2 | 6 |
| 24-persistent-layer-cache-for-existing-cheap-facts | 04 | 19 min | 2 | 7 |
| 24-persistent-layer-cache-for-existing-cheap-facts | 05 | 28 min | 3 | 17 |
| 26-semantic-index-deepening | 01 | 12 min | 3 | 5 |
| 26-semantic-index-deepening | 02 | 19 min | 3 | 2 |
| 26-semantic-index-deepening | 03 | 70 min | 3 | 5 |
| 26-semantic-index-deepening | 04 | 23min | 3 | 6 |
| 26-semantic-index-deepening | 05 | 13 min | 3 | 4 |
| 26-semantic-index-deepening | 06 | 17 min | 3 | 14 |
| 27-layered-module-package-topology-graph | 01 | 12 min | 3 | 8 |
| 27-layered-module-package-topology-graph | 02 | 14 min | 3 | 5 |
| 27-layered-module-package-topology-graph | 03 | 16 min | 3 | 5 |
| 27-layered-module-package-topology-graph | 04 | 14 min | 3 | 6 |
| 27-layered-module-package-topology-graph | 05 | 23 min | 3 | 12 |
| 27-layered-module-package-topology-graph | 06 | 17 min | 2 | 21 |
| 27-layered-module-package-topology-graph | 07 | 5 min | 1 | 2 |
| 28-private-semantic-mir-and-place-identity | 01 | 19 min | 3 | 12 |
| 28-private-semantic-mir-and-place-identity | 02 | 12 min | 3 | 4 |
| 28-private-semantic-mir-and-place-identity | 03 | 14 min | 2 | 2 |
| 28-private-semantic-mir-and-place-identity | 04 | 17 min | 2 | 2 |
| 28-private-semantic-mir-and-place-identity | 05 | 26 min | 3 | 12 |
| 28-private-semantic-mir-and-place-identity | 06 | 12 min | 2 | 13 |
| 28-private-semantic-mir-and-place-identity | 07 | 11 min | 1 | 6 |
| 29-local-cfg-and-control-dependence | 01 | 18 min | 3 | 7 |
| 29-local-cfg-and-control-dependence | 02 | 24 min | 3 | 4 |
| 29-local-cfg-and-control-dependence | 03 | 34 min | 3 | 12 |
| 29-local-cfg-and-control-dependence | 04 | 28 min | 2 | 4 |
| 29-local-cfg-and-control-dependence | 05 | 31 min | 2 | 3 |
| 29-local-cfg-and-control-dependence | 06 | 68 min | 3 | 19 |
| 30-direct-call-facts | 02 | 8 min | 2 | 9 |
| 30-direct-call-facts | 03 | 12 min | 2 | 4 |
| 30-direct-call-facts | 04 | 17 min | 3 | 7 |
| 30-direct-call-facts | 05 | 14 min | 3 | 9 |
| 30-direct-call-facts | 06 | 5min | 1 | 3 |
| 30-direct-call-facts | 08 | 10 min | 3 | 2 |
| 31-p0-abstract-domain-kernel | 01 | 8 min | 3 | 5 |
| 31-p0-abstract-domain-kernel | 02 | 14 min | 3 | 5 |
| 31-p0-abstract-domain-kernel | 03 | 16 min | 2 | 13 |
| 31-p0-abstract-domain-kernel | 04 | 14 min | 2 | 9 |
| 31-p0-abstract-domain-kernel | 05 | 43 min | 3 | 19 |
| 32-summary-kernel-and-direct-summaries | 01 | 8 min | 2 | 6 |
| 32-summary-kernel-and-direct-summaries | 02 | 5 min | 2 | 4 |
| 32-summary-kernel-and-direct-summaries | 03 | 6 min | 2 | 2 |
| 32-summary-kernel-and-direct-summaries | 04 | 12 min | 2 | 10 |
| 32-summary-kernel-and-direct-summaries | 05 | 9 min | 2 | 4 |
| 32-summary-kernel-and-direct-summaries | 06 | 10 min | 2 | 11 |
| 32-summary-kernel-and-direct-summaries | 07 | 10 min | 3 | 1 |
| 35-framework-entrypoints-and-trust-boundaries | 01 | 5 min | 2 | 6 |
| 35-framework-entrypoints-and-trust-boundaries | 02 | 7 min | 2 | 8 |
| 35-framework-entrypoints-and-trust-boundaries | 03 | 5 min | 1 | 2 |
| 35-framework-entrypoints-and-trust-boundaries | 04 | 4 min | 1 | 2 |
| 35-framework-entrypoints-and-trust-boundaries | 05 | 6 min | 2 | 6 |
| 35-framework-entrypoints-and-trust-boundaries | 06 | 8 min | 2 | 6 |
| 35-framework-entrypoints-and-trust-boundaries | 07 | recorded | 2 | recorded |
| 35-framework-entrypoints-and-trust-boundaries | 08 | recorded | 1 | 2 |

## Session

- Last session: 2026-05-24
- Last activity: 2026-05-24 - Completed Phase 35 plan 8 of 8 and public no-leak proof.
- Stopped at: Phase 35 complete; ready to start Phase 36.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260524 | Fix PR 41 Ubuntu clippy failures | 2026-05-24 | implemented | [260524-fix-pr41-ubuntu-clippy](./quick/260524-fix-pr41-ubuntu-clippy/) |
| 260524 | Fix deep review entrypoint issues | 2026-05-24 | implemented | [260524-fix-deep-review-entrypoint-issues](./quick/260524-fix-deep-review-entrypoint-issues/) |
| 260522-n3q | Fix Phase 33 review findings with TDD tests | 2026-05-22 | implemented | [260522-n3q-fix-phase-33-review-findings-with-tdd-te](./quick/260522-n3q-fix-phase-33-review-findings-with-tdd-te/) |
| 260521-nem | Add realistic structured coverage for direct calls and abstract domains | 2026-05-21 | implemented | [260521-nem-add-realistic-structured-coverage-for-di](./quick/260521-nem-add-realistic-structured-coverage-for-di/) |
| 260521-m9k | Fix critical PR review findings for direct calls and abstract domains | 2026-05-21 | implemented | [260521-m9k-fix-critical-pr-review-findings-for-dire](./quick/260521-m9k-fix-critical-pr-review-findings-for-dire/) |
| 260521-b38 | Fix CFG digest payload and stable unsupported control-flow keys | 2026-05-21 | implemented | [260521-b38-fix-cfg-digest-payload-and-stable-unsupp](./quick/260521-b38-fix-cfg-digest-payload-and-stable-unsupp/) |
| 260521-af1 | Fix CFG stored reachability for synthetic exits | 2026-05-21 | implemented | [260521-af1-fix-cfg-stored-reachability-for-syntheti](./quick/260521-af1-fix-cfg-stored-reachability-for-syntheti/) |
| 260521-a5k | Fix CFG PR review findings | 2026-05-21 | implemented | [260521-a5k-fix-cfg-pr-review-findings](./quick/260521-a5k-fix-cfg-pr-review-findings/) |
| 260520-jho | Speed up CI with Rust caching and lighter PR platform checks, then measure Actions runtime | 2026-05-20 | implemented | [260520-jho-speed-up-ci-with-rust-caching-and-lighte](./quick/260520-jho-speed-up-ci-with-rust-caching-and-lighte/) |
| 260520-ii6 | Merge latest main security fixes into PR 33 branch and rerun all local checks | 2026-05-20 | implemented | [260520-ii6-merge-latest-main-security-fixes-into-pr](./quick/260520-ii6-merge-latest-main-security-fixes-into-pr/) |
| 260520-iba | Resolve PR 33 merge conflict against latest main and re-review merge readiness | 2026-05-20 | implemented | [260520-iba-resolve-pr-33-merge-conflict-against-lat](./quick/260520-iba-resolve-pr-33-merge-conflict-against-lat/) |
| 260520-h6j | Fix Phase 28 local MIR correctness issues and add edge-case tests | 2026-05-20 | implemented | [260520-h6j-fix-phase-28-local-mir-correctness-issue](./quick/260520-h6j-fix-phase-28-local-mir-correctness-issue/) |
| 260520-fpj | Fix remaining go.work repo-boundary issues and run another security review | 2026-05-20 | implemented | [260520-fpj-fix-remaining-go-work-repo-boundary-secu](./quick/260520-fpj-fix-remaining-go-work-repo-boundary-secu/) |
| 260520-da2 | Harden core trust boundaries, add regression tests, and run a secondary deep security review | 2026-05-20 | implemented | [260520-da2-harden-core-trust-boundaries-and-run-sec](./quick/260520-da2-harden-core-trust-boundaries-and-run-sec/) |
| 260520-c7k | Fix security findings around repo escape reads, workspace glob validation, Go package pattern validation, topology input size limits, and synthetic go.work creation | 2026-05-20 | implemented | [260520-c7k-fix-security-findings-around-repo-escape](./quick/260520-c7k-fix-security-findings-around-repo-escape/) |
| 260520-ai8 | Fix package-manager topology review findings with TDD tests and deep review | 2026-05-20 | implemented | [260520-ai8-fix-package-manager-topology-review-find](./quick/260520-ai8-fix-package-manager-topology-review-find/) |
| 260520-a6t | Fix pnpm workspace package-manager review findings | 2026-05-20 | implemented | [260520-a6t-fix-pnpm-workspace-package-manager-revie](./quick/260520-a6t-fix-pnpm-workspace-package-manager-revie/) |
| 260520-9jr | Fix package-manager topology review findings | 2026-05-20 | implemented | [260520-9jr-fix-package-manager-topology-review-find](./quick/260520-9jr-fix-package-manager-topology-review-find/) |
| 260519-vl1 | Full lockfile-based package manager support for TS/JS topology | 2026-05-19 | implemented | [260519-vl1-full-lockfile-based-package-manager-supp](./quick/260519-vl1-full-lockfile-based-package-manager-supp/) |
| 260519-qdf | Fix second Phase 27 topology review findings | 2026-05-19 | cbb635e | [260519-qdf-fix-second-phase-27-topology-review-find](./quick/260519-qdf-fix-second-phase-27-topology-review-find/) |
| 260519-ci | Fix attached Phase 26 CI failures for manifest version, cross-platform path validation, and layer-cache eval budget | 2026-05-19 | implemented | [260519-ci-fix-phase-26-ci-failures](./quick/260519-ci-fix-phase-26-ci-failures/) |
| 260519-fqg | Fix PR review findings for semantic index keys, validation, lint failures, and rerun deep review | 2026-05-19 | implemented | [260519-fqg-fix-pr-review-findings-for-semantic-inde](./quick/260519-fqg-fix-pr-review-findings-for-semantic-inde/) |
| 260518-qzd | Research and plan ai-friendly polint check output format | 2026-05-18 | implemented | [260518-qzd-research-and-plan-ai-friendly-polint-che](./quick/260518-qzd-research-and-plan-ai-friendly-polint-che/) |

## Next Action

Start Phase 36 (p0-type-value-place-alias-substrate) when ready.
