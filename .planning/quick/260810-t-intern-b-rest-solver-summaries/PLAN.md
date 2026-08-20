# Quick Plan: T-INTERN-B rest-solver-summaries

## Goal
Migrate identity-owning solver/summaries/slicing/incremental-key fields from `stable_key: String` (and related `*_stable_key: String` fact identities) to `StableKeyId`, with ControlOrder preserving deterministic resolved-text ordering.

## Scope
- analysis/solver: DerivedEdgeFact, ContributingFact, Go RTA dispatch-site identities, TS points-to callsite/constraint identities + producers/stores/validation/digest/provenance consumers
- analysis/summaries: SummaryFact, SummaryEventFact, FunctionSummaryState callable/stable identities + builders/stores/SCC/provider digests
- analysis/slicing + analysis_kernel/incremental/keys.rs: true identities → StableKeyId; projected/debug path text → `*_text` rename (no dual fields)
- policy_queries.rs ControlOrder: never order by StableKeyId allocation; keep resolved-text deterministic order

## Out of scope
- FactMeta / stable_key_owners (T-INTERN-C)
- Solver densification
- Public import path / leak allowlist changes

## Regression
Fix `solver_derived_call_edges_project_to_refined_calls` (fixture StableKeyId(0) vs contributing text lookup) as part of solver/refined-call integration.

## Validation
cargo check/clippy/fmt + focused tests + public_surface_leak + determinism_gate; golden once if green
