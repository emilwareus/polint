# Phase 50: JS/TS Object/Property/Prototype/`this` Model & Driver - Context

**Gathered:** 2026-06-04
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 50 --auto`

<domain>
## Phase Boundary

Phase 50 delivers **JS-05**: a private JS/TS object/property/prototype/class/`this` model that feeds the unified solver after Phase 49's function-token driver. It should add:

1. Private TS frontend object-model extraction under `crates/polint/src/ts/object_model/`.
2. A private solver driver under `crates/polint/src/analysis/solver/ts_object_model/`.
3. Allocation-site abstraction for object literals, arrays, functions/classes, class instances, prototypes, modules, and the roadmap-required callable receiver forms.
4. Property read/write modeling with exact buckets for known keys and bounded buckets for computed or unknown keys.
5. Prototype/class lookup with explicit termination.
6. `this` resolution for arrows, methods, constructors, bound functions, `call`, and `apply` where supporting facts are known.
7. Native fixtures and Jelly benchmark evidence showing object-model recall lift without precision flooding.

This phase is the **JS/TS object model only**. It explicitly does **not**:

- Implement broad native/library callback modeling such as `Array.prototype.map`, `Promise.then`, or framework callbacks unless a case is required for the roadmap-named `bind`/`call`/`apply` receiver forms. Broad models remain Phase 51/adaptation or future native-model work.
- Rework `refined_calls::provider` to project over solver output. That is GRAPH-05, Phase 52.
- Consolidate the unsupported/unknown taxonomy or expose `polint inspect unknowns`. That is TAX-01, Phase 52.
- Add adaptation model facts or `ModelEdge` producers. That is ADAPT-01/ADAPT-02, Phase 51.
- Perform the milestone-wide cache/budget sweep. That is CACHE-01/CACHE-02, Phase 53.
- Promote solver, object-model, property, or receiver internals to the public SDK, runner, README user workflows, or public CLI JSON. v1.3 internals stay `pub(crate)`.

</domain>

<decisions>
## Implementation Decisions

### Object Identity And Allocation Sites

- **D-01:** Add a private `ts::object_model` frontend layer, likely `crates/polint/src/ts/object_model/`, for allocation sites, property operations, receiver facts, class/prototype facts, and any TS-specific lowering needed before the solver driver runs. Keep every type `pub(crate)`.
- **D-02:** Add a private `analysis::solver::ts_object_model` driver, likely `crates/polint/src/analysis/solver/ts_object_model/`, registered as a `SolverPolicy` or equivalent edge-contributing policy beside `go_rta` and `ts_tokens`. The planner may choose exact module slicing, but production must still route through `SolverEngine::run_to_solver_output`.
- **D-03:** Represent objects as stable allocation-site tokens composed from existing identities: TS inventory function/callsite facts, semantic graph nodes, file/span, lexical container, module/package context, and kind. Do not key objects by display names, run-local Oxc node IDs, or TypeScript type names alone.
- **D-04:** Allocation-site kinds in scope are object literals, arrays, function objects, class declarations/expressions, class instances from `new`, prototype objects, module namespace/import objects where already represented, and minimal selected native/function objects needed for `bind`/`call`/`apply`.
- **D-05:** Stable keys are the public contract inside the private engine: length-prefixed parts, BTree ordering, dense IDs only after stable-key sort, and no source text or AST payload duplication. Function/object/span identities compose existing Phase 42/45 identities rather than copying them into a parallel namespace.

### Property Reads, Writes, And Computed Keys

- **D-06:** Consume and extend the existing semantic-graph constraint vocabulary instead of inventing a parallel property language. `Alloc`, `FieldStore`, `FieldLoad`, `CopyEdge`, and `CallConstraint` remain the core solver inputs; new object-model facts should lower into those constraints where practical.
- **D-07:** Property stores write function/object tokens into per-object property buckets; property reads load those tokens into the callee/value place. A call like `obj.method()` resolves only when the receiver object's property bucket contains callable tokens. Never derive a callee from the property name alone.
- **D-08:** Use exact property buckets for statically-known keys: static identifiers, private identifiers, string literals, numeric literals normalized to the same property-key convention the planner selects, and simple constant template literals if existing extraction can prove them.
- **D-09:** Computed or unknown keys flow into bounded computed/unknown buckets. Unknown reads may consult the computed bucket plus exact keys only under a conservative documented rule chosen by the planner; the default posture is precision-first and should not read every exact property bucket just to improve recall.
- **D-10:** Bucket overflow is explicit. If an object exceeds max property buckets, computed buckets, or tokens per property, the solver latches `BudgetStatus::BudgetExceeded` and records evidence for the later unknown taxonomy. It must not silently truncate and must not flood every property read with every possible target.
- **D-11:** Preserve unresolved reasons from Phase 45. `PropertyFlowRequired`, `PrototypeModelRequired`, and `ThisModelRequired` should become resolved only when this model has enough facts to justify a target. `Eval`, non-string dynamic import, unsupported native callback, and external package gaps remain honest unresolved/unsupported rows.

### `this`, Calls, Constructors, And Receiver Binding

- **D-12:** Model the roadmap-named receiver forms: arrow lexical `this`, method receiver binding, constructor/new instance binding, bound functions, and `Function.prototype.call` / `Function.prototype.apply` when callee and receiver facts are known.
- **D-13:** Represent `this` as an explicit semantic place/receiver node in the object-model input snapshot. For a method call, copy the receiver object token into the callee's receiver place before deriving call edges. For arrows, capture the lexical receiver instead of rebinding at the callsite.
- **D-14:** Constructors allocate an instance object token, bind `this` to that instance, and connect instance/prototype/class facts so method calls on the result can traverse the prototype chain.
- **D-15:** Bound functions carry a stable bound-receiver summary when the base callable and receiver are known. `call`/`apply` are supported only when the target function token and first-argument receiver can be resolved under budget. Otherwise retain `ThisModelRequired` or the appropriate unsupported native-callback reason.
- **D-16:** Do not use TypeScript declared types as runtime receiver identity. Type facts may narrow candidates only when already represented as stable semantic facts with provenance; they must not fabricate JS runtime objects.

### Prototype, Class, And Accessor Lookup

- **D-17:** Model class method definitions and object methods as property stores on the appropriate prototype/static/instance bucket. Class constructors, static methods, instance methods, accessors, and `extends` links should get distinct stable facts.
- **D-18:** Prototype lookup is bounded by a visited set and explicit max-depth/fanout caps. Cycles, excessive depth, and dynamic prototype mutation produce budget/unsupported evidence rather than unbounded traversal.
- **D-19:** Accessor getter/setter call edges are in scope when the getter/setter function is statically represented and the property access is known. Dynamic accessors or descriptor mutation stay unsupported until adaptation/native-model phases provide validated facts.
- **D-20:** Prototype mutation support should be conservative: direct `Ctor.prototype.name = fn` and simple `Object.setPrototypeOf` / `__proto__` patterns may be included if they lower to stable facts cleanly. Dynamic or reflective mutation must stay explicit unknown evidence.

### Solver Integration, Cache, And Configuration

- **D-21:** The object model should integrate with the existing solver provider without regressing Go RTA or TS token output. Solver output merges language-agnostic `CopyEdge` closure, Go RTA, TS tokens, and TS object-model derived edges under one `SolverBudget`.
- **D-22:** Add object-model budget knobs as a distinct JS object sub-budget, not by overloading token caps. The exact config shape is planner discretion, but prefer a minimal `.polint.toml` surface under the existing `[solver.js]` namespace, using explicit object-prefixed names if a nested table would create unnecessary public configuration churn.
- **D-23:** Required object-model caps include at least: max objects per variable/place, max properties per object, max tokens per property, max computed/unknown buckets per object, max prototype depth, max receiver candidates per callsite, and max object-model worklist steps.
- **D-24:** Configured caps must overlay only positive values onto documented defaults, matching the existing zero-clamp pattern for `[solver.go]` and `[solver.js]`. A `0` cap is a typo/default fallback, not a hidden way to disable the object model.
- **D-25:** Cache participation is mandatory. The solver parameter digest must include a frozen algorithm string such as `ts_object_model_fixpoint_v1`, all object-model budget knobs, and every upstream digest consumed by object extraction or solving. If object-model extraction reads TS inventory/scope/binding/MIR facts not already folded into `polint.semantic_graph`, add explicit digest participation.
- **D-26:** The object-model output digest must include run-level budget status, stable derived-edge keys, status/precision, provenance fragments, object-model fact stable keys, and object-model budget/config parts. A run that truncated property/prototype exploration must never share a digest with a complete run.

### Verification And Acceptance

- **D-27:** Add native fixtures for object literal method calls, property read/write flow, computed-property bucket collapse, class constructor + instance method, static method, prototype chain and `extends`, getter/setter access, arrow-vs-method `this`, bound functions, and `call`/`apply`.
- **D-28:** Include at least one fixture that was previously unresolved as `PropertyFlowRequired`, one as `PrototypeModelRequired`, and one as `ThisModelRequired`, proving the new model converts only justified cases to derived call edges.
- **D-29:** Add tight-budget fixtures for property bucket overflow, receiver fanout overflow, and prototype-depth termination. Each must assert explicit `BudgetExceeded` / unknown evidence and must prove no fake target is emitted after the cap.
- **D-30:** Extend or mirror `tests/eval-fixtures/ts-tokens/alias-parameter-return/` for object/property cases. The existing `holder["propertyTarget"]()` case is a useful boundary seed: Phase 49 leaves it outside token flow; Phase 50 should either resolve it with object facts or keep it explicitly unsupported if the key is still too dynamic.
- **D-31:** Update the polyglot canary to show Go RTA, TS token, and TS object-model policies can all contribute intra-language edges without Go<->TS cross-language edges.
- **D-32:** Record Jelly benchmark deltas against `oracle-jelly` and `whole-repo` scoring modes. The goal is a real object-model recall lift after token propagation while precision stays first-class; recall-by-flooding is a failure, not a shortcut.
- **D-33:** Keep the Phase 43 determinism gate and Phase 42 public-surface leak gate green. Do not extend `ALLOWED_PRELUDE` in `crates/polint/tests/public_surface_leak.rs`.

### Agent's Discretion

- Exact file slicing inside `ts/object_model/` and `analysis/solver/ts_object_model/`.
- Exact fact/newtype names for object tokens, property keys, receiver places, prototype edges, and budget evidence.
- Whether object extraction is a separate provider before `polint.semantic_graph`, a semantic-graph build extension, or a closed snapshot built inside `polint.solver`, provided consumed inputs participate in digests and provider-order dependencies remain deterministic.
- The final `[solver.js]` object budget field names, provided they are minimal, positive-clamped, documented, and digest-participating.
- Natural plan slicing. A likely split is: (1) object facts/lowering + property-key extraction, (2) solver object-model policy + receiver/prototype semantics + budgets/cache, (3) fixtures/Jelly/polyglot/determinism/leak proof.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 50 goal, dependencies on Phase 49 and Phase 45, JS-05 success criteria, and Phase 51-54 boundaries.
- `.planning/REQUIREMENTS.md` - JS-05 requirement text, JS-04 upstream boundary, GRAPH-05/TAX-01 later ownership, CACHE-01/CACHE-02 later consolidation, and BENCH-01 final promotion gate.
- `.planning/PROJECT.md` - v1.3 milestone goal, Jelly baseline, shared semantic graph / solver core keystone, precision-first benchmark posture, and no-public-SDK-promotion discipline.
- `.planning/STATE.md` - current Phase 50 focus and open repo-admin action T-42-04-10 for branch protection leak-gate checks.

### Immediate Upstream Phase Context

- `.planning/phases/49-js-ts-function-token-propagation-driver/49-CONTEXT.md` - Primary upstream for TS tokens: `ts_tokens` policy, function-token boundaries, `"too-many-tokens"` sentinel, `[solver.js]` budget pattern, token fixtures, and explicit deferral of property/prototype/`this` modeling.
- `.planning/phases/45-js-ts-inventory-scope-bindings-module-graph-direct-calls/45-CONTEXT.md` - TS inventory, lexical scope/binding, module graph, direct binding facts, Jelly span discipline, and unresolved reasons (`PropertyFlowRequired`, `PrototypeModelRequired`, `ThisModelRequired`) Phase 50 should consume.
- `.planning/phases/47-unified-solver-core-derived-edge-provenance/47-CONTEXT.md` - Solver core, `SolverPolicy`, `PolicyOutcome`, `DerivedEdgeProvenance`, provider/cache/determinism/leak discipline.
- `.planning/phases/48-go-rta-driver/48-CONTEXT.md` - Multi-policy production seam, Go RTA sub-budget pattern, budget-exceeded honesty, and polyglot canary precedent that Phase 50 must preserve.
- `.planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-CONTEXT.md` - Constraint vocabulary (`Alloc`, `FieldLoad`, `FieldStore`, `CopyEdge`, `CallConstraint`), semantic graph identity, composition-over-duplication, provider slot discipline, and determinism inherited by solver phases.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` - Determinism gate, scoring-mode discipline, dense-ID-after-sort rule, and provider digest recipe.
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Jelly identity/span rendering, identity-vs-unsupported taxonomy, dedup ordering, and public-surface-leak gate.

### Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - Primary v1.3 architecture source. Section 8 defines JS/TS object/property/prototype/class/`this` modeling, expected Jelly impact, complexity, property propagation costs, and precision risks.
- `research/type-alias-points-to/languages/typescript-javascript.md` - TS/JS accuracy model: object properties, function/object/module tokens, dynamic behavior as unknown, and recommended bounded points-to/object/property path.
- `research/call-graphs/FINAL-REPORT.md` - Dynamic language call-graph framing: property calls, callbacks, module systems, prototype chains, and precision tiers.
- `research/evaluation-harness/STANDARD.md` - Native fixture and deterministic observed-output conventions.
- `research/evaluation-harness/decisions/decision-log.md` - Benchmark architecture decisions and hidden/internal-first evaluation posture.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/solver/ts_tokens/{mod.rs,inputs.rs,fixpoint.rs,dispatch.rs}` - Current JS-04 policy, token state, callsite resolution, provenance, sentinel handling, and fixture-driven expectations to extend without regression.
- `crates/polint/src/analysis/solver/{policy.rs,engine.rs,provider.rs,budget.rs,cache_key.rs,facts.rs,store.rs,provenance.rs,validate.rs}` - Solver policy registration, output merging, budget status, cache-key recipe, derived edge facts, provenance, and validation patterns.
- `crates/polint/src/analysis/semantic_graph/{constraints.rs,facts.rs,build.rs,provider.rs,store.rs,cache_key.rs,validate.rs}` - Constraint vocabulary and semantic graph stable identity/lowering patterns.
- `crates/polint/src/ts/inventory/{facts.rs,extract.rs,store.rs}` - Function/callsite identities, class/method/constructor/accessor inventory, and Jelly-shaped spans for allocation/object identity.
- `crates/polint/src/ts/scope/{facts.rs,extract.rs,store.rs}` - Lexical bindings and scope facts needed for property/receiver facts and arrow lexical `this`.
- `crates/polint/src/ts/binding/{facts.rs,direct.rs,store.rs}` - Direct binding facts and unresolved reason taxonomy Phase 50 should resolve only with justified object/prototype/receiver facts.
- `crates/polint/src/analysis/mir/lower_ts.rs` - TS MIR lowering context for property operations and receiver-sensitive expression shapes.
- `crates/polint/src/config/mod.rs` - Existing `[solver.go]` and `[solver.js]` config mapping plus positive-cap overlay pattern.
- `crates/polint/src/eval/ts_tokens.rs` - Private executable fixture gate for TS token solver rows; mirror or extend for object-model proof.
- `crates/polint/src/analysis/refined_calls/ts_js.rs` - Existing v1.2 TS/JS projection awareness only. Do not let this phase rework public refined-call projection.
- `crates/polint/tests/public_surface_leak.rs` - Public API leak gate. New object-model and solver types must stay unreachable from `polint::sdk::prelude::*`.
- `tests/eval-fixtures/ts-tokens/alias-parameter-return/` - Existing token fixture with a property-call boundary seed.
- `tests/eval-fixtures/ts-tokens/token-explosion/` - Budget-exhaustion fixture precedent.
- `tests/eval-fixtures/polyglot-canary/go-ts/` - Mixed Go+TS canary to extend for object-model no-cross-language proof.
- `tests/eval-fixtures/determinism/ts_tokens/` and `tests/eval-fixtures/determinism/ts_reachable/` - Determinism fixture precedents.
- `tests/eval-fixtures/semantic-graph/ts_direct_bindings/` and `tests/eval-fixtures/semantic-graph/ts_graph/` - TS semantic graph constraint fixture precedents.
- `tests/eval-fixtures/jelly/ts-inventory-spans/` - Jelly span/identity fixture context for any object-model Jelly delta proof.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::solver::ts_tokens` is already real and private. It owns a closed input snapshot, BTree-based fixpoint, `"too-many-tokens"` sentinel, provenance-rich `DerivedEdgeFact`s, and candidate caps.
- `SolverEngine::run_to_solver_output` already merges policy-derived edges with the language-agnostic `CopyEdge` closure, normalizes by stable key, and reports run-level `BudgetStatus`.
- `SolverBudget` already has `go` and `js` sub-budget channels. The JS token budget is a useful pattern but should not be overloaded for object/property/prototype costs.
- `config::SolverConfig::to_js_sub_budget` already implements positive-only overlay for `[solver.js]` token caps.
- `ConstraintKind` already includes `Alloc`, `FieldLoad`, `FieldStore`, and `CallConstraint`, which are the right vocabulary for object/property modeling.
- `TsDirectBindingReason` already separates `TokenFlowRequired`, `PropertyFlowRequired`, `PrototypeModelRequired`, and `ThisModelRequired`, giving this phase precise handoff categories.
- Native eval fixtures already cover TS token propagation, token explosion, polyglot canary, semantic graph TS bindings, Jelly spans, and determinism.

