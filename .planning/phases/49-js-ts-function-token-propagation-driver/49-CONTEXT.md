# Phase 49: JS/TS Function-Token Propagation Driver - Context

**Gathered:** 2026-06-03
**Status:** Ready for planning
**Mode:** `/gsd-discuss-phase 49 --auto`

<domain>
## Phase Boundary

Phase 49 makes the reserved `TsTokensPolicy` stub real. Phase 45 produced the JS/TS inventory, lexical scope, binding, module graph, and direct binding constraints. Phase 47 built the unified private `analysis::solver` core and the `SolverPolicy` seam. Phase 48 proved that seam with the Go RTA policy while leaving `TsTokensPolicy` intentionally empty. This phase delivers exactly **JS-04**:

1. A private JS/TS function-token propagation driver in `analysis::solver::ts_tokens`.
2. Function-token propagation through `CopyEdge` and call/return constraints, including local aliases, assignments, parameters, returns, and closures.
3. Per-variable token caps with a `"too-many-tokens"` sentinel.
4. `BudgetExceeded` reporting that downstream unknown taxonomy can consume, never silent precision loss.
5. Memory-ceiling proof for token-explosion inputs.
6. Per-language `solver_config.js.*` knobs and Jelly benchmark recall improvement without precision regression beyond the future Phase 54 floor.

This phase is the **JS/TS token driver only**. It explicitly does **not**:

- Implement JS/TS object, property, prototype, class, allocation-site, or `this` modeling. That is **JS-05, Phase 50**.
- Rework `refined_calls::provider` to project over solver output. That is **GRAPH-05, Phase 52**.
- Consolidate the unsupported/unknown taxonomy or expose `polint inspect unknowns`. That is **TAX-01, Phase 52**.
- Add adaptation model facts or `ModelEdge` producers. That is **ADAPT-01/02, Phase 51**.
- Promote public SDK or CLI graph/call-graph contracts. v1.3 solver and token types stay `pub(crate)`.
- Enforce the final benchmark promotion gate. That is **BENCH-01, Phase 54**.

</domain>

<decisions>
## Implementation Decisions

### Solver Integration Seam

- **D-01:** Add a private `analysis::solver::ts_tokens` module, likely under `crates/polint/src/analysis/solver/ts_tokens/`. Every new type, fact, index, store, and helper stays `pub(crate)`. Do not promote any token type to `polint::sdk`, `runner`, README user-facing contracts, or public CLI JSON.
- **D-02:** Replace the honest `TsTokensPolicy` stub in `crates/polint/src/analysis/solver/policy.rs` with a real `SolverPolicy` implementation. The policy owns a closed JS/TS token input snapshot and returns `PolicyOutcome { derived_edges, budget_status, steps, .. }`.
- **D-03:** Keep production routed through `SolverEngine::run_to_solver_output` in `crates/polint/src/analysis/solver/provider.rs`. The output must merge three sources under one `SolverBudget`: existing language-agnostic `CopyEdge` closure, Go RTA policy edges, and TS token policy edges. Points-to byte-identity and Go RTA behavior must not regress.
- **D-04:** TS token-resolved calls are `DerivedEdgeFact`s in the existing unified solver vocabulary. Use `caller-function-node -> callee-function-node` call edges with `DerivedEdgeProvenance` listing the contributing callsite, token-flow constraints, source binding/inventory facts, and the producing `CallConstraint`. Do not create a parallel public TS call-edge family.
- **D-05:** The existing v1.2 `analysis::refined_calls::ts_js` function-token projection is awareness context only. Phase 49 should make solver-owned token propagation the authoritative private producer for JS-04. Phase 52 later decides how `refined_calls` projects solver output while preserving the public `RefinedCallEdgeFact` contract.

### Token Carrier And Propagation Scope

