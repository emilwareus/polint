# Phase 50 Pattern Map

**Phase:** 50 - JS/TS Object/Property/Prototype/`this` Model & Driver
**Generated:** 2026-06-04
**Mode:** inline pattern mapping because GSD subagents are not installed in this workspace.

## Target File Families

| Role | New / Modified Files | Closest Analogs | Pattern To Reuse |
|------|----------------------|-----------------|------------------|
| TS object-model facts and extraction | `crates/polint/src/ts/object_model/{mod.rs,facts.rs,extract.rs,store.rs}` | `crates/polint/src/ts/inventory/*`, `crates/polint/src/ts/binding/*`, `crates/polint/src/ts/scope/*` | Crate-private fact rows with stable keys, dense IDs assigned by stores, deterministic sort/dedup, Oxc extraction kept behind TS adapter boundaries. |
| Semantic graph lowering | `crates/polint/src/analysis/semantic_graph/{build.rs,constraints.rs,cache_key.rs,provider.rs,validate.rs}` | Existing `collect_ts_direct_bindings`, `ConstraintKind::{Alloc,FieldLoad,FieldStore,CallConstraint}` | Compose existing semantic node identities; lower to closed constraint vocabulary rather than parallel object-edge facts when solver input can be a constraint. |
| Solver object driver | `crates/polint/src/analysis/solver/ts_object_model/{mod.rs,inputs.rs,fixpoint.rs,dispatch.rs,prototype.rs,receiver.rs}` | `analysis/solver/ts_tokens/*`, `analysis/solver/go_rta/*` | Closed input snapshot, deterministic `VecDeque` worklist, BTree accumulation, `SolverPolicy` output via `DerivedEdgeFact`, conservative precision ceiling, provenance from stable contributing facts. |
| Budget/config/cache | `analysis/solver/budget.rs`, `analysis/solver/cache_key.rs`, `analysis/solver/provider.rs`, `config/mod.rs` | `GoRtaSubBudget`, `JsTokensSubBudget`, `SolverConfig::to_js_sub_budget`, `solver_provider_parameter_digest` | Positive-only cap overlay, frozen algorithm-version strings, explicit budget parts in parameter/output digest, run-level `BudgetStatus` in output digest. |
| Native eval fixtures | `tests/eval-fixtures/ts-object-model/*`, `crates/polint/src/eval/ts_object_model.rs`, `eval/mod.rs` | `tests/eval-fixtures/ts-tokens/*`, `crates/polint/src/eval/ts_tokens.rs`, `tests/eval-fixtures/go-rta/*` | Self-contained fixture repos with `.polint.toml`, private eval gate that inspects internal solver rows, explicit budget and determinism cases. |
| Polyglot/determinism/leak proof | `tests/eval-fixtures/polyglot-canary/go-ts/*`, `tests/eval-fixtures/determinism/ts_object_model/*`, `public_surface_leak.rs` | Phase 48/49 canary and determinism fixture layout | Assert intra-language TS object edges, no Go<->TS edges, byte-identical observed output, and no `ALLOWED_PRELUDE` expansion. |

## Reusable Code Patterns

### Stable Fact Rows

- Fact rows are `pub(crate)`, derive deterministic ordering/serialization where needed, and carry stable keys built from existing identities.
- Dense IDs are read concerns, assigned after normalized sort/dedup, and must not enter stable serialized digests.
- New TS object-model facts should mirror `TsInventoryFunctionFact`, `TsDirectBindingFact`, and semantic graph facts: identity fields first, stable key last, status/reason explicit.

### Closed Solver Inputs

- `TsTokenInputs::from_db(db)` is the closest current pattern: build a closed snapshot from already-populated `AnalysisDb` stores, normalize it, then pass it into a policy that owns it.
- Phase 50 should use the same shape for `TsObjectModelInputs::from_db(db)`, with indexes for allocation tokens, property stores/loads, receiver places, prototype links, class metadata, call obligations, and eligible unresolved handoffs.

### Budget Honesty

- `BudgetStatus::BudgetExceeded` is run-level evidence and also digest input.
- Token overflow uses `"too-many-tokens"` as a token sentinel. Object/property overflow should use distinct object-model budget reasons, not the token sentinel as a fake object/callee.
- `0` config caps fall back to defaults. Do not use zero as disable/unbounded.

### Derived Edges

- `ts_tokens::dispatch::resolve_token_callsite` is the direct analog for object-property call edges.
- Object-derived edges should be `DerivedEdgeFact { source: caller function node, target: callee function node, provenance }`.
- Provenance must include callsite constraint, property load/store facts, allocation/prototype/receiver facts, and token-flow facts when object resolution composes with Phase 49 tokens.

### Verification Shape

- Keep private executable tests in `crates/polint/src/eval/` while the public refined-call projection is deferred.
- Use fixture comments to state which cases remain unsupported and why.
- Pair happy-path fixtures with tight-budget fixtures so precision and termination are proven together.

## Risk Notes For Planner

- The main planning risk is digest completeness. If `ts_object_model` reads TS inventory/scope/binding/MIR facts directly, either expose an object-model provider output digest or add those consumed digests to `polint.solver`.
- The second risk is scope creep into native/library models. Keep broad callbacks deferred; implement only the roadmap-named receiver forms needed for `bind`/`call`/`apply`.
- The third risk is precision flooding from computed keys and prototype lookup. Plans must include exact caps and tests that prove overflow produces evidence instead of broad edges.