### Established Patterns

- Every v1.3 graph-engine family stays private until benchmark gates and public-contract reviews promote it.
- Stable keys compose existing facts and never rely on run-local dense IDs.
- Determinism is achieved with BTree accumulation, sorted stable keys, and dense IDs only after sorting.
- Budget exhaustion is an explicit signal. Existing Go RTA and TS token phases treat `0` config caps as default fallback rather than disabling analysis.
- Solver cache digests fold algorithm-version strings, budget knobs, upstream provider output digests, run-level budget status, edge stable keys, precision/status, and provenance fragments.
- Dynamic behavior is represented as unknown/unsupported unless stable facts prove a target.

### Integration Points

- `analysis::solver::provider::derive_solver_with_cache_stats` is the production integration point for new edge-contributing solver policies.
- `analysis::semantic_graph::build` is the likely place to lower new object/property frontend facts into `Alloc`, `FieldLoad`, `FieldStore`, `CopyEdge`, and `CallConstraint`.
- `AnalysisDb` replacement/store paths need any new object-model facts to be normalized, validated, and digest-visible.
- `eval::ts_tokens` or a sibling `eval::ts_object_model` should provide private executable proof before public refined-call projection exists.
- The polyglot canary is the guard that shared solver work does not create Go<->TS edges.