- **D-06:** The propagated value is a **function token** keyed by existing Phase 45 TS inventory function identity (`TsInventoryFunctionId` plus stable key / semantic function node). Tokens compose existing identities; they must not duplicate function, module, or span payloads.
- **D-07:** Token variables should be stable semantic graph place/binding nodes where available, not string names or run-local parser positions. Dense IDs are assigned only after stable-key sort.
- **D-08:** Phase 49 propagates tokens through direct local copies and the roadmap-named call/return flows: assignment/aliasing, parameter passing, return values, and closures. It should consume Phase 45 `CopyEdge` and `CallConstraint` rows plus TS inventory/scope/binding facts needed to map call arguments, parameters, and returns.
- **D-09:** Closures are in scope only as function tokens captured and returned/passed through lexical bindings. Full object environment modeling, property bags, prototype walks, `this` binding, `.call`/`.apply`/`.bind`, and native callback semantics are deferred to Phase 50 or adaptation phases unless a case can be represented as plain function-token flow without object semantics.
- **D-10:** Direct bindings already resolved by Phase 45 remain direct evidence. The token driver should convert `TokenFlowRequired` unresolved/direct-binding cases where the missing edge is genuinely function-token flow. It must leave `PropertyFlowRequired`, `PrototypeModelRequired`, and `ThisModelRequired` unresolved/unsupported with their existing honest reasons.
- **D-11:** Native/library callbacks and framework models remain unresolved unless they are represented by validated model facts in later phases. Do not inflate Jelly recall by guessing callback targets.

### Budget, Sentinel, And Honesty

- **D-12:** Add a JS/TS sub-budget to `SolverBudget` beside `points_to` and `go`, e.g. `js: JsTokensSubBudget` or `ts: TsTokensSubBudget`. Required caps include at least per-variable token cap, propagation step/worklist cap, per-callsite candidate cap, and an optional closure-depth or return-flow cap if research/planning finds it necessary.
- **D-13:** When a variable exceeds the token cap, collapse its token set to a stable `"too-many-tokens"` sentinel. The sentinel is not a real function token and must never be rendered as a callable target. It exists to preserve monotonic state and to explain why precision stopped.
- **D-14:** Any overflow or worklist cap hit latches run-level `BudgetStatus::BudgetExceeded` and emits/records an honest budget signal consumed by later unknown taxonomy. Edges fully derived before exhaustion keep their honest status; edges not reached are not fabricated.
- **D-15:** Derived token edges never claim exact precision. Use the existing solver precision ceiling / conservative tiers and worst-trust provenance discipline from Phases 47 and 48.
- **D-16:** Token propagation must be deterministic: BTree-ordered accumulation, sorted stable keys, dense IDs only after sort, and stable sentinel rendering. No `HashMap` iteration order may leak into observed output or digests.

### Config And Cache Participation

- **D-17:** Extend `[solver]` config with a JS/TS sub-table, preferably `[solver.js]` because the roadmap says `solver_config.js.*`. Accept aliases only if the planner decides they are necessary; avoid exposing a broad or public CLI surface.
- **D-18:** Configured JS token caps must overlay positive values onto documented defaults, mirroring the Phase 48 zero-clamp pattern for `solver.go.*`. A `0` cap should not silently disable all token propagation.
- **D-19:** Every JS token budget knob participates in `solver_provider_parameter_digest` and in the solver output digest. Add a frozen algorithm-version string such as `ts_tokens_fixpoint_v1`; any algorithm or budget change must invalidate the solver cache.
- **D-20:** The solver provider digest must include upstream outputs consumed by the TS token driver. At minimum this includes `polint.semantic_graph`; planning must confirm whether TS inventory/scope/binding/module provider output digests are separately available or already folded into semantic graph / current kernel digests. If consumed but not digested, add the missing participation.
- **D-21:** The token driver must remain cache-compatible with Phase 53. Do the specific invalidation work needed now for JS-04, but leave the milestone-wide cache/budget consolidation sweep to Phase 53.

### Verification And Acceptance

