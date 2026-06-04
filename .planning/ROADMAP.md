# Roadmap: polint

## Milestones

- [x] **v1.0 MVP** - repo-local static analysis framework for Go and TypeScript/JavaScript, shipped 2026-05-02. Archive: [v1.0 roadmap](milestones/v1.0-ROADMAP.md).
- [x] **v1.1 Capability Fulfillment** - capability planning, resolved imports/module graph, and symbol/reference foundations for Go and TS/JS.
- [x] **v1.2 Static Analysis Engine Implementation** - private, validated, cache-aware, agent-extensible analysis engine substrate; 22 phases and 136 plans shipped 2026-05-27. Archive: [v1.2 roadmap](milestones/v1.2-ROADMAP.md).
- [ ] **v1.3 Graph Engine Precision** - shared semantic graph + unified call solver; raises Go RTA and Jelly recall from <3% to >25-30% while holding precision floors. 13 phases (42-54) starting 2026-05-27.

## Current Status

**Milestone:** v1.3 Graph Engine Precision (active)
**Phases planned:** 13 (Phase 42 - Phase 54)
**Requirements coverage:** 27/27 mapped
**Granularity:** fine

Phase numbering continues from v1.2's last phase 41. Phases 45/46 may run in parallel; phases 48/49 may run in parallel. All new analysis modules stay `pub(crate)` — v1.2 promotion discipline applies; no public SDK promotion in v1.3.

## Phases (v1.3)

- [x] **Phase 42: Benchmark Identity, Renderers, Dedup & Identity Taxonomy** - Stable identity records, Go `RelString` and Jelly span renderers, identity-vs-unsupported categorization, public-surface-leak CI gate. ✅ Verified 5/5 (full Go module import-path RelString deferred to Phase 46; broad Jelly coverage to Phase 45).
- [x] **Phase 43: Reachability, Roots & Per-Suite Scoring Mode** - Explicit roots from v1.2 entrypoints, per-suite scoring mode, determinism gate (10-shuffle byte-identical observed JSON).
- [x] **Phase 44: Semantic Graph Skeleton & Constraint Vocabulary** - Private `analysis::semantic_graph` with typed nodes/edges/indexes/cache key; constraint enum (`CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`).
- [x] **Phase 45: JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls** - Oxc-backed exact-span function/callsite enumeration, lexical scopes, ESM/CJS/tsconfig module graph, direct call emission as constraints. ✅ Verified 5/5. May run in parallel with Phase 46.
- [ ] **Phase 46: Go Semantic Frontend & Sidecar** - `polint-go-frontend` Go binary (`go/packages` + `go/ssa`), NDJSON protocol, sidecar client, lowering to semantic graph; typed process boundary, version pinning, failure taxonomy. May run in parallel with Phase 45.
- [ ] **Phase 47: Unified Solver Core & Derived-Edge Provenance** - Private `analysis::solver` (deterministic `VecDeque` worklist, `SolverBudget`, `BudgetStatus`, per-language `SolverPolicy` scaffolding); folds in `points_to` as a sub-domain; `DerivedEdgeProvenance` contract.
- [x] **Phase 48: Go RTA Driver** - Private `analysis::solver::go_rta` (reachable functions from roots, address-taken tracking, dynamic dispatch by signature, interface invoke by method-set, fixed-point iteration). May run in parallel with Phase 49.
- [x] **Phase 49: JS/TS Function-Token Propagation Driver** - Private `analysis::solver::ts_tokens` with per-variable token caps, `"too-many-tokens"` sentinel, `BudgetExceeded` reporting. May run in parallel with Phase 48.
- [ ] **Phase 50: JS/TS Object/Property/Prototype/`this` Model & Driver** - Private `src/ts/object_model/` + `analysis::solver::ts_object_model` (allocation-site abstraction, bounded property buckets with computed-property handling, prototype-walk termination, `this` binding rules).
- [ ] **Phase 51: Adaptation Model Layer** - Private `analysis::adaptation` (TOML schema, loader, validator confirming target symbols exist, `ModelEdge` emission); `benchmark adapted` mode with prompt hash, accepted/rejected facts, deltas, held-out subset reporting, sandboxed agent.
- [ ] **Phase 52: Refined-Calls Rework & Unknown Taxonomy Consolidation** - `refined_calls::provider` projects over solver output preserving v1.2 `RefinedCallEdgeFact` contract; consolidated taxonomy via `polint inspect unknowns --format json`.
- [ ] **Phase 53: Cache & Solver Budgets Consolidation** - Cache keys digest sidecar binary, Go toolchain, adaptation model files, and solver budgets across every new family; solver budgets enforce token-set/property/fanout/model/package-depth caps with `BudgetExceeded` as facts.
- [ ] **Phase 54: Benchmark Promotion Gate Extension** - Per-suite precision floors (Go ≥60%, Jelly configurable), F-score β=0.5 alongside F1, per-language deltas, polyglot Go+TS canary, public-API leak CI gate (no v1.3 solver types in `polint::sdk::prelude::*`).