</code_context>

<specifics>
## Specific Ideas

- Suggested object-model fixture cases: `const holder = { target }; holder.target()`, `holder["target"]()`, `holder[key]()` with a computed bucket, `class C { m() { target(); } } new C().m()`, `class D extends C {}`, `Ctor.prototype.m = target`, getter/setter calls, arrow lexical `this`, method receiver `this`, constructor `this`, `fn.bind(receiver)()`, `fn.call(receiver)`, and `fn.apply(receiver, args)`.
- Preserve the exact `"too-many-tokens"` sentinel for token overflow. Add a distinct object/property budget signal rather than reusing that sentinel as an object token.
- If adding config fields under `[solver.js]`, prefer explicit names such as `max_object_properties_per_object`, `max_object_tokens_per_property`, `max_object_prototype_depth`, and `max_object_receiver_candidates_per_callsite`.
- A good first acceptance target is converting the existing token fixture's `holder["propertyTarget"]()` from a Phase 49 boundary case into a justified object-property edge, then adding class/prototype/`this` fixtures around it.
- Keep broad native models out of this phase except the roadmap-required `bind`/`call`/`apply` receiver semantics.

</specifics>

<deferred>
## Deferred Ideas

- Broad native/library callback models (`Array.prototype.map`, `forEach`, `Promise.then`, event emitters, framework callbacks) remain Phase 51/adaptation or future native-model work.
- Adaptation-model facts and validated repo-local framework/native models remain Phase 51.
- Refined calls projection over solver output and public `RefinedCallEdgeFact` preservation remain Phase 52.
- Unsupported/unknown taxonomy consolidation and `polint inspect unknowns --format json` remain Phase 52.
- Milestone-wide cache and solver-budget consolidation remains Phase 53.
- Hard benchmark promotion gates, per-language floors, and final public-API leak CI gate promotion remain Phase 54.
- Public SDK views over solver/object-model output are out of v1.3 and deferred to v1.4+.

</deferred>

---

*Phase: 50-JS/TS Object/Property/Prototype/`this` Model & Driver*
*Context gathered: 2026-06-04*
