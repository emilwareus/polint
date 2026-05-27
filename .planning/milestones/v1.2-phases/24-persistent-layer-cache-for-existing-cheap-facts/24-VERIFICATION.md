---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
verified: 2026-05-18T12:43:49Z
status: passed
score: "8/8 must-haves verified"
overrides_applied: 0
---

# Phase 24: Persistent Layer Cache for Existing Cheap Facts Verification Report

**Phase Goal:** Persist parse/syntax, imports, module facts, symbols/references, and metrics layers with conservative invalidation.
**Verified:** 2026-05-18T12:43:49Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Existing cheap fact layers persist through the layer cache. | VERIFIED | Go and TS syntax providers read/write `LayerCacheStore` payloads in `go/adapter.rs:101` and `ts/adapter.rs:111`; module, symbol, and metrics providers do the same in `module_graph/mod.rs:276`, `symbol_graph/mod.rs:124`, and `metrics.rs:57`. Imports are part of persisted `CachedFileFacts` in `core/mod.rs:527`. |
| 2 | Syntax cache is not invalidated by unrelated rule edits. | VERIFIED | `LayerKey::syntax_layer_key` takes parser/source/config/lifecycle/toolchain inputs and no rule digest in `keys.rs:160`; `syntax_cache_ignores_unrelated_rule_edits` asserts identical manifests after a rule edit in `cli.rs:285`. Spot-check passed. |
| 3 | Module and symbol layers invalidate on import, lifecycle, and config changes. | VERIFIED | Module keys include import, lifecycle, config, and upstream syntax digests in `keys.rs:203`; symbol keys include source/function/package/import/lifecycle/config/module/syntax digests in `keys.rs:255`. Regression tests cover these changes in `keys.rs:550` and `keys.rs:637`; eval import-edit fixture expects module and symbol misses. |
| 4 | Metrics layer invalidates conservatively on source/function/config/syntax inputs. | VERIFIED | Metrics keys include source, function, config, and upstream syntax digests in `keys.rs:315`; dependency edges mirror those inputs in `metrics.rs:202`; key tests cover source/function/config/syntax changes in `keys.rs:778`. |
| 5 | Stale, corrupt, mismatched, unsupported, deserialization-failing, or path-invalid cache entries fail closed. | VERIFIED | `LayerCacheStore::read_json_validated` returns miss/invalid/bypass instead of panicking in `layer_cache.rs:127`; manifest, digest, validation, dependency, symlink, and managed-root checks are in `layer_cache.rs:296` and `layer_cache.rs:312`. Stale/path tests cover these cases in `layer_cache.rs:679`. |
| 6 | Dependency index, change sets, and invalidation vocabulary exist and classify unknown changes conservatively. | VERIFIED | `DependencyIndex` stores sorted forward/reverse edges in `dependency_index.rs:77`; `InvalidationPlan::from_change_set` drops mismatched schemas and recomputes/quarantines/drop changes in `invalidation.rs:73` and `invalidation.rs:208`. |
| 7 | Cache stats report deterministic hits, misses, writes, disabled bypasses, invalid reads, and verified reuse. | VERIFIED | Counters exist in `stats.rs:7`; `KernelRunReport::new` aggregates them in `run_report.rs:11`; eval emits `layer_cache.provider.*` and aggregate invariants in `observed.rs:295`; fixture expectations cover cold/warm/disabled/import-edit counters in `expected.polint-eval.toml:11`. |
| 8 | Layer cache internals stay private and public behavior remains compatible. | VERIFIED | Public JSON and source-surface no-leak checks are in `cli.rs:471` and `cli.rs:617`; the external rule fixture imports only `polint::sdk::prelude::*` and uses `polint::runner::run_cli` in `cli.rs:561`. Spot-check passed. |