## Phase Details

### Phase 42: Benchmark Identity, Renderers, Dedup & Identity Taxonomy
**Goal**: polint can render benchmark-grade identity for every function and callsite, dedupe by semantic identity, and distinguish identity-vs-unsupported categories so every downstream metric becomes trustworthy.
**Depends on**: v1.2 substrate (`FactMeta`, `analysis::ids`, `analysis::calls`)
**Requirements**: IDENT-01, IDENT-02, IDENT-03
**Success Criteria** (what must be TRUE):
  1. polint emits a stable identity record `(file, span, language, package/module, container, display, signature digest)` for every function and callsite, deduplicated before scoring (verified by snapshot fixtures).
  2. Per-benchmark renderers produce Go `RelString`-style names and Jelly `file:start_line:start_col:end_line:end_col` spans with ≥99% Jelly oracle-span coverage on micro fixtures across Linux + macOS CI.
  3. CRLF/LF normalization fixture passes and produces byte-identical renderer output.
  4. Evaluation output reports distinct categories `wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing`.
  5. Public-surface-leak CI gate is installed: external rule crate compiles against `polint::sdk::prelude::*` and reaches zero v1.3 solver types. ✅ Addressed by Plan 04 (leak-gate job on Linux + macOS; locked ALLOWED_PRELUDE = 97 entries).
**Plans**: 5 total — 01 (identity substrate) ✅, 02 (renderers) ✅, 03 (identity taxonomy) ✅, 04 (public-surface-leak CI gate) ✅, 05 (gap closure: Go package-name qualification + dedup total-order determinism) ✅

### Phase 43: Reachability, Roots & Per-Suite Scoring Mode
**Goal**: polint discovers explicit reachability roots from the v1.2 entrypoint substrate, scores each benchmark suite in the mode its oracle expects, and inherits a determinism gate every subsequent solver phase must pass.
**Depends on**: Phase 42 (identity); v1.2 entrypoints
**Requirements**: REACH-01, REACH-02, REACH-03
**Success Criteria** (what must be TRUE):
  1. Reachability roots (`main`, `init`, exported, tests, configured repo entrypoints) are discoverable as typed facts derived from the v1.2 entrypoint substrate.
  2. Each suite manifest declares a `scoring_mode` (`oracle-rta`, `oracle-jelly`, `whole-repo`) and the gate fails if it is missing; unreachable direct calls remain facts but are marked outside the reachable graph.
  3. Determinism gate fixture passes: 10 shuffled provider-order runs produce byte-identical observed JSON, identical solver step counts, and identical budget-exceeded reasons.
  4. The determinism gate is wired so every subsequent solver-introducing phase inherits it as an acceptance gate.
