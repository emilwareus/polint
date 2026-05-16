# Analysis Kernel Decision Log

## D1. Kernel Style

Question: Should polint use a fixed pipeline, Salsa, Datalog, property graph, or hybrid kernel?

Options:

| Option | Pros | Cons | Decision |
|---|---|---|---|
| Keep fixed pipeline | Simple, current code works | Does not scale to extensions, recursive analyses, provenance, invalidation | Reject as long-term |
| Adopt Salsa now | Strong incrementality, proven in rust-analyzer | Too much commitment before fact families stabilize; relation-heavy outputs still need solvers | Defer |
| Adopt Datalog/Souffle now | Great for recursion/fixpoints | Public/product mismatch; engine lifecycle too heavy | Defer as public engine |
| Public property graph | Powerful traversal model | API leakage, provenance/cache complexity | Reject as public API |
| Hybrid provider DAG plus relation sub-engine | Fits current code and future analyses | More design work | Choose |

Decision: Build a hybrid provider DAG, with internal relation/fixpoint support for recursive families.

## D2. Public API Shape

Question: Should rules see the kernel?

Decision: No. Rules should see typed SDK fact views. Kernel types remain `pub(crate)`.

Rationale: Everything public is a liability. The product should expose stable, ergonomic facts, not mutable storage internals.

## D3. Storage Rewrite Timing

Question: Should `AnalysisDb` be replaced before kernel work?

Decision: No. Keep `AnalysisDb` initially and add provider/layer/provenance sidecars.

Rationale: The main risk is orchestration and lifecycle, not Vec storage. A storage rewrite before provider manifests would increase blast radius without proving the kernel.

## D4. Provenance Storage

Question: Inline provenance into every fact or use side tables?

Decision: Use side tables keyed by `(FactFamily, RunId)`.

Rationale: Most rules do not need provenance on every query. Side tables preserve performance and allow richer metadata for extension/future facts.

## D5. Extension Merge

Question: Can extensions override native facts?

Decision: Not in the first implementation. Additive union only, with explicit validated suppressions delayed.

Rationale: Additive facts are useful and lower risk. Replacement/suppression can hide true findings and requires stronger validation/evaluation.

## D6. Cache Keys

Question: Should parser caches depend on rule digest?

Decision: Long term, no. Parser caches should depend on source, parser/language lifecycle config, provider/schema version, and polint version. Rule digest belongs in rule diagnostics and provider plan selection, not raw parsing.

Rationale: Current key is safe but over-invalidates. Layer-specific keys will improve CI and agent iteration.

## D7. Unknowns

Question: Should unresolved behavior be represented as missing facts or explicit facts?

Decision: Explicit facts.

Rationale: Agents need actionable uncertainty. Silent absence cannot distinguish "does not exist" from "not analyzed."

## D8. Recursive Analysis Engine

Question: Should relation/fixpoint machinery be implemented before entrypoints/extensions?

Decision: No. Build provider/layer/provenance/cache first, then add relation/fixpoint machinery for call graph/data flow.

Rationale: Recursive analyses need the kernel. Building them before validation/cache/provenance repeats the old problem.

## D9. Validation Strictness

Question: Should validation be optional?

Decision: Extension validation must be mandatory. Native deep invariant validation can be test/debug heavy, but lightweight checks should always run.

Rationale: Extensions are the high-capability, high-risk surface. They cannot bypass trust gates.

## D10. First Vertical Slice

Question: Which fact family should prove the kernel?

Decision: `Entrypoints<'_>`.

Rationale: Entry points are high leverage for call graph and data flow, narrow enough to validate, easy to fixture-test, and ideal for agent-authored repo-local models.

