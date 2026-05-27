---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
reviewed: 2026-05-18T12:32:54Z
depth: standard
files_reviewed: 29
files_reviewed_list:
  - crates/polint/src/analysis_kernel/incremental/change_set.rs
  - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
  - crates/polint/src/analysis_kernel/incremental/digest.rs
  - crates/polint/src/analysis_kernel/incremental/invalidation.rs
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
  - crates/polint/src/analysis_kernel/incremental/stats.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/cache/mod.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/go/adapter.rs
  - crates/polint/src/go/tests.rs
  - crates/polint/src/metrics.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/module_graph/model.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/symbol_graph/model.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/src/ts/tests.rs
  - crates/polint/tests/cli.rs
  - tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml
  - tests/eval-fixtures/cache/layer-cache/repo/.polint.toml
  - tests/eval-fixtures/cache/layer-cache/repo/goapp/go.mod
  - tests/eval-fixtures/cache/layer-cache/repo/goapp/payment.go
  - tests/eval-fixtures/cache/layer-cache/repo/web/package.json
  - tests/eval-fixtures/cache/layer-cache/repo/web/src/app.ts
  - tests/eval-fixtures/cache/layer-cache/repo/web/tsconfig.json
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 24: Code Review Report

**Reviewed:** 2026-05-18T12:32:54Z
**Depth:** standard
**Files Reviewed:** 29
**Status:** issues_found

## Summary

Reviewed the incremental cache primitives, layer-cache store, Go/TS syntax adapters, module/symbol/metrics derived layers, cache CLI coverage, and the new layer-cache eval fixture. The layer keys and validation generally include source, config, lifecycle, dependency, and payload checks, and public cache internals remain scoped to the intended internal/bench surfaces.

One warning remains: metrics layer cache write failures are swallowed instead of producing the same controlled internal cache diagnostic that the syntax, module graph, and symbol graph layers emit.

## Warnings

### WR-01: Metrics layer cache write failures are silently suppressed

**File:** `crates/polint/src/metrics.rs:381`

**Issue:** `write_metrics_layer_payload` returns `None` when payload digesting fails and ignores `store.write_json` errors at line 398. Unlike the Go/TS syntax, module graph, and symbol graph cache paths, this hides cache serialization, permission, path-safety, and disk errors from the run diagnostics. A metrics layer cache failure can therefore degrade to uncached behavior with no user-visible or test-visible signal.

**Fix:** Give `MetricsDerivation` a diagnostics vector, pass it into `write_metrics_layer_payload`, emit `internal/cache` warnings on serialization and write errors, and extend `diagnostics` in `AnalysisKernel::run` before building the report.

```rust
fn write_metrics_layer_payload(
    store: &LayerCacheStore,
    layer_key: LayerKey,
    payload: &MetricsLayerPayload,
    dependencies: Vec<DependencyEdge>,
    stats: &mut CacheStats,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Digest> {
    let payload_digest = match LayerCacheStore::payload_digest_for_json(payload) {
        Ok(digest) => digest,
        Err(error) => {
            diagnostics.push(cache_write_diagnostic("metrics layer", error));
            return None;
        }
    };

    let output_digest = metrics_output_digest(&layer_key, &payload_digest);
    let manifest = LayerCacheManifest::new(
        layer_key,
        output_digest.clone(),
        payload_digest,
        dependencies,
        PrecisionTier::Syntax,
        "native_trusted",
        Vec::new(),
    );

    match store.write_json(&manifest, payload) {
        Ok(LayerCacheWriteStatus::Written) => stats.record_write(),
        Ok(LayerCacheWriteStatus::BypassedDisabled) => stats.record_disabled_bypass(),
        Err(error) => diagnostics.push(cache_write_diagnostic("metrics layer", error)),
    }

    Some(output_digest)
}
```

Add a focused regression test that points the cache at an unwritable or file-backed layer-cache path and asserts the metrics provider emits an `internal/cache` diagnostic, matching the other cached providers.

---

_Reviewed: 2026-05-18T12:32:54Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