**Plans**: 3 total
- [x] 43-01-PLAN.md — analysis::reachability module + ReachabilityRootFact/RootKind + root discovery (Go main/init/exported, entrypoint bridge, configured roots) + polint.reachability provider/cache + kernel splice (REACH-01)
- [x] 43-02-PLAN.md — required scoring_mode field + 4 suite manifest updates + reachable-set BFS/DFS + CallReachabilityFact marking + mode-aware scoring filter (REACH-02)
- [x] 43-03-PLAN.md — reserved solver_step_count/budget_exceeded_reasons section + N=10 determinism-gate harness + Go/TS fixtures + fast-CI Linux+macOS job + inheritance contract (REACH-03)

### Phase 44: Semantic Graph Skeleton & Constraint Vocabulary
**Goal**: polint has a private shared semantic graph with stable identities, typed edges, and a closed constraint vocabulary that language frontends emit into — the architectural keystone for the unified solver.
**Depends on**: Phase 42 (identity), Phase 43 (reachability)
**Requirements**: GRAPH-01, GRAPH-02
**Success Criteria** (what must be TRUE):
  1. Private `analysis::semantic_graph` exists with typed `NodeKind` (function, callsite, scope, place, abstract object, module, package) and `EdgeKind` (call, member-of, alloc, flow), outgoing/incoming/by-kind indexes, validation, provider manifest, and cache key.
  2. Constraint vocabulary is defined as a closed enum (`CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`) with snapshot fixtures asserting language frontends emit the expected shapes.
  3. Dependency index for the shared-graph cache layer is designed and lists every contributing input (semantic index, module graph, MIR, CFG, direct calls, types, summaries, entrypoints, extensions, accepted adaptation models, solver budgets).
  4. Public-boundary proof: `analysis::semantic_graph` and the constraint enum stay `pub(crate)`, never reachable from `polint::sdk::prelude::*`.
**Plans**: 3 total
- [x] 44-01-PLAN.md — analysis::semantic_graph module + NodeKind/EdgeKind closed enums (composing existing v1.2 IDs) + node/edge facts + SemanticNodeId/SemanticEdgeId + stable keys + SemanticGraphStore indexes (GRAPH-01)
- [x] 44-02-PLAN.md — ConstraintKind closed vocabulary + ConstraintFact family + SemanticConstraintId + build_semantic_graph emission from existing facts + ModelEdge reserved-empty + points-to naming-collision guard (GRAPH-02)
- [x] 44-03-PLAN.md — polint.semantic_graph provider + cache key + validation + kernel order/run/validation splice + Go/TS snapshot fixtures + determinism-gate inheritance + public-surface-leak proof (GRAPH-01, GRAPH-02)

### Phase 45: JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls
**Goal**: polint enumerates every JS/TS function and callsite with Jelly-shaped spans, builds proper lexical scopes and a module graph, and emits direct bindings as constraints — the JS/TS foundation for the token solver.
**Depends on**: Phase 42 (identity), Phase 44 (semantic graph skeleton)
**Requirements**: JS-01, JS-02, JS-03
**Success Criteria** (what must be TRUE):
  1. polint enumerates JS/TS functions (declarations, expressions, arrows, methods, constructors, accessors, class static blocks) and callsites (calls, `new`, tagged templates, optional calls, dynamic import, require) with Jelly-shaped spans matching ≥99% of Jelly fixture oracle spans.
  2. Lexical scopes (`var`, `let`, `const`, functions, classes, imports, destructuring, parameters, catch, re-exports) and a module graph covering ESM, CommonJS, and TypeScript path aliases are built and stored as private facts.
  3. JS/TS direct call bindings (`f()`, `ns.f()`, imported aliases, local aliases) emit `CopyEdge` + `CallConstraint` constraints into the semantic graph (verified by snapshot fixtures).
  4. All new JS/TS modules (`src/ts/inventory/`, `src/ts/scope/`) stay `pub(crate)` and pass the public-surface-leak gate.
