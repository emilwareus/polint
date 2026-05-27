# Phase 38 Pattern Map: Local Plus Summary-Projected Data Flow

## Closest Existing Analogs

| New Area | Closest Analog | Why It Matters |
|---|---|---|
| Data-flow fact contracts and store | `crates/polint/src/analysis/refined_calls/{facts,store}.rs` | New private fact family with normalized output, dense IDs, stable keys, and indexed store. |
| Provider/cache/kernel wiring | `crates/polint/src/analysis/refined_calls/{cache_key,provider}.rs`, `crates/polint/src/analysis_kernel/provider.rs` | Phase 38 should follow the post-type-value provider pattern and add `polint.data_flow` before metrics. |
| Local MIR/CFG-derived extraction | `crates/polint/src/analysis/calls/extract.rs`, `crates/polint/src/analysis/domains/provider.rs` | Build rows from semantic MIR/places/CFG without reparsing source or leaking parser objects. |
| Summary projection | `crates/polint/src/analysis/summaries/{facts,store,provider}.rs` | `SummaryDomainKind::DataFlowTito`, `FlowKind`, and `FlowRoot` already reserve flow vocabulary. |
| Model/extension integration | `crates/polint/src/analysis/extensions/{sinks,store,validate}.rs` | Repo-local model facts must stay additive, validated, precision-ceiling gated, and quarantine-aware. |
| Debug/eval/no-leak proof | `crates/polint/src/analysis/refined_calls/debug.rs`, `crates/polint/src/eval/fixtures.rs` | Add observed data-flow rows and taxonomy tests in the native fixture harness. |

## Reusable Patterns

- Add crate-private modules under `analysis/` and keep public `sdk`, `runner`, CLI, README, and `docs/facts` unchanged until promotion.
- Use stable keys as persistent identity and reassign dense IDs after deterministic sorting.
- Store rebuildable indexes in the fact store; facts remain the source of truth.
- Provider output digests include provider/schema, config/lifecycle, upstream output digests, extension/model/tool components, and parameter/budget digests.
- Unknown, unsupported, setup-missing, rejected, havoc, and budget-exceeded cases are first-class rows.
- Extension/model facts must bind to existing stable facts or declared synthetic identities; malformed facts become validation diagnostics or rejected rows.
- Eval expected rows use compact payload fragments and invariants, not raw source, absolute paths, parser IDs, timestamps, or dense IDs.

## Files To Reuse

- `crates/polint/src/analysis/ids.rs` — add `DataFlowNodeId`, `DataFlowEdgeId`, model/budget/path IDs.
- `crates/polint/src/analysis/mod.rs` — register `pub(crate) mod data_flow;`.
- `crates/polint/src/core/mod.rs` — add `AnalysisDb` storage/accessors.
- `crates/polint/src/analysis_kernel/metadata.rs` — add fact families.
- `crates/polint/src/analysis_kernel/provider.rs` and `mod.rs` — provider manifest and execution order.
- `crates/polint/src/analysis_kernel/debug.rs`, `validation.rs` — debug/validation integration.
- `crates/polint/src/eval/model.rs`, `fixtures.rs` — fixture area, observation, and taxonomy.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` — provider order update.

## Plan Handoff Notes

- Start with compact fact contracts and local value-flow rows; keep source/sink semantics as model facts layered over generic value flow.
- Avoid a dependency cycle with refined calls. Phase 38 consumes refined calls; refined calls do not consume data flow.
- Make query-scoped path search useful for fixtures and Phase 39 consumers, but defer rich evidence rendering and SARIF/JSON evidence bundles to Phase 39.
