# Requirements: polint v1.3 Graph Engine Precision

**Defined:** 2026-05-27
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Milestone goal:** Turn v1.2's isolated semantic facts (MIR, CFG, calls, types, points-to, data flow) into a unified semantic graph and solver core, and use it to raise Go x/tools RTA and Jelly JS/TS call-graph benchmark recall from <3% to >25-30% while keeping precision a first-class target.

**Source research:** `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md`, `.planning/research/SUMMARY.md` (architecture, features, stack, pitfalls).

## v1 Requirements

Requirements for the v1.3 milestone. Each maps to exactly one roadmap phase.

### Identity & Reachability

- [ ] **IDENT-01**: polint emits stable internal identity records `(file, span, language, package/module, container, display, signature digest)` for every function and callsite, deduplicated by semantic identity before scoring.
- [ ] **IDENT-02**: polint provides per-benchmark identity renderers — Go `RelString`-style function/method names and Jelly `file:start_line:start_col:end_line:end_col` callsite/function spans — with ≥99% Jelly oracle-span coverage on micro fixtures and CRLF/LF normalization.
- [ ] **IDENT-03**: polint reports identity-vs-unsupported categories distinctly (`wrong_identity`, `unsupported_edge`, `unresolved_edge`, `package_load_limitation`, `model_missing`) in evaluation output.
- [ ] **REACH-01**: polint discovers explicit reachability roots from the v1.2 entrypoint substrate (`main`, `init`, exported, tests, configured repo entrypoints) and exposes them as typed facts.
- [ ] **REACH-02**: polint scores benchmark suites in the mode each oracle expects via a `scoring_mode` field (`oracle-rta`, `oracle-jelly`, `whole-repo`) on suite manifests; unreachable direct calls remain facts but are marked outside the reachable graph.
- [ ] **REACH-03**: polint enforces a determinism gate (10 shuffled provider-order runs produce byte-identical observed JSON) before any solver phase lands and the gate is inherited by every subsequent solver phase.

### Shared Semantic Graph & Unified Solver

- [ ] **GRAPH-01**: polint has a private `analysis::semantic_graph` with typed `NodeKind` (function, callsite, scope, place, abstract object, module, package) and `EdgeKind` (call, member-of, alloc, flow), indexes, validation, provider manifest, and cache key.
- [ ] **GRAPH-02**: polint defines a constraint vocabulary (`CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`) that language frontends emit into the semantic graph; constraint emission is verified by snapshot fixtures.
- [ ] **GRAPH-03**: polint has a private unified `analysis::solver` with deterministic `VecDeque` worklist, explicit `SolverBudget` / `BudgetStatus`, per-language `SolverPolicy` trait scaffolding, and folds v1.2's `points_to::solver` in as a sub-domain.
- [ ] **GRAPH-04**: every solver-derived edge carries `DerivedEdgeProvenance` (contributing fact IDs, constraint kind, solver step) consumable by `polint explain`.
- [ ] **GRAPH-05**: `refined_calls::provider` is reworked to project over solver output and preserves the v1.2 `RefinedCallEdgeFact` contract for downstream `data_flow`/`evidence`/SDK views without contract changes.

### Go Critical Path

- [ ] **GO-01**: polint ships a `polint-go-frontend` Go sidecar binary that uses `go/packages` + `go/ssa` + `golang.org/x/tools v0.45.0` and emits NDJSON facts (functions, methods, receiver types, init, method sets, call sites, types) over stdio with a versioned schema.
- [ ] **GO-02**: polint has `src/go/semantic/` with a sidecar client and lowering layer that maps NDJSON facts to semantic-graph constraints with stable identities and exact source spans.
- [ ] **GO-03**: the sidecar process boundary enforces typed protocol with explicit terminators, per-request timeouts, cancellation propagation, a single long-lived sidecar per `polint check`, and orphan-process cleanup verified by a SIGTERM fixture (no surviving Go processes after 5 seconds).
- [ ] **GO-04**: polint distinguishes `GoPackagesLoadFailed`, `GoVersionUnsupported`, and `GoSidecarTimeout` in the unsupported/unknown taxonomy and includes the sidecar binary digest + Go toolchain version in cache keys.
- [ ] **GO-05**: polint has a private Go RTA driver in `analysis::solver::go_rta` (reachable functions from roots, address-taken function tracking, dynamic call sites by signature, runtime types through interfaces, interface invoke by method-set, fixed-point iteration).