**Plans**: 5 total
- [x] 45-01-PLAN.md — private JS/TS inventory fact model, Oxc function/callsite extraction, normalized inventory output (JS-01)
- [x] 45-02-PLAN.md — private JS/TS scope/binding facts, Oxc semantic extraction, scope store indexes, unresolved dynamic boundary rows (JS-02)
- [x] 45-03-PLAN.md — private TS direct binding facts, local/import/module-mediated direct bindings, normalized binding store/cache contract (JS-02, JS-03 foundation)
- [x] 45-04-PLAN.md — project TS direct bindings into semantic graph `CopyEdge` and `CallConstraint` rows (JS-03)
- [x] 45-05-PLAN.md — close Phase 45 with Jelly, module/binding, cache/determinism, and public-surface fixtures (JS-01, JS-02, JS-03)
**UI hint**: no

May run in parallel with Phase 46 (shares no Rust modules).

### Phase 46: Go Semantic Frontend & Sidecar
**Goal**: polint runs a co-shipped Go sidecar backed by `go/packages` + `go/ssa` over a typed NDJSON protocol with version pinning, timeouts, and clean orphan handling, lowering Go semantic facts into the shared semantic graph.
**Depends on**: Phase 42 (identity), Phase 44 (semantic graph skeleton)
**Requirements**: GO-01, GO-02, GO-03, GO-04
**Success Criteria** (what must be TRUE):
  1. `polint-go-frontend` Go sidecar binary uses `go/packages` + `go/ssa` + `golang.org/x/tools v0.45.0` and emits NDJSON facts (functions, methods, receiver types, init, method sets, call sites, types) over stdio with a versioned schema.
  2. `src/go/semantic/` (sidecar client + lowering) maps NDJSON facts to semantic-graph constraints with stable identities and exact source spans.
  3. Process boundary is hardened: typed protocol with explicit terminators, per-request timeouts, cancellation propagation, a single long-lived sidecar per `polint check`, and SIGTERM-cleanup fixture asserts no surviving Go processes after 5 seconds.
  4. `GoPackagesLoadFailed`, `GoVersionUnsupported`, and `GoSidecarTimeout` appear as distinct categories in the unsupported/unknown taxonomy; the sidecar binary digest + Go toolchain version participate in cache keys.
**Plans**: TBD

May run in parallel with Phase 45 (shares no Rust modules; Go sidecar workstream is independent).

### Phase 47: Unified Solver Core & Derived-Edge Provenance
**Goal**: polint has a single private deterministic solver consuming the constraint vocabulary, with explicit budgets, per-language policy scaffolding, and full provenance on every derived edge — the heart of v1.3.
**Depends on**: Phase 44 (constraint vocabulary), Phase 45 (TS constraints) or Phase 46 (Go constraints) — at least one frontend emitting constraints
**Requirements**: GRAPH-03, GRAPH-04
**Success Criteria** (what must be TRUE):
  1. Private `analysis::solver` exists with deterministic `VecDeque` worklist, explicit `SolverBudget` and `BudgetStatus`, and per-language `SolverPolicy` trait scaffolding; v1.2's `points_to::solver` is folded in as a sub-domain.
  2. Solver inherits the determinism gate from Phase 43 — 10-shuffle byte-identical observed JSON passes.
  3. Every solver-derived edge carries `DerivedEdgeProvenance` (contributing fact IDs totally ordered by stable ID, constraint kind, solver step) consumable by `polint explain`; property test asserts deletion of any contributing fact invalidates the derived edge.
  4. Dependency contract is documented (closed input set, single-fixpoint-per-run, bounded outer iterations) and a cycle-detection fixture proves no solver↔summary loop is admitted.
  5. All solver types stay `pub(crate)` and the public-surface-leak gate continues to pass.
**Plans**: 3 total
- [x] 47-01-PLAN.md — analysis::solver core: VecDeque worklist engine + SolverBudget/BudgetStatus + SolverPolicy trait (points-to fold as first real impl + Go/TS honest stubs) (GRAPH-03)
- [x] 47-02-PLAN.md — DerivedEdgeProvenance (contributing facts total-ordered by stable ID + constraint kind + solver step) + derived-edge fact family/store + polint explain consumption + deletion property test (GRAPH-04)
- [x] 47-03-PLAN.md — polint.solver provider/cache-key/validate wiring + dependency-contract doc + cycle-detection fixture + ~7 provider-order snapshot updates + determinism-gate + public-surface-leak proof (GRAPH-03, GRAPH-04)