- **D-22:** Add native TS token fixtures proving alias, assignment, parameter, return, and closure propagation. Include at least one previously `TokenFlowRequired` case that becomes resolved by the TS token policy.
- **D-23:** Add a token-explosion fixture with deliberately tight caps proving the `"too-many-tokens"` sentinel plus `BudgetExceeded` signal. The fixture should also prove the driver does not silently drop precision or produce fake targets.
- **D-24:** Add a memory-ceiling fixture for token explosion. Prefer a deterministic native eval fixture that asserts bounded output shape and budget status; if RSS is measured, keep it as a conservative ceiling, not a brittle exact benchmark.
- **D-25:** Add Jelly-focused evidence that aggregate JS/TS recall improves without unacceptable precision regression. Phase 54 owns the hard promotion floor, but Phase 49 must show a real delta from token propagation and no recall-by-flooding.
- **D-26:** Update the polyglot Go+TS canary so TS token policy activity is meaningful while preserving no Go<->TS cross-language edges. Phase 48's canary currently documents TS policy emptiness; Phase 49 should flip that expectation to "TS policy derives legitimate intra-TS token edges, still no cross-language interference, Go RTA unchanged."
- **D-27:** Keep the Phase 43 determinism gate and Phase 42 public-surface leak gate green. Do not extend `ALLOWED_PRELUDE` in `crates/polint/tests/public_surface_leak.rs`.

### Agent's Discretion

- Internal file layout for `analysis::solver::ts_tokens/` is planner discretion, provided it mirrors the solver/go_rta style and remains crate-private.
- Exact names for token variables, token-set facts, sentinel enum/newtype, and JS sub-budget fields are planner discretion, provided stable keys, cache participation, and budget behavior are explicit.
- Natural plan slicing is planner discretion. A likely split is: (1) token input extraction/indexes + budget/config/cache keys, (2) token fixpoint policy + derived edge provenance, (3) fixtures/gates/Jelly/polyglot proof.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap And Requirements

- `.planning/ROADMAP.md` - Phase 49 goal, dependencies, JS-04 success criteria, Phase 49/48 parallel-eligibility note, and Phase 50/52/53/54 boundaries.
- `.planning/REQUIREMENTS.md` - JS-04 requirement text, JS-05 boundary, GRAPH-05/TAX-01 later-phase ownership, CACHE-01/CACHE-02 later consolidation, and BENCH-01 final promotion gate.
- `.planning/PROJECT.md` - v1.3 milestone goal, Jelly baseline, shared semantic graph / solver core keystone, precision-first benchmark posture, and no-public-SDK-promotion discipline.
- `.planning/STATE.md` - current Phase 49 focus and open repo-admin action T-42-04-10 for branch protection leak-gate checks.

### Immediate Upstream Phase Context

- `.planning/phases/45-js-ts-inventory-scope-bindings-module-graph-direct-calls/45-CONTEXT.md` - Primary TS frontend context: inventory, scope/binding, module graph, direct `CopyEdge`/`CallConstraint` emission, `TokenFlowRequired` boundary, Jelly span discipline, and fixture expectations.
- `.planning/phases/47-unified-solver-core-derived-edge-provenance/47-CONTEXT.md` - Solver core, `SolverPolicy`, `PolicyOutcome`, `DerivedEdgeProvenance`, budget semantics, provider/cache/determinism/leak discipline.
- `.planning/phases/48-go-rta-driver/48-CONTEXT.md` - Production multi-policy seam realized by Go RTA, `GoRtaSubBudget` pattern, budget-exceeded honesty, polyglot canary, and no-cross-language-interference proof that Phase 49 must preserve.
- `.planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-CONTEXT.md` - Constraint vocabulary (`CopyEdge`, `CallConstraint`, etc.), semantic graph node/edge identity, composition-over-duplication, provider slot discipline, and determinism inherited by solver phases.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` - Determinism gate, scoring-mode discipline, dense-ID-after-sort rule, and provider digest recipe.
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` - Jelly identity/span rendering, identity-vs-unsupported taxonomy, dedup ordering, and public-surface-leak gate.

### Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` - Primary v1.3 architecture source for Jelly benchmark baselines, shared representation + solver core, JS/TS token propagation as the main Jelly recall lever, and precision-first constraints.
- `research/call-graphs/FINAL-REPORT.md` - JS/TS call graph direction: Oxc callsite/binding facts first, then bounded function-token value flow; unresolved/dynamic facts stay first-class.
- `research/type-alias-points-to/languages/typescript-javascript.md` - TS/JS type/value/alias and function-token precision background.
- `research/evaluation-harness/STANDARD.md` - Native fixture and deterministic observed-output conventions.
- `research/evaluation-harness/decisions/decision-log.md` - Benchmark architecture decisions and hidden/internal-first evaluation posture.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/solver/policy.rs` - `TsTokensPolicy` honest stub to replace, `PolicyOutcome` channel, `GoRtaPolicy` precedent.
- `crates/polint/src/analysis/solver/engine.rs` - `SolverEngine::run_to_solver_output`, `derive_edges`, budget combine, stable normalization, provenance behavior.
- `crates/polint/src/analysis/solver/budget.rs` - `SolverBudget`, `BudgetStatus`, `PointsToSubBudget`, `GoRtaSubBudget`; add the JS/TS sub-budget here.
- `crates/polint/src/analysis/solver/provider.rs` - Production provider registers `GoRtaPolicy` and `TsTokensPolicy`; update policy construction, input digests, diagnostics, and output digest.
- `crates/polint/src/analysis/solver/cache_key.rs` - Add JS token algorithm version and JS sub-budget parts to `solver_provider_parameter_digest`.
- `crates/polint/src/analysis/solver/{facts.rs,store.rs,provenance.rs,validate.rs}` - Derived edge fact/store/provenance/validation patterns.
- `crates/polint/src/analysis/semantic_graph/{constraints.rs,facts.rs,store.rs,provider.rs,build.rs,validate.rs}` - Input vocabulary, semantic node identities, TS constraint production, validation, and stable-key conventions.
- `crates/polint/src/ts/inventory/{facts.rs,extract.rs,store.rs}` - Function/callsite identities and Jelly-shaped spans used as token/callsite inputs.
- `crates/polint/src/ts/scope/{facts.rs,extract.rs,store.rs}` - Lexical binding facts, including existing unsupported reasons that mention function-token phase.
- `crates/polint/src/ts/binding/{facts.rs,direct.rs,store.rs}` - Direct binding facts and `TsDirectBindingReason::TokenFlowRequired` cases Phase 49 should convert where token flow is sufficient.
- `crates/polint/src/analysis/refined_calls/ts_js.rs` - Existing v1.2 TS/JS function-token refinement over type/value/points-to facts. Read for compatibility and later Phase 52 projection awareness; do not let it define Phase 49's solver-owned output contract.
- `crates/polint/src/config/mod.rs` - Existing `[solver.go]` mapping and zero-clamp pattern; add `[solver.js]` here.
- `crates/polint/tests/public_surface_leak.rs` - Public API leak gate; new token types stay unreachable from public SDK/prelude.
- `tests/eval-fixtures/semantic-graph/ts_direct_bindings/` - Phase 45 direct binding fixture to extend or mirror for token-flow inputs.
- `tests/eval-fixtures/jelly/ts-inventory-spans/` - Jelly TS inventory/span fixture; use for recall/identity context.
- `tests/eval-fixtures/polyglot-canary/go-ts/` - Phase 48 canary to update for real TS token policy behavior.
- `tests/eval-fixtures/determinism/ts_reachable/` and `tests/eval-fixtures/determinism/go_rta/` - Determinism fixture precedents.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `TsTokensPolicy` already exists as an honest stub in `analysis::solver::policy`; it has the right trait boundary and tests that should be inverted once Phase 49 lands.
- `SolverEngine::run_to_solver_output` already merges `derive_edges` output and policy-derived edges, then normalizes by stable key. This is the correct production seam.
- `GoRtaPolicy` and `analysis::solver::go_rta` provide the closest implementation model for policy-owned closed inputs, budgeted fixpoint, derived edges, and provenance.
- `SolverBudget` already has a per-domain sub-budget pattern through `GoRtaSubBudget`; copy that shape for JS token caps.
- `config::SolverConfig` already parses `[solver.go]` and overlays positive caps onto defaults. Reuse that pattern for `[solver.js]`.
- Phase 45 TS inventory, scope, and binding modules already expose the identities and unresolved reasons needed to seed token propagation without reparsing source.
- `semantic_graph::constraints` already has the `CopyEdge` and `CallConstraint` vocabulary the token driver should consume.

### Established Patterns

- All v1.3 graph engine internals are crate-private and guarded by public-surface leak tests.
- Stable keys compose existing identities and length-prefixed parts; dense IDs are assigned only after stable-key sorting.
- Budget exhaustion is an explicit `BudgetExceeded` signal, not silent truncation.
- Solver output digests fold provider versions, algorithm-version strings, budget knobs, upstream output digests, row stable keys, statuses, precision, and provenance fragments.
- Native eval fixtures and deterministic observed JSON are the accepted proof style for private analysis families.
- Unsupported/dynamic behavior remains an explicit unknown/unresolved fact unless this phase can prove a real target.

### Integration Points

- `analysis::solver::provider::derive_solver_with_cache_stats` constructs policies and has to build closed TS token inputs before calling `SolverEngine`.
- `analysis_kernel::provider` likely needs provider-digest wiring updates if TS inventory/scope/binding outputs are not already in the solver provider's consumed digest set.
- `AnalysisDb` / internal stores may need token input accessors and possibly replacement paths if the policy emits intermediate private token facts for debug/eval.
- `eval::external::jelly_callgraph` and native eval fixtures are the place to show JS/TS recall delta, but Phase 54 owns final hard gates.
- The polyglot canary fixture must move from "TS policy stub contributes no call_constraint edge" to "TS policy contributes legitimate intra-TS edges while Go and TS remain isolated."

</code_context>

<specifics>
## Specific Ideas

- Token-flow fixtures should include: `const alias = target; alias()`, assignment after declaration, parameter callback (`call(cb)`), higher-order return (`make()()` or `const f = make(); f()`), closure capture returning a local function, and a case that remains unresolved because it needs property/prototype/this modeling.
- Use `"too-many-tokens"` exactly as the sentinel string unless planning finds an existing stable tag convention that is stronger. It should appear only as a sentinel/status/evidence tag, never as a real function identity.
- Suggested config defaults should be generous enough for ordinary repos but low enough for test fixtures to trigger: per-variable token cap, per-callsite candidate cap, max token worklist steps, and optional closure/return depth cap.
- Add a regression that a `[solver.js]` zero cap falls back to the documented default rather than disabling all TS token propagation.
- Preserve Phase 48 Go RTA output byte-for-byte where possible; if the merged solver output necessarily changes because TS policy now contributes edges in mixed fixtures, make those changes fixture-specific and documented.

</specifics>

<deferred>
## Deferred Ideas

- JS/TS object/property/prototype/class/`this` modeling remains Phase 50.
- Adaptation-model facts and validated native/framework models remain Phase 51.
- Refined calls projection over solver output and unknown-taxonomy consolidation remain Phase 52.
- Milestone-wide cache and solver-budget consolidation remains Phase 53.
- Hard benchmark promotion gates and public API leak CI gate promotion remain Phase 54.
- Public SDK views over solver output are out of v1.3 and deferred to v1.4+.

</deferred>

---

*Phase: 49-JS/TS Function-Token Propagation Driver*
*Context gathered: 2026-06-03*
