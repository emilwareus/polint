# Phase 51: Adaptation Model Layer - Patterns

**Generated:** 2026-06-04
**Purpose:** Inline pattern map for Phase 51 planning. Research is disabled for this phase, so this file records the existing code shapes that implementation should follow.

## Module And Visibility

| New Surface | Closest Existing Pattern | Notes |
|-------------|--------------------------|-------|
| `crates/polint/src/analysis/adaptation/mod.rs` | `analysis::solver::ts_object_model`, `analysis::semantic_graph` | Keep the whole module private to `analysis` and expose only `pub(crate)` types needed by graph/solver/eval internals. |
| `facts.rs` / `store.rs` | `semantic_graph::{facts,store}` and `solver::{facts,store}` | Stable keys first, dense IDs after deterministic sorting, no raw display-string identity. |
| `loader.rs` | Existing config/cache TOML parsing patterns | Parse TOML with structured errors; normalize model paths and content before digesting. |
| `validate.rs` | `semantic_graph::validate`, `solver::validate` | Fail closed with deterministic rejection reasons for non-resolving targets, broad patterns, oracle-shaped RHS, and budget hits. |
| `budget.rs` | `analysis::solver::budget::{GoRtaSubBudget, JsTokensSubBudget, JsObjectModelSubBudget}` | Add model-specific caps without changing public SDK shape. |

## Graph And Solver Integration

| New Work | Closest Existing Pattern | Notes |
|----------|--------------------------|-------|
| `ConstraintKind::ModelEdge` producer | `semantic_graph::build` constraint emission | `ModelEdge` is already in the closed vocabulary; Phase 51 should become its first real producer. |
| Accepted model lowering | `solver::ts_object_model` derived-edge handoff | Accepted facts lower to constraints, then solver output/provenance; rejected facts remain report-only. |
| Cache digest participation | `semantic_graph::cache_key`, `solver::cache_key` | Include normalized model files, accepted/rejected status, validator version, algorithm string, prompt hash, and relevant budget knobs. |
| Budget evidence | `solver::budget`, prior `BudgetExceeded` facts | Budget overflow must be explicit and deterministic, never silent truncation. |

## Evaluation And Reporting

| New Work | Closest Existing Pattern | Notes |
|----------|--------------------------|-------|
| Adaptation record extensions | `eval::adaptation::AdaptationRecord` | Reuse prompt hash, allowed/forbidden inputs, changed artifacts, digests, no-change reason, and validation. |
| Delta report fields | `eval::delta::AdaptationDeltaReport` | Extend accepted/rejected fact deltas from extension facts to model facts; keep case-sorted deterministic output. |
| Markdown report | `eval::markdown`, `eval::report` | Existing adaptation prompt/path/hash and changed-file rendering should gain model fact and held-out sections. |
| Fixture style | `tests/eval-fixtures/extension/{adaptation-delta,rejection-delta}` | Mirror accepted/rejected delta fixtures with model facts and oracle-sandbox cases. |

## Boundaries

- Do not expose adaptation facts through `polint::sdk::prelude::*`.
- Do not add `RefinedCallEdgeFact` projection in Phase 51; Phase 52 owns that.
- Do not claim corpus-level benchmark floors; Phase 54 owns promotion gates.
- Do not add broad built-in native shim libraries beyond minimal fixtures needed to prove the schema.