### Phase 48: Go RTA Driver
**Goal**: polint resolves Go interface calls and dynamic dispatch through a hand-rolled RTA driver over the unified solver, lifting Go x/tools RTA recall toward the 70-90% algorithmic ceiling while holding precision.
**Depends on**: Phase 43 (reachability), Phase 46 (Go semantic frontend), Phase 47 (solver core)
**Requirements**: GO-05
**Success Criteria** (what must be TRUE):
  1. Private `analysis::solver::go_rta` implements reachability fixpoint from roots, address-taken function tracking, dynamic call sites by signature, runtime types through interfaces, interface invoke by method-set, and fixed-point iteration.
  2. Iteration cap fixture demonstrates `BudgetExceeded` is emitted for runaway dispatch rather than silently dropped.
  3. Per-language `solver_config.go.*` knobs (e.g., address-taken threshold) exist and a polyglot Go+TS canary fixture exercises cross-language non-regression.
  4. Native fixture coverage proves RTA produces benchmark-grade edges on Go x/tools testdata and the determinism gate still passes.
**Plans**: 3 total
- [x] 48-01-PLAN.md — Go-frontend RTA-signal emission: sidecar harvests *ssa.MakeInterface instantiated types + *ssa.MakeClosure/func-value address-taken + dynamic-callsite dispatch detail; new crate-private GoSemantic* facts lowered/stored/validated/cache-keyed (GO-05)
- [x] 48-02-PLAN.md — analysis::solver::go_rta RTA fixpoint policy (reachability ⊗ instantiated-types ⊗ dispatch) + SolverEngine production routing + PolicyOutcome derived-edge channel + GoRtaSubBudget + [solver] config table + cache-key; points-to byte-identical (GO-05)
- [x] 48-03-PLAN.md — verification: iteration-cap BudgetExceeded fixture + interface-dispatch/address-taken native fixtures + polyglot Go+TS canary + go_rta determinism fixture; determinism + public-surface-leak gates stay green (GO-05)

May run in parallel with Phase 49 (drivers share the solver but their iteration logic is independent).

### Phase 49: JS/TS Function-Token Propagation Driver
**Goal**: polint propagates JS/TS function tokens through assignments, parameters, returns, and closures inside the unified solver — the main Jelly recall lever — with strict per-variable budgets to prevent memory blowup.
**Depends on**: Phase 45 (JS/TS scope + direct binding), Phase 47 (solver core)
**Requirements**: JS-04
**Success Criteria** (what must be TRUE):
  1. Private `analysis::solver::ts_tokens` propagates tokens through `CopyEdge` and call/return constraints with a per-variable token cap.
  2. When the cap is exceeded, the solver collapses to a `"too-many-tokens"` sentinel and emits `BudgetExceeded` consumed by the unknown taxonomy rather than silently dropping precision.
  3. Memory-ceiling fixture proves RSS stays bounded on token-explosion inputs (uses `BitSet`/`roaring::RoaringBitmap` indexed by stable function ID).
  4. Per-language `solver_config.js.*` knobs (e.g., function-expression inclusion) exist; aggregate metrics on Jelly fixtures show recall improvement without precision regression beyond the Phase 54 floor.
**Plans**:
- [x] 49-01-PLAN.md — JS token budget/config/cache substrate and `TokenFlowRequired` handoff classifier.
- [x] 49-02-PLAN.md — private `analysis::solver::ts_tokens` closed inputs, deterministic token fixpoint, `"too-many-tokens"` sentinel, token-derived `DerivedEdgeFact` dispatch, and real `TsTokensPolicy`.
- [x] 49-03-PLAN.md — native TS token fixtures, token-explosion budget proof, polyglot/determinism/Jelly evidence, leak gate, and full-suite sweep.

