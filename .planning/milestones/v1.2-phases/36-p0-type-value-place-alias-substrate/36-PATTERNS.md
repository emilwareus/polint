# Phase 36 Pattern Map

**Generated:** 2026-05-24
**Source:** Inline GSD plan-phase pattern mapping

## Closest Existing Analogs

| Planned Area | Closest Existing Analog | Why It Matters |
|---|---|---|
| Type/value/alias fact contracts | `crates/polint/src/analysis/entrypoints/facts.rs`, `crates/polint/src/analysis/domains/facts.rs`, `crates/polint/src/analysis/calls/facts.rs` | Fact rows are crate-private structs with dense IDs, stable keys, status/precision enums, serde derives, and no SDK exposure. |
| Stores and normalized output | `crates/polint/src/analysis/entrypoints/store.rs`, `crates/polint/src/analysis/calls/store.rs`, `crates/polint/src/analysis/domains/store.rs` | Stores normalize by stable key, reassign dense IDs, validate references, and build BTreeMap indexes. |
| Provider wiring | `crates/polint/src/analysis/domains/provider.rs`, `crates/polint/src/analysis/summaries/provider.rs`, `crates/polint/src/analysis/entrypoints/provider.rs` | Providers compute normalized output, output digests, cache stats, update `AnalysisDb`, and return diagnostics. |
| Provider manifests | `crates/polint/src/analysis_kernel/provider.rs` | New providers declare IDs, inputs, outputs, schema versions, language scope, and precision ceiling in one deterministic list. |
| Cache key vocabulary | `crates/polint/src/analysis/domains/cache_key.rs`, `crates/polint/src/analysis/summaries/cache_key.rs`, `crates/polint/src/analysis_kernel/incremental/keys.rs` | Cache identity includes provider/schema parameters, config, lifecycle, upstream output digests, tool/model/extension sentinels. |
| MIR/place seed facts | `crates/polint/src/analysis/places.rs`, `crates/polint/src/analysis/mir/op.rs`, `crates/polint/src/analysis/mir/lower_go.rs`, `crates/polint/src/analysis/mir/lower_ts.rs` | Phase 36 should extend existing `PlaceFact`/`MirValue` identity rather than inventing parallel place identity. |
| Extension merge boundary | `crates/polint/src/analysis/extensions/sinks.rs`, `crates/polint/src/analysis/extensions/provider.rs`, `crates/polint/src/analysis_kernel/incremental/quarantine.rs` | Extension facts are normalized, precision-ceiling gated, validated, and quarantine-aware. |
| Eval/no-leak proof | `crates/polint/src/eval/`, `tests/eval-fixtures/abstract-domains/core`, `tests/eval-fixtures/framework-entrypoints/mixed-go-ts`, no-leak tests in `analysis_kernel/mod.rs` | Phase 36 needs deterministic internal eval fixtures and public boundary assertions. |

## Reusable Patterns

- Keep new modules under `crates/polint/src/analysis/` with `pub(crate)` visibility and register them through `analysis/mod.rs`, not `lib.rs` or SDK modules.
- Dense ID newtypes live in `analysis/ids.rs` and use the existing derive set.
- Stable keys should be built from sorted, labeled parts using existing stable key helpers, with run-local dense IDs assigned after normalization.
- Provider outputs should follow the established pattern: build facts, normalize, compute digest from upstream digests plus output rows, record recompute stats, replace facts in `AnalysisDb`, validate, and expose debug/eval through test-facing helpers only.
- Unknown, unsupported, setup-missing, and budget-exceeded conditions should be fact rows or events, not silent absence.
- Public no-leak tests should scan normal CLI JSON/help, SDK, runner, README, docs, and crate-root exports for internal markers.

## Files Likely To Change

- `crates/polint/src/analysis/ids.rs`
- `crates/polint/src/analysis/mod.rs`
- `crates/polint/src/analysis/types/`
- `crates/polint/src/analysis/values/`
- `crates/polint/src/analysis/access_paths/`
- `crates/polint/src/analysis/points_to/`
- `crates/polint/src/analysis/aliases/`
- `crates/polint/src/analysis/provider.rs`
- `crates/polint/src/analysis_kernel/provider.rs`
- `crates/polint/src/analysis_kernel/mod.rs`
- `crates/polint/src/analysis_kernel/metadata.rs`
- `crates/polint/src/analysis_kernel/validation.rs`
- `crates/polint/src/analysis_kernel/incremental/keys.rs`
- `crates/polint/src/core/mod.rs`
- `crates/polint/src/eval/`
- `tests/eval-fixtures/type-value-alias/`

## Cautions

- Do not make whole-repo points-to mandatory for baseline checks.
- Do not expose raw tool output or new SDK views in Phase 36.
- Do not replace existing `PlaceFact` identities used by MIR, calls, summaries, domains, and entrypoints.
- Do not let extension facts delete native facts or claim exact precision without validation evidence.
