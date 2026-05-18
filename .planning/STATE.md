---
gsd_state_version: 1.0
milestone: v1.2
milestone_name: Static Analysis Engine Implementation
status: executing
last_updated: "2026-05-18T06:00:35.344Z"
last_activity: 2026-05-18 -- Phase 23 planning complete
progress:
  total_phases: 22
  completed_phases: 3
  total_plans: 17
  completed_plans: 12
  percent: 71
---

# State: polint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-16)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** Phase 23 — input-snapshots-and-cache-key-vocabulary

## Current Status

- **GitHub:** `emilwareus/polint` (public repository name).
- Active branch policy: do not push directly to `main` unless explicitly instructed; create a feature/fix branch before sharing remote work.
- v1.0 MVP was audited, archived, tagged, and closed on 2026-05-02.
- v1.1 Capability Fulfillment completed the capability plan, resolved imports/module graph, and symbols/references foundations.
- Static-analysis engine research completed on 2026-05-16 in `research/ROADMAP.md`.
- v1.2 requirements are defined in `.planning/REQUIREMENTS.md`.
- v1.2 roadmap is defined in `.planning/ROADMAP.md`.
- Phase 22 has been shipped for review in PR #22: https://github.com/emilwareus/polint/pull/22.
- Each v1.2 research PR maps to one GSD phase, in order, from Phase 20 through Phase 41.
- New broad research is not needed by default. Use the relevant research documents referenced by each phase; do additional research only for a concrete implementation gap.

## Current Position

Milestone: v1.2 Static Analysis Engine Implementation
Status: Ready to execute
Phase: 23 (input-snapshots-and-cache-key-vocabulary) — READY FOR PLANNING
Plan: Not started
Last activity: 2026-05-18 -- Phase 23 planning complete

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 20 | Complete | 2/2 plans complete; private kernel facade/delegation plus internal provider manifests/order inspection done |
| 21 | Complete | 4/4 plans complete; provenance, precision, validation metadata, deterministic debug JSON, and public compatibility proof done; requirement SAE-FND-02 |
| 22 | Complete | 6/6 plans complete; evaluation model/report hashing, generic matchers/metrics, native fixture runner, provenance/cache/extension fixtures, fixture category coverage, and public-boundary proof done; requirement SAE-FND-03 |
| 23 | Pending | Input snapshots and cache-key vocabulary; requirement SAE-FND-04 |
| 24 | Pending | Persistent layer cache for existing cheap facts; requirement SAE-FND-05 |
| 25 | Pending | Rule manifest, inspect, and test skeleton; requirement SAE-FND-06 |
| 26 | Pending | Semantic index deepening; requirement SAE-SEM-01 |
| 27 | Pending | Layered module/package/topology graph; requirement SAE-SEM-02 |
| 28 | Pending | Private semantic MIR and place identity; requirement SAE-SEM-03 |
| 29 | Pending | Local CFG and control dependence; requirement SAE-SEM-04 |
| 30 | Pending | Direct call facts; requirement SAE-SEM-05 |
| 31 | Pending | P0 abstract-domain kernel; requirement SAE-INT-01 |
| 32 | Pending | Summary kernel and direct summaries; requirement SAE-INT-02 |
| 33 | Pending | Demand queries and summary SCC cache; requirement SAE-INT-03 |
| 34 | Pending | Rust extension/provider sink; requirement SAE-INT-04 |
| 35 | Pending | Framework entrypoints and trust boundaries; requirement SAE-INT-05 |
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

## Session

- Last session: 2026-05-17
- Stopped at: Completed 22-06-PLAN.md

## Next Action

Phase 22 is ready for verification:

`/gsd-verify-work 22`