May run in parallel with Phase 48 (drivers share the solver core but their iteration logic is independent).

### Phase 50: JS/TS Object/Property/Prototype/`this` Model & Driver
**Goal**: polint models JS/TS allocation sites, property reads/writes, prototype chains, classes, and `this` binding inside the unified solver — the highest precision/cost tradeoff in v1.3 — behind a capability flag until gates approve.
**Depends on**: Phase 49 (token propagation stable), Phase 45 (JS/TS inventory)
**Requirements**: JS-05
**Success Criteria** (what must be TRUE):
  1. Private `src/ts/object_model/` + `analysis::solver::ts_object_model` implement allocation-site abstraction, bounded property buckets with computed-property handling, prototype-walk termination, and `this` resolution for arrow, method, constructor, bound, `call`, and `apply` forms.
  2. Per-family budgets cap property buckets and receiver-set fanout; budget exhaustion appears as facts in the unknown taxonomy, not as silent precision drops.
  3. Native fixtures cover prototype-walk termination, arrow-vs-method `this`, and computed-property collapse; determinism gate continues to pass with the object model enabled.
  4. The model ships behind a capability flag and Jelly benchmark deltas are recorded against both `oracle-jelly` and `whole-repo` scoring modes.
**Plans**:
- [x] 50-01-PLAN.md — private TS object-model facts, deterministic storage, and semantic graph lowering for object/property/receiver/class facts.
- [x] 50-02-PLAN.md — object-model opt-in flag, per-family object budgets, config mapping, and solver digest participation.
- [ ] 50-03-PLAN.md — private `analysis::solver::ts_object_model` property-bucket fixpoint and property-backed derived edge dispatch.
- [ ] 50-04-PLAN.md — bounded prototype/class/accessor lookup plus `this`/receiver binding for methods, arrows, constructors, bound, `call`, and `apply`.
- [ ] 50-05-PLAN.md — native object-model fixtures, budget/determinism/polyglot/Jelly evidence, leak gate, full regression, and roadmap closeout.

### Phase 51: Adaptation Model Layer
**Goal**: polint accepts repo-local validated framework/native model facts as solver constraints, with sandboxed agent runs, accept/reject reporting, and held-out validation that prevents oracle-label leakage and recall flooding.
**Depends on**: Phase 47 (solver functional), Phase 48 or Phase 49 (at least one driver functional)
**Requirements**: ADAPT-01, ADAPT-02
**Success Criteria** (what must be TRUE):
  1. Private `analysis::adaptation/` exists with a TOML model schema (source pattern, target pattern, confidence, language, scope, evidence), a loader, and a validator that rejects facts whose targets do not resolve in the semantic graph.
  2. `benchmark adapted` mode reports prompt hash, changed model files, accepted/rejected facts, unknown delta, precision/recall delta, runtime/cache delta, and held-out subset deltas.
  3. The adaptation agent runs in a sandbox directory that cannot read benchmark oracle files (`research/evaluation-harness/repos/*/expected*`, `research/evaluation-harness/suites/*.toml`); prompt-sanitizer fixture asserts no oracle paths leak in.
  4. Validator rejects model facts whose RHS exactly matches oracle expectations and wildcard/broad-pattern models; `ModelEdge` constraints are emitted only for accepted facts.
**Plans**: TBD