### JS/TS Critical Path

- [ ] **JS-01**: polint enumerates JS/TS functions (declarations, expressions, arrows, methods, constructors, accessors, class static blocks) and callsites (calls, `new`, tagged templates, optional calls, dynamic import, require) with exact Jelly-shaped spans matching ≥99% of Jelly fixture oracle spans.
- [ ] **JS-02**: polint builds proper lexical scopes (`var`, `let`, `const`, functions, classes, imports, destructuring, parameters, catch, re-exports) and a module graph covering ESM, CommonJS, and TypeScript path aliases.
- [ ] **JS-03**: polint emits JS/TS direct call bindings (`f()`, `ns.f()`, imported aliases, local aliases) as `CopyEdge` + `CallConstraint` constraints into the semantic graph.
- [ ] **JS-04**: polint has a private JS/TS function-token propagation driver in `analysis::solver::ts_tokens` with per-variable token caps, a `"too-many-tokens"` sentinel, and `BudgetExceeded` reporting consumed by the unknown taxonomy.
- [ ] **JS-05**: polint has a private JS/TS object/property/prototype/class/`this` model in `src/ts/object_model/` + `analysis::solver::ts_object_model` (allocation-site abstraction, bounded property buckets with computed-property handling, prototype-walk termination, `this` for arrow/method/constructor/bound/`call`/`apply`).

### Adaptation, Cache, Budgets, Taxonomy

- [ ] **ADAPT-01**: polint has a private `analysis::adaptation/` with a TOML model schema (source pattern, target pattern, confidence, language, scope, evidence), a loader, and a validator that confirms target symbols exist in the semantic graph before accepting facts.
- [ ] **ADAPT-02**: `benchmark adapted` mode reports prompt hash, changed model files, accepted/rejected facts, unknown delta, precision/recall delta, runtime/cache delta, and held-out subset deltas; the adaptation agent runs in a sandbox that cannot read benchmark oracle files.
- [ ] **CACHE-01**: every new v1.3 fact family has a dependency index and cache key participating digests for the sidecar binary, Go toolchain version, adaptation model files, and solver budgets, verified by both must-invalidate and must-preserve-hit fixtures.
- [ ] **CACHE-02**: polint enforces solver budgets across token-set size, property abstraction, dynamic-call fanout, model expansion, and package depth, with budget exhaustion surfaced as facts rather than silent precision drops.
- [ ] **TAX-01**: polint consolidates the unsupported/unknown taxonomy across providers (`SetupMissing`, `UnsupportedSemantic`, `MissingFact`, `OutOfScope`, plus sidecar-specific failure modes) and exposes it via `polint inspect unknowns --format json`.

### Benchmark Promotion

- [ ] **BENCH-01**: the benchmark promotion gate enforces hard per-suite precision floors (Go ≥60%, Jelly configurable), tracks F-score β=0.5 alongside F1, enforces per-language deltas separately, includes a polyglot Go+TS canary fixture, and asserts a public-API leak CI gate (no v1.3 solver types reachable from `polint::sdk::prelude::*`).

## Future Requirements

Deferred to v1.4+. Tracked but not in the v1.3 roadmap.

### Precision Layers

- **PREC-FUT-01**: Go VTA provider (type-flow refinement above RTA).
- **PREC-FUT-02**: Bounded Andersen-style points-to as a separate opt-in family.
- **PREC-FUT-03**: Context sensitivity (k-CFA / object-sensitive) on top of the v1.3 solver core.

### Public Surface

