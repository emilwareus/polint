# Phase 24: Persistent Layer Cache for Existing Cheap Facts - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-18
**Phase:** 24-persistent-layer-cache-for-existing-cheap-facts
**Mode:** `--auto`
**Areas discussed:** Layer cache boundary, Invalidation and dependencies, Persistence and compatibility, Verification and observability

---

## Layer Cache Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Existing cheap provider layers only | Persist current parse/syntax, imports, module graph, symbol/reference, and metrics layers while keeping diagnostics/query/summary caches out of scope. | yes |
| Arbitrary query and diagnostic cache | Cache diagnostics and demand-query style outputs immediately. | no |
| Source files plus all future layers | Try to create the full future cache model in one phase. | no |

**User's choice:** Auto-selected the recommended conservative scope.
**Notes:** This matches the roadmap success criteria and the incremental-query research warning to start with layer caching, not diagnostic or arbitrary query caching.

---

## Invalidation And Dependencies

| Option | Description | Selected |
|--------|-------------|----------|
| Fail closed and recompute broadly | Reuse only when every key, manifest, schema, dependency, and digest validation matches. Recompute on uncertainty. | yes |
| Aggressively reuse when digests look equal | Prefer cache reuse even when dependency classification is incomplete. | no |
| Add a complex shape classifier first | Delay layer persistence until detailed syntax/import/public API shape classification exists. | no |

**User's choice:** Auto-selected fail-closed invalidation.
**Notes:** The phase can still introduce dependency indexes and basic change classification, but stale reuse must fail safely.

---

## Persistence And Compatibility

| Option | Description | Selected |
|--------|-------------|----------|
| Incremental compatibility path | Build on `CacheLayout`, Phase 23 typed keys, and current file-fact cache behavior while adding layer manifests/payloads incrementally. | yes |
| Replace current cache immediately | Remove the current file-fact cache and migrate everything to the new layer cache in one step. | no |
| Add a separate experimental cache root | Create a disconnected cache implementation that does not integrate with existing cache status and layout. | no |

**User's choice:** Auto-selected incremental compatibility.
**Notes:** This keeps blast radius controlled and lets syntax providers bridge from current cache stats while derived layers gain explicit manifests.

---

## Verification And Observability

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic stale-reuse and stats proof | Add unit, integration, and native eval proof for cold/warm hits, rule-edit non-invalidation, import/config/lifecycle invalidation, stale cache fallback, stats, and public no-leak behavior. | yes |
| Basic hit/miss smoke tests | Only prove that one cache entry can be read back. | no |
| Manual validation only | Rely on local runs without pinned fixtures or regression tests. | no |

**User's choice:** Auto-selected deterministic proof.
**Notes:** Phase 23 already established native eval and no-leak patterns; Phase 24 should extend them for real layer-cache behavior.

---

## the agent's Discretion

- Exact type names and file layout.
- Whether syntax layer cache first wraps the existing file-fact cache or writes a new layer payload immediately.
- The plan split across syntax, derived layers, dependency indexes, invalidation, and verification.
- How precise initial change classification should be, as long as uncertain cases recompute.

## Deferred Ideas

- Public rule manifest, inspect, and test loop - Phase 25.
- Demand-query, summary, extension, and public query/cache surfaces - later v1.2 phases.