### Phase 52: Refined-Calls Rework & Unknown Taxonomy Consolidation
**Goal**: polint retires v1.2's heuristic refined-call refiners in favor of a thin projection over solver output, preserves the public `RefinedCallEdgeFact` contract unchanged, and exposes a consolidated unsupported/unknown diagnostic queue.
**Depends on**: Phase 47-50 (solver + drivers functional), Phase 46 (Go sidecar failure modes)
**Requirements**: GRAPH-05, TAX-01
**Success Criteria** (what must be TRUE):
  1. `refined_calls::provider` is reworked to project over solver output and preserves the v1.2 `RefinedCallEdgeFact` contract for downstream `data_flow`/`evidence`/SDK views without contract changes (verified by integration tests against v1.2 fixtures).
  2. Private `analysis::unknown_taxonomy` consolidates categories across providers: `SetupMissing`, `UnsupportedSemantic`, `MissingFact`, `OutOfScope`, plus sidecar-specific `GoPackagesLoadFailed`, `GoVersionUnsupported`, `GoSidecarTimeout`.
  3. `polint inspect unknowns --format json` is added as a public CLI surface (the only new public CLI surface in v1.3) and returns the consolidated taxonomy with stable JSON.
  4. v1.2 heuristic refiners are removed; downstream data-flow/evidence fixtures continue to pass byte-identical or with explicitly-documented improvements.
**Plans**: TBD

### Phase 53: Cache & Solver Budgets Consolidation
**Goal**: polint threads cache key participation and solver budgets uniformly across every new v1.3 fact family, with positive (must-invalidate) and negative (must-preserve-hit) fixtures proving correct cross-family invalidation.
**Depends on**: All earlier v1.3 phases (consolidation sweep)
**Requirements**: CACHE-01, CACHE-02
**Success Criteria** (what must be TRUE):
  1. Every new v1.3 fact family (semantic graph, solver, RTA driver, token driver, object model, adaptation) declares a dependency index and cache-key digest that includes the sidecar binary digest, Go toolchain version, adaptation model files, and solver budgets.
  2. Single-input-mutation fixtures prove each upstream change invalidates the right downstream layer (must-invalidate); negative fixtures prove no-op changes preserve cache hits (must-preserve-hit).
  3. Solver budgets are enforced across token-set size, property abstraction, dynamic-call fanout, model expansion, and package depth; budget exhaustion surfaces as facts (`BudgetExceeded`) rather than silent precision drops.
  4. Cold/warm RSS thresholds appear as required columns in the benchmark report.
**Plans**: TBD

### Phase 54: Benchmark Promotion Gate Extension
**Goal**: polint enforces v1.3's exit gates — per-language precision floors, F-score β=0.5 tracking, per-language deltas, polyglot canary, and a public-API leak CI gate — to prove the milestone delivers benchmark-grade precision/recall without leaking solver internals.
**Depends on**: All earlier v1.3 phases (final exit gate)
**Requirements**: BENCH-01
**Success Criteria** (what must be TRUE):
  1. Promotion gate enforces hard per-suite precision floors (Go ≥60%, Jelly configurable) and rejects "flooding" synthetic fixtures regardless of recall improvement.
  2. F-score β=0.5 (precision-weighted) is tracked alongside F1 in promotion reports; per-language deltas are reported and enforced separately.
  3. Polyglot Go+TS canary fixture is included in the gate and runs on every solver change.
  4. Public-API leak CI gate asserts no v1.3 solver types are reachable from `polint::sdk::prelude::*`; the gate fails the build if a `pub` slips in by accident.
  5. v1.3 milestone audit records final Go and Jelly recall numbers against baseline (`<3%` → `>25-30%` target) with precision floors held.
**Plans**: TBD

## Phase Progress

| Phase | Name | Plans Complete | Status | Completed |
|-------|------|----------------|--------|-----------|
| 42 | Benchmark Identity, Renderers, Dedup & Identity Taxonomy | 5/5 | Complete   | 2026-05-29 |
| 43 | Reachability, Roots & Per-Suite Scoring Mode | 3/3 | Complete    | 2026-05-29 |
| 44 | Semantic Graph Skeleton & Constraint Vocabulary | 3/3 | Complete    | 2026-05-30 |
| 45 | JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls | 5/5 | Complete    | 2026-05-31 |
| 46 | Go Semantic Frontend & Sidecar | 4/4 | Complete    | 2026-06-01 |
| 47 | Unified Solver Core & Derived-Edge Provenance | 3/3 | Complete    | 2026-06-02 |
| 48 | Go RTA Driver | 3/3 | Complete    | 2026-06-02 |
| 49 | JS/TS Function-Token Propagation Driver | 3/3 | Complete | 2026-06-03 |
| 50 | JS/TS Object/Property/Prototype/`this` Model & Driver | 2/5 | In Progress|  |
| 51 | Adaptation Model Layer | 0/0 | Not started | - |
| 52 | Refined-Calls Rework & Unknown Taxonomy Consolidation | 0/0 | Not started | - |
| 53 | Cache & Solver Budgets Consolidation | 0/0 | Not started | - |
| 54 | Benchmark Promotion Gate Extension | 0/0 | Not started | - |