- **SDK-FUT-01**: Public SDK views over the v1.3 semantic graph and unified call graph (requires two-milestone benchmark stability before promotion).
- **SDK-FUT-02**: Public `polint inspect graph` and `polint query` commands over solver output.

### Language Scope

- **LANG-FUT-01**: Python semantic frontend and benchmark adapters.
- **LANG-FUT-02**: Java semantic frontend and benchmark adapters.

### Adaptation

- **ADAPT-FUT-01**: Native-callable shim library for JS built-ins (`Array.prototype.map`, `Promise.then`, etc.) loaded through the adaptation schema.
- **ADAPT-FUT-02**: Reflection / dynamic-import auto-modelling with safe defaults.

## Out of Scope

Explicitly excluded from v1.3 to prevent scope creep.

| Feature | Reason |
|---------|--------|
| cgo-hosted Go runtime inside the Rust process | Breaks rayon determinism, the workspace `unsafe_code = "forbid"` lint, and cross-compilation. Use the out-of-process sidecar instead. |
| Reimplementing `go/types` / `go/ssa` in Rust | Multi-engineer-year, diverges from the upstream compatibility authority. Sidecar consumes the official Go libraries. |
| Datalog / Datafrog / `differential-dataflow` framework | Opaque scheduling conflicts with v1.2's deterministic layer-cache and provenance invariants. Solver stays a hand-rolled deterministic worklist. |
| Async runtime (`tokio`/`async-std`) in the solver core | Determinism. The solver is synchronous; rayon handles parallelism at safe boundaries. |
| Promoting any v1.3 type to the public SDK | v1.2 promotion discipline applies: new analysis stays `pub(crate)` until benchmark gates approve over two milestones. |
| Python and Java parity | Go and TS/JS must prove the complete model first. |
| Whole-program closed-world points-to as default analysis | Cost/precision tradeoff is wrong for repo-local rules; available as opt-in only later. |
| Auto-modeled reflection edges in Go or JS | Inventing edges from heuristics destroys precision; explicit `unsupported_edge` is correct behavior. |
| Recall-by-flooding (emitting every plausible edge) | Hard precision floors in the promotion gate make this fail-build, by design. |
| Benchmark-label adaptation (agent reading oracle expected edges) | Adaptation agent runs in a sandbox that cannot read oracle files; model facts whose targets match oracle expectations are rejected. |
| Wildcard / broad-pattern adaptation models | Schema validator requires concrete target patterns; broad patterns flooding recall are rejected design-time. |
| Public CLI for the new semantic graph or solver | `polint inspect unknowns --format json` is the only new public CLI surface in v1.3. |

## Traceability

Which phases cover which requirements. Filled by the roadmapper after roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| IDENT-01 | TBD | Pending |
| IDENT-02 | TBD | Pending |
| IDENT-03 | TBD | Pending |
| REACH-01 | TBD | Pending |
| REACH-02 | TBD | Pending |
| REACH-03 | TBD | Pending |
| GRAPH-01 | TBD | Pending |
| GRAPH-02 | TBD | Pending |
| GRAPH-03 | TBD | Pending |
| GRAPH-04 | TBD | Pending |
| GRAPH-05 | TBD | Pending |
| GO-01 | TBD | Pending |
| GO-02 | TBD | Pending |
| GO-03 | TBD | Pending |
| GO-04 | TBD | Pending |
| GO-05 | TBD | Pending |
| JS-01 | TBD | Pending |
| JS-02 | TBD | Pending |
| JS-03 | TBD | Pending |
| JS-04 | TBD | Pending |
| JS-05 | TBD | Pending |
| ADAPT-01 | TBD | Pending |
| ADAPT-02 | TBD | Pending |
| CACHE-01 | TBD | Pending |
| CACHE-02 | TBD | Pending |
| TAX-01 | TBD | Pending |
| BENCH-01 | TBD | Pending |

**Coverage:**
- v1.3 requirements: 27 total
- Mapped to phases: 0 (pending roadmap)
- Unmapped: 27

---
*Requirements defined: 2026-05-27*
*Last updated: 2026-05-27 after milestone v1.3 start*
