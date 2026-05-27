---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 05
subsystem: analysis-entrypoints
tags: [extraction-pipeline, trust-boundaries, dispatch-edges, unresolved-merge, provider-wiring]
dependency_graph:
  requires: [entrypoint-facts, entrypoint-store, entrypoints-provider-kernel-wiring, go-framework-recognizers, ts-js-framework-recognizers]
  provides: [entrypoints-extraction-pipeline, trust-boundary-derivation, dispatch-edge-derivation, unresolved-merge, populated-entrypoints-provider]
  affects: [entrypoints-provider, eval-fixtures, validation]
tech_stack:
  added: []
  patterns: [extraction-orchestrator, per-entrypoint-per-source-kind-derivation, edge-kind-mapping, stable-key-dedup-merge]
key_files:
  created:
    - crates/polint/src/analysis/entrypoints/extract.rs
    - crates/polint/src/analysis/entrypoints/trust_boundaries.rs
    - crates/polint/src/analysis/entrypoints/dispatch.rs
    - crates/polint/src/analysis/entrypoints/unresolved.rs
  modified:
    - crates/polint/src/analysis/entrypoints/mod.rs
    - crates/polint/src/analysis/entrypoints/provider.rs
decisions:
  - Trust boundary source kinds follow per-entrypoint-kind rules from D-19/D-20/D-21
  - HTTP routes produce PathParam (if path has params), QueryString, RequestBody (for mutation methods), RequestHeader
  - HTTP middleware produces RequestBody, RequestHeader, QueryString
  - MCP tools and prompts produce McpArguments; MCP resources produce McpResourceUri
  - CLI commands produce CliArgs and CliFlags; test entrypoints produce no boundaries
  - Job/QueueConsumer produce QueuePayload; others produce Unknown per D-20
  - Dispatch edge kinds map from EntrypointKind following D-04 specification
  - Unresolved merge uses BTreeMap by stable key for dedup and deterministic sort
metrics:
  duration: 6 min
  completed: 2026-05-24
---

# Phase 35 Plan 05: Extraction Pipeline, Trust Boundaries, Dispatch Edges, and Provider Wiring Summary

Extraction orchestrator wiring Go and TS/JS recognizers into provider, per-entrypoint per-source-kind trust boundary derivation, framework dispatch edge derivation, and unresolved fact merge producing populated EntrypointOutput with deterministic output digest.

## What Was Done

### Task 1: Create extraction orchestrator, trust boundary derivation, dispatch edge derivation, and unresolved merge
- Created `extract.rs` with `extract_entrypoints(db)` orchestrating Go and TS/JS recognizers, deriving trust boundaries, dispatch edges, and merging unresolved facts
- Created `trust_boundaries.rs` with `derive_trust_boundaries(db, entrypoints)` producing per-entrypoint per-source-kind TrustBoundaryFact rows:
  - HTTP routes: PathParam (if path has /:id or /{id}), QueryString (always), RequestBody (POST/PUT/PATCH/DELETE), RequestHeader (always)
  - HTTP middleware: RequestBody, RequestHeader, QueryString
  - MCP tool/prompt: McpArguments; MCP resource: McpResourceUri
  - CLI command: CliArgs, CliFlags
  - Test: no trust boundaries (not external-facing)
  - Job/QueueConsumer: QueuePayload
  - Others (ServerlessHandler, LifecycleCallback, EventListener, GeneratedDispatch): Unknown per D-20
- Created `dispatch.rs` with `derive_dispatch_edges(db, entrypoints)` producing FrameworkDispatchEdgeFact for each entrypoint:
  - HttpRoute -> RouteDispatch, HttpMiddleware -> MiddlewareChain
  - McpTool/McpResource/McpPrompt -> McpDispatch, Test -> TestRunner
  - CliCommand -> RouteDispatch, Job/QueueConsumer -> JobScheduler
  - LifecycleCallback -> LifecycleHook, EventListener -> EventDispatch
  - ServerlessHandler/GeneratedDispatch -> RouteDispatch
- Created `unresolved.rs` with `merge_unresolved(go, ts)` combining and deduplicating by stable key (first occurrence wins), sorted by BTreeMap key order
- Updated `mod.rs` to declare extract, trust_boundaries, dispatch, unresolved modules
- Trust boundary precision follows entrypoint precision per D-21
- All stable keys use semantic_stable_key with appropriate FactFamily variants
- 18 trust boundary tests, 10 dispatch edge tests, 5 unresolved merge tests, 1 extract test

### Task 2: Wire extraction into provider to produce populated output
- Replaced `EntrypointOutput::empty()` with `extract_entrypoints(db)` call in `derive_entrypoints_with_cache_stats`
- Provider now runs Go and TS/JS recognizers, derives trust boundaries and dispatch edges, merges unresolved, normalizes, computes output digest, and stores via `db.replace_entrypoint_facts`
- Added 3 new provider tests:
  - `populated_output_produces_non_absent_digest`: verifies populated output produces a non-empty digest
  - `output_digest_changes_when_entrypoints_added`: verifies digest changes with different entrypoint content
  - `output_digest_is_deterministic_for_same_input`: verifies identical inputs produce identical digests

## Verification Results

- `cargo test -p polint --lib analysis::entrypoints` -- 80 passed
- `cargo check -p polint` -- succeeds with no warnings

## Deviations from Plan

None - plan executed exactly as written.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 636b546 | feat(35-05): create extraction orchestrator, trust boundary derivation, dispatch edges, and unresolved merge |
| 2 | 23008fb | feat(35-05): wire extraction into provider to produce populated output |

## Self-Check: PASSED