## Parallel-Eligible Phases

- **Phase 45 ↔ Phase 46** — JS/TS inventory/scope/bindings and Go semantic frontend/sidecar share no Rust modules; can run concurrently after Phase 44 lands.
- **Phase 48 ↔ Phase 49** — Go RTA driver and JS/TS function-token propagation driver share the solver core but their iteration logic and fixtures are independent; can run concurrently after Phase 47 lands.

## Promotion Discipline (Inherited from v1.2)

- Every new v1.3 module is `pub(crate)`. No public SDK type promotion in v1.3.
- The only new public CLI surface in v1.3 is `polint inspect unknowns --format json` (Phase 52).
- Public-API leak CI gate (Phase 42 installs, Phase 54 enforces) ensures no v1.3 solver type is reachable from `polint::sdk::prelude::*`.
- Future SDK promotion of `Reachability<'_>`, `CallGraph<'_>`, or `Adaptation<'_>` requires two-milestone benchmark stability and a separate explicit phase (deferred to v1.4+).

## Archived Phase Summary

<details>
<summary>v1.2 Static Analysis Engine Implementation (Phases 20-41) - shipped 2026-05-27</summary>

| Phase | Name | Plans | Completed |
|-------|------|-------|-----------|
| 20 | Private Analysis Kernel Facade | 2/2 | 2026-05-16 |
| 21 | Provenance, Precision, and Validation Metadata | 4/4 | 2026-05-17 |
| 22 | Internal Evaluation Harness MVP | 6/6 | 2026-05-17 |
| 23 | Input Snapshots and Cache-Key Vocabulary | 5/5 | 2026-05-18 |
| 24 | Persistent Layer Cache for Existing Cheap Facts | 5/5 | 2026-05-18 |
| 25 | Rule Manifest, Inspect, and Test Skeleton | 4/4 | 2026-05-18 |
| 26 | Semantic Index Deepening | 6/6 | 2026-05-19 |
| 27 | Layered Module/Package/Topology Graph | 7/7 | 2026-05-19 |
| 28 | Private Semantic MIR and Place Identity | 7/7 | 2026-05-20 |
| 29 | Local CFG and Control Dependence | 6/6 | 2026-05-20 |
| 30 | Direct Call Facts | 8/8 | 2026-05-21 |
| 31 | P0 Abstract-Domain Kernel | 5/5 | 2026-05-21 |
| 32 | Summary Kernel and Direct Summaries | 7/7 | 2026-05-21 |
| 33 | Demand Queries and Summary SCC Cache | 7/7 | 2026-05-24 |
| 34 | Rust Extension/Provider Sink | 6/6 | 2026-05-23 |
| 35 | Framework Entrypoints and Trust Boundaries | 8/8 | 2026-05-24 |
| 36 | P0 Type/Value/Place/Alias Substrate | 7/7 | 2026-05-24 |
| 37 | Refined Call Graph Providers | 6/6 | 2026-05-25 |
| 38 | Local Plus Summary-Projected Data Flow | 10/10 | 2026-05-25 |
| 39 | Slicing, Paths, and Evidence Bundles | 7/7 | 2026-05-25 |
| 40 | External Benchmark Adapters and Promotion Gates | 8/8 | 2026-05-26 |
| 41 | Public SDK Query Views and Agent Ergonomics | 5/5 | 2026-05-26 |

</details>