**Score:** 8/8 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` | Safe manifest/blob store | VERIFIED | Manifest schema, manifest-last writes, digest validation, disabled bypasses, and stale/path tests exist. |
| `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` | Dependency index | VERIFIED | Versioned schema with deterministic `BTreeMap` forward/reverse indexes. |
| `crates/polint/src/analysis_kernel/incremental/change_set.rs` and `invalidation.rs` | Change-set and fail-closed invalidation vocabulary | VERIFIED | Change kinds and recompute/drop/quarantine planning exist. |
| `crates/polint/src/analysis_kernel/incremental/keys.rs` | Layer key constructors | VERIFIED | Syntax, module graph, symbol graph, and metrics key constructors include conservative inputs. |
| `crates/polint/src/go/adapter.rs` and `ts/adapter.rs` | Syntax layer persistence | VERIFIED | Both adapters read/write normalized syntax payloads through `LayerCacheStore`. |
| `crates/polint/src/module_graph/mod.rs`, `symbol_graph/mod.rs`, `metrics.rs` | Derived layer persistence | VERIFIED | Providers read/write cached payloads, restore facts into `AnalysisDb`, and report stats. GSD pattern checks missed literal `LayerKind::*` strings in some files, but manual wiring verifies the functionality through key helpers. |
| `crates/polint/src/eval/observed.rs`, `eval/fixtures.rs`, `tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml` | Real-provider eval proof | VERIFIED | Cold/warm/disabled/import-edit fixture covers all Phase 24 providers. |
| `crates/polint/tests/cli.rs` | Public no-leak and compatibility proof | VERIFIED | Public JSON, CLI help, SDK, runner, and crate-root markers are checked. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `CacheLayout` | `LayerCacheStore` | `layer_cache_dir()` passed to provider stores | VERIFIED | `CacheLayout::layer_cache_dir` returns `.polint/cache/layers`; providers call `LayerCacheStore::new(cache.layer_cache_dir(), cache.is_enabled())`. |
| Kernel syntax providers | Module/symbol/metrics layers | Provider output digests | VERIFIED | `analysis_kernel/mod.rs:108` threads Go/TS output digests into module, symbol, and metrics derivations. |
| Module/symbol/metrics providers | Dependency index | Manifest dependency edges | VERIFIED | Derived providers build source/import/config/lifecycle/upstream dependency edges before writes. |
| Kernel run report | Eval fixture | `provider_outputs` and aggregate stats | VERIFIED | Eval observations come from `KernelRunReport.provider_outputs` and `KernelRunReport.cache_stats`. |
| Layer cache internals | Public CLI/SDK surfaces | Negative marker checks | VERIFIED | Public no-leak integration test passed. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| Go/TS syntax adapters | `SyntaxLayerPayload.files[].facts` | Real parser analysis through existing file-fact paths, restored with `restore_file_facts` | Yes | FLOWING |
| Module graph | `ModuleGraphLayerPayload.resolved_imports/nodes/edges` | `derive_requested_module_graph_uncached` builds facts from `AnalysisDb` imports/packages/files | Yes | FLOWING |
| Symbol graph | `SymbolGraphLayerPayload.symbols/definitions/references` | Go/TS symbol derivation and `AnalysisDb::replace_symbol_graph_facts` | Yes | FLOWING |
| Metrics | `MetricsLayerPayload.file/function/complexity_metrics` | Metrics derived from real `AnalysisDb` files and functions | Yes | FLOWING |
| Eval fixture | `layer_cache.*` invariants | Real `AnalysisKernel::run` output via `KernelRunReport` | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Real-provider layer-cache eval passes | `cargo test -p polint --lib eval_layer_cache_fixture_passes --locked` | 1 passed | PASS |
| Stale/path/cache provider tests pass | `cargo test -p polint --lib layer_cache --locked` | 34 passed | PASS |
| Public layer-cache internals stay private | `cargo test -p polint --test cli layer_cache_internals_stay_private --locked` | 1 passed | PASS |
| Unrelated rule edits do not invalidate syntax cache | `cargo test -p polint --test cli syntax_cache_ignores_unrelated_rule_edits --locked` | 1 passed | PASS |
| Layer-key invalidation tests pass | `cargo test -p polint --lib layer_key --locked` | 9 passed | PASS |
| Full workspace regression | Orchestrator gate: `cargo test --workspace --all-features --locked` | Reported passed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SAE-FND-05 | 24-01 through 24-05 | Existing cheap fact layers persist through a conservative layer cache with dependency indexes, change sets, hit/miss reporting, and stale-reuse safeguards. | SATISFIED | Persistent syntax/import/module/symbol/reference/metrics layer paths exist, dependency/index/invalidation vocabulary exists, hit/miss stats are reported internally, and stale reuse tests pass. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/polint/src/metrics.rs` | 381 | Metrics layer cache write failures are silently suppressed. | Warning | Non-blocking for Phase 24 goal. A metrics cache write failure degrades to recompute/no persistence for that write, not stale reuse; normal persistence, invalidation, stats, and stale-safety are verified. This matches WR-01 in `24-REVIEW.md` and should be fixed as follow-up. |

### Human Verification Required

None.

### Gaps Summary

No blocking gaps found. Phase 24 achieves the roadmap goal and SAE-FND-05. The one review warning remains a quality/observability follow-up, not a goal-achievement blocker.

---

_Verified: 2026-05-18T12:43:49Z_
_Verifier: Claude (gsd-verifier)_
