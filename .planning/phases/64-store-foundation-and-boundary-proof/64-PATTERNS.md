# Phase 64 Pattern Map

**Mapped:** 2026-07-10
**Purpose:** Concrete existing analogs for the private semantic-store foundation.

## Files and Roles

| Planned area | Closest existing analog | Pattern to preserve |
|---|---|---|
| Workspace dependency | `Cargo.toml`, `crates/polint/Cargo.toml` | Workspace-pinned dependency plus `*.workspace = true`; keep `unsafe_code` and `unreachable_pub` lint policy unchanged. |
| Cache-owned store path | `crates/polint/src/cache/mod.rs` | `CacheLayout` owns `.polint/cache`; `POLINT_CACHE_DIR` remains the override; disabled caches return before filesystem access. |
| Safe local filesystem ownership | `crates/polint/src/repo_fs.rs` | Reject symlink ancestors and path escape; create only verified directories with `ensure_repo_dir` / `create_dir_all_no_symlink`. |
| Kernel-private module | `crates/polint/src/analysis_kernel/mod.rs` | Private module declaration, curated `pub(crate)` vocabulary, no crate-root or SDK re-export. |
| Private run telemetry | `crates/polint/src/analysis_kernel/incremental/run_report.rs` | Typed, deterministic fields on `KernelRunReport`; test accessors stay crate-private/test-only. |
| Controlled cache outcomes | `crates/polint/src/cache/mod.rs`, `analysis_kernel/incremental/layer_cache.rs` | Small status enums (`Disabled`, hit/miss/invalid) instead of panics or untyped strings; invalid owned cache state becomes a controlled outcome. |
| Public boundary gate | `crates/polint/tests/public_surface_leak.rs`, `crates/polint/src/analysis_kernel/mod.rs` no-leak tests | Compile an outside-consumer probe, parse allowlisted SDK exports, scan public output/docs for forbidden internal markers. |
| Phase boundary benchmark | `crates/polint/src/eval/bench/runner.rs`, `gate.rs` | Isolated child-process measurement, deterministic diagnostics digest, `evaluate_regression_budget`, and `is_blocking`. |

## Concrete Interfaces

### Cache ownership

`Cache::default_for_repo(repo, enabled)` derives its root from `CacheLayout::for_repo`; `CacheLayout` currently exposes `analysis_dir`, `derived_dir`, and `layer_cache_dir`. Add a sibling semantic-store directory/path there rather than inventing another root. Preserve the existing rule that `enabled == false` returns before creating `.polint/cache`.

### Kernel integration

`AnalysisKernel::run` finishes provider work, calls `validation::validate_fact_metadata`, finalizes metadata, and constructs `KernelRunReport` immediately before returning `KernelOutput`. The Phase 64 store hook belongs after validation/finalization and before run-report construction. It must consume only typed cache/store configuration and return private status; providers and rules remain unaware of it.

### Failure vocabulary

Existing cache code maps disabled, miss, invalid/evicted, and write outcomes into enums. The store should mirror this approach with typed outcomes for disabled, ready, busy/skipped, future schema, rebuild-needed/invalid, and corrupt state. `thiserror` is already available for lower-level internal errors; production code should not `unwrap` or `expect`.

### Boundary proof

`public_surface_leak.rs` is the source of truth for deliberate `polint::sdk::prelude` exports and compiles `tests/fixtures/public-surface-leak-probe` as an outside consumer. The store must add no allowlist entries. Internal no-leak scans should cover `sdk`, `runner`, `cli`, `lib.rs`, README, `docs/facts`, examples, and generated skill text for `rusqlite`, store module paths, SQL/table vocabulary, and raw database IDs.

### Performance proof

`run_repo_perf_point_isolated` is the required comparable measurement path because peak RSS is process-global. Extend its internal child harness with an explicit semantic-store-enabled mode, populate `CurvePoint.size.store_bytes`, compute a check-scoped diagnostics digest, and feed both into `evaluate_regression_budget` against `store-disabled-check.json`.

## Planning Constraints

- Do not add a public CLI/config/environment contract for store activation.
- Do not add manifest, generation, fact, summary, graph, query, cache-status, or search schemas in Phase 64.
- Keep rusqlite types and SQL text inside `analysis_kernel/store/`.
- Use precise typed errors/outcomes and borrowed `&Path` / `&str` inputs.
- Verify with focused tests first, then `cargo fmt`, full workspace clippy/lint, and the external leak probe.
