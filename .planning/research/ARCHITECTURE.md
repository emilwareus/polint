# Architecture Research

**Domain:** Multi-language static analysis engine — shared semantic graph and unified call-graph solver for benchmark-grade precision/recall (Go x/tools RTA, Jelly JS/TS).
**Researched:** 2026-05-27
**Confidence:** HIGH (grounded in existing v1.2 crate layout, `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md`, `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md`, and `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md`).

## Standard Architecture

### System Overview — v1.3 Layering Over v1.2 Substrate

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Public surface (frozen v1.2 contract; v1.3 promotes only after gates)    │
│  ┌────────────┐ ┌────────────┐ ┌──────────────┐ ┌──────────────────┐    │
│  │ polint CLI │ │ Rule SDK   │ │ Inspect/Test │ │ Benchmark gates  │    │
│  │ check/test │ │ prelude    │ │ JSON         │ │ default vs adapt │    │
│  └─────┬──────┘ └─────┬──────┘ └──────┬───────┘ └────────┬─────────┘    │
├────────┴──────────────┴───────────────┴──────────────────┴───────────────┤
│ Runner + AnalysisKernel facade (v1.2)                                    │
│  - kernel plan, provider DAG, validation, merge, cap-support reporting   │
├──────────────────────────────────────────────────────────────────────────┤
│ NEW v1.3 LAYER — Shared semantic graph + unified solver core             │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ analysis::semantic_graph    (stable identities, edges, indexes)    │  │
│  │ analysis::reachability      (roots + reachable subgraph)           │  │
│  │ analysis::solver            (constraint store + worklist fixpoint) │  │
│  │   - value/points-to constraints, call constraints, model facts     │  │
│  │   - budgets (tokens, properties, fanout, package depth)            │  │
│  │ analysis::adaptation        (validated repo-local models)          │  │
│  │ analysis::benchmark_identity (RelString, Jelly spans, dedupe)      │  │
│  └────────────────────────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────────┤
│ v1.2 fact-family substrate (consumed by solver; mostly unchanged)        │
│  ┌──────┐ ┌─────┐ ┌─────────────┐ ┌────────────┐ ┌─────────────────┐    │
│  │ MIR  │ │ CFG │ │ direct calls│ │ summaries  │ │ type/value/alias│    │
│  └──────┘ └─────┘ └─────────────┘ └────────────┘ └─────────────────┘    │
│  ┌────────────┐ ┌─────────────┐ ┌──────────────┐ ┌────────────────┐     │
│  │ entrypoints│ │ refined cals│ │ data flow    │ │ slicing/evid.  │     │
│  └────────────┘ └─────────────┘ └──────────────┘ └────────────────┘     │
├──────────────────────────────────────────────────────────────────────────┤
│ Language frontends (own parsing, lifecycle, lowering, identity mapping)  │
│  ┌───────────────────────┐         ┌─────────────────────────────────┐   │
│  │ src/go/  (tree-sitter)│         │ src/ts/  (Oxc parser+semantic)  │   │
│  │  + NEW go_semantic/   │         │  + NEW ts_inventory/, scope/    │   │
│  │    (calls Go sidecar) │         │    object_model/ (extends Oxc)  │   │
│  └────────────┬──────────┘         └─────────────────────────────────┘   │
│               │ JSON-RPC over stdin/stdout                                │
│               ▼                                                            │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ NEW Go sidecar binary: crates/polint/go-sidecar/polint-go-semantic │  │
│  │  - go/packages + go/types + x/tools/go/ssa  (Go-native)            │  │
│  │  - emits stable Go semantic facts (packages, methods, SSA, ...)    │  │
│  └────────────────────────────────────────────────────────────────────┘  │
├──────────────────────────────────────────────────────────────────────────┤
│ Persistence (v1.2 layer cache + new fact families)                       │
│  ┌─────────────────┐ ┌────────────────┐ ┌────────────────────────────┐  │
│  │ InputSnapshot   │ │ LayerCacheStore│ │ Quarantine + dep index     │  │
│  └─────────────────┘ └────────────────┘ └────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | New or Existing | Responsibility | Typical Implementation |
|-----------|-----------------|----------------|------------------------|
| `analysis_kernel::AnalysisKernel` | Existing (v1.2 Phase 20) | Owns provider DAG, scheduling, validation, fact-family ownership, capability support reporting. v1.3 extends manifest list. | `pub(crate) struct AnalysisKernel; KernelInput/KernelOutput`. |
| `analysis::ids` (`FunctionId`, `CallSiteId`, `PlaceId`, `ObjectTokenId`, `PtVarId`, `MirBodyId`, `MirOpId`) | Existing | Dense in-run handles + `stable_key_from_parts` for cross-run identity. v1.3 adds `GraphNodeId`, `GraphEdgeId`, `ConstraintId`, `TokenId`. | Newtype-wrapped `u64` with `FactMeta` sidecar for stable keys. |
| `analysis::mir`, `analysis::cfg`, `analysis::places` | Existing | Per-function operation/control-flow/place facts. v1.3 reads these; does not duplicate. | Already private modules. |
| `analysis::calls` (direct), `analysis::summaries`, `analysis::types`, `analysis::points_to`, `analysis::aliases`, `analysis::entrypoints`, `analysis::extensions` | Existing | Fact producers feeding the solver. v1.3 adds constraint extractors but keeps fact storage. | Module-per-family with `provider.rs`, `store.rs`, `facts.rs`, `cache_key.rs`, `validate.rs`. |
| `analysis::refined_calls` | Existing (v1.2 Phase 37) | Becomes the **call-fact projection layer** for the new unified solver, not a competing solver. Refined edges now come from `analysis::solver` output. | Reuse the existing module structure; rewrite `provider.rs` to consume `solver` output. |
| `analysis::semantic_graph` | **NEW** | Shared node/edge store: functions, callsites, scopes, places, abstract objects, modules, packages. Stable identity layer. Edges typed (`Call`, `MemberOf`, `Alloc`, `Flow`, ...). | `pub(crate) struct SemanticGraph { nodes, edges, indexes }` inside `crates/polint/src/analysis/semantic_graph/`. |
| `analysis::reachability` | **NEW** | Root discovery + reachable-from-roots BFS over the semantic graph. Per-suite mode-aware (RTA-style filter). | `RootSet` (main/init/exported/tests/configured) + `Reachable<F>` flags. |
| `analysis::solver` | **NEW** | Constraint store + worklist fixpoint. Constraint kinds: `CopyEdge`, `Load`, `Store`, `FieldLoad`, `FieldStore`, `Alloc`, `CallConstraint`, `ReturnFlow`, `ModelEdge`. Generalization of existing `points_to::Solver`. | `pub(crate) struct Solver { constraints, sets, queue, budget, status }` with deterministic `BTreeMap`/`VecDeque`. |
| `analysis::adaptation` | **NEW** | Validates and consumes repo-local model facts. Produces `ModelEdge` constraints for the solver. Reports accepted/rejected counts + prompt hash + cache delta. | Schema-validated TOML/JSON model files → `ModelFact` → solver constraints. |
| `analysis::benchmark_identity` | **NEW** | Per-suite identity renderers (Go `RelString`, Jelly `file:start_line:start_col:end_line:end_col`), deduplication, unsupported/setup-missing categorization. | Pure functions over existing facts + `FactMeta` stable keys. |
| `src/go/` + `src/ts/` adapter modules | Existing, **extended** | Frontends own parsing, AST lowering, package/module lifecycle, Oxc semantic facts, language-specific identity mapping. v1.3 adds Go semantic emission (via sidecar) and JS scope/binding/inventory expansion. | Same crate, same module roots. New submodules: `go::semantic`, `ts::inventory`, `ts::scope`, `ts::object_model`. |
| Go sidecar binary (NEW: `polint-go-semantic`) | **NEW**, sibling to existing `polint-go-symbols` | Out-of-process Go helper that runs `go/packages` + `go/types` + `x/tools/go/ssa` and emits stable JSON semantic facts (packages, types, method sets, init functions, SSA-like ops, dynamic dispatch sites). | Go binary in `crates/polint/go-sidecar/polint-go-semantic/`. JSON over stdin/stdout. Rust side spawns via `std::process::Command`. |
| `analysis_kernel::incremental` | Existing | Digest, cache keys, dependency index, quarantine. v1.3 adds new family digests but does not change the cache framework. | New `LayerKind` variants for `SemanticGraph`, `Solver`, `AdaptationModel`, `BenchmarkIdentity`. |

## Recommended Project Structure

```
crates/polint/
├── Cargo.toml
├── go-sidecar/
│   ├── polint-go-symbols/                  # v1.2 sidecar (kept as-is)
│   └── polint-go-semantic/                 # NEW — go/packages + ssa emitter
│       ├── go.mod
│       ├── main.go
│       └── internal/
│           ├── semantic/                   # NEW — fact emission
│           ├── packages/                   # go/packages loader wrappers
│           ├── ssa/                        # x/tools/go/ssa wrapper
│           └── identity/                   # RelString renderer
├── src/
│   ├── analysis/                           # private substrate (existing)
│   │   ├── mir/, cfg/, places.rs           # existing — unchanged
│   │   ├── calls/                          # existing — unchanged (Tier 0)
│   │   ├── summaries/, types/              # existing — fed into solver
│   │   ├── points_to/                      # existing — folded into solver
│   │   ├── refined_calls/                  # existing — REWORKED as projection
│   │   ├── entrypoints/                    # existing — feeds roots
│   │   ├── extensions/                     # existing — model intake path
│   │   │
│   │   ├── semantic_graph/                 # NEW
│   │   │   ├── mod.rs
│   │   │   ├── nodes.rs                    # GraphNode, NodeKind enum
│   │   │   ├── edges.rs                    # GraphEdge, EdgeKind enum
│   │   │   ├── indexes.rs                  # outgoing/incoming/by-kind indexes
│   │   │   ├── store.rs                    # SemanticGraphStore
│   │   │   ├── provider.rs                 # constraint emission orchestrator
│   │   │   ├── cache_key.rs                # provider parameter digest
│   │   │   └── validate.rs                 # invariants (no dangling refs)
│   │   │
│   │   ├── reachability/                   # NEW
│   │   │   ├── mod.rs
│   │   │   ├── roots.rs                    # RootSet construction from entrypoints
│   │   │   ├── facts.rs                    # ReachableFact, RootFact
│   │   │   ├── solver.rs                   # BFS over semantic graph
│   │   │   └── provider.rs
│   │   │
│   │   ├── solver/                         # NEW — unified solver core
│   │   │   ├── mod.rs
│   │   │   ├── constraint.rs               # ConstraintKind enum (CopyEdge, Load, Alloc, CallConstraint, ...)
│   │   │   ├── store.rs                    # ConstraintStore (BTreeMap-keyed)
│   │   │   ├── budget.rs                   # SolverBudget (tokens, fanout, properties, depth)
│   │   │   ├── worklist.rs                 # deterministic VecDeque worklist
│   │   │   ├── tokens.rs                   # AbstractToken (function value, object)
│   │   │   ├── go_rta.rs                   # Go RTA / VTA driver over constraints
│   │   │   ├── ts_tokens.rs                # JS/TS function-token propagation driver
│   │   │   ├── ts_object_model.rs          # JS/TS object/prop/proto/this
│   │   │   ├── facts.rs                    # SolvedCallEdge, SolvedFlow, BudgetStatus
│   │   │   ├── provider.rs
│   │   │   ├── cache_key.rs
│   │   │   └── validate.rs
│   │   │
│   │   ├── adaptation/                     # NEW — validated repo-local models
│   │   │   ├── mod.rs
│   │   │   ├── schema.rs                   # ModelFact schema (TOML/JSON)
│   │   │   ├── loader.rs                   # repo-local .polint/models/*.toml
│   │   │   ├── validator.rs                # target-symbol existence checks
│   │   │   ├── facts.rs                    # AcceptedModelFact, RejectedModelFact
│   │   │   ├── provider.rs                 # emits ModelEdge constraints
│   │   │   ├── cache_key.rs
│   │   │   └── debug.rs                    # prompt hash, accepted/rejected reporting
│   │   │
│   │   ├── benchmark_identity/             # NEW
│   │   │   ├── mod.rs
│   │   │   ├── go_relstring.rs             # x/tools RelString renderer
│   │   │   ├── jelly_span.rs               # Jelly span renderer
│   │   │   ├── dedupe.rs
│   │   │   └── categorize.rs               # wrong-identity vs unsupported vs unresolved
│   │   │
│   │   └── unknown_taxonomy/               # NEW
│   │       ├── mod.rs
│   │       ├── kinds.rs                    # SetupMissing, UnsupportedSemantic, MissingFact, OutOfScope
│   │       └── report.rs                   # actionable diagnostic queue
│   │
│   ├── analysis_kernel/                    # existing — extended manifests
│   ├── go/                                 # existing
│   │   ├── adapter.rs, lifecycle.rs, mod.rs, tests.rs   # existing
│   │   └── semantic/                       # NEW
│   │       ├── mod.rs
│   │       ├── sidecar_client.rs           # spawn + JSON-RPC to polint-go-semantic
│   │       ├── facts.rs                    # decoded sidecar facts
│   │       └── lower.rs                    # maps sidecar facts → analysis::semantic_graph
│   ├── ts/                                 # existing
│   │   ├── adapter.rs, mod.rs, tests.rs    # existing
│   │   ├── inventory/                      # NEW — exact callsite/function inventory from Oxc
│   │   ├── scope/                          # NEW — bindings, imports, module graph
│   │   └── object_model/                   # NEW — alloc sites, property writes, this binding
│   ├── sdk/                                # existing public surface (unchanged in v1.3)
│   └── ...                                 # everything else unchanged
└── tests/
```

### Structure Rationale

- **Stay inside the existing `polint` crate.** v1.2 deliberately consolidated everything into `crates/polint` (no separate `polint-graph`, `polint-solver`, etc.). Promoting submodules to crates now would force public visibility and break the v1.2 promotion discipline. v1.3 keeps the new substrate `pub(crate)` until benchmark gates approve a public view.
- **`analysis::semantic_graph` as a sibling of existing fact modules** rather than a wrapper. The graph is a *projection* over existing facts (functions from semantic index, callsites from `calls::`, places from `places.rs`, allocations from `values`). It owns identities and edges, not the underlying facts.
- **`analysis::solver` absorbs and generalizes `analysis::points_to::solver`.** Today the points-to solver is a private worklist over `PointsToConstraintFact`. v1.3 generalizes the constraint vocabulary (adds `CallConstraint`, `ModelEdge`, token sets) and keeps the same worklist discipline (`BTreeMap`+`VecDeque`, deterministic order, explicit budget).
- **Refined-calls becomes a projection, not a competing solver.** v1.2's `refined_calls::{framework, go, ts_js, summaries, extensions}` heuristic refiners are replaced by `solver`-derived edges. `refined_calls::provider` becomes a thin projector that reads solver output and emits `RefinedCallEdgeFact` for downstream consumers (data flow, evidence). This preserves the existing cache key and downstream contracts.
- **Go sidecar is a new binary, not cgo.** `crates/polint/go-sidecar/polint-go-symbols` already proves the JSON-over-stdio pattern. A second binary `polint-go-semantic` keeps the boundary clean: Rust never links against Go, the sidecar can use `go/packages`/`go/types`/`x/tools/go/ssa` natively, and the cache key digests the sidecar binary digest + lifecycle inputs.
- **JS solver lives inside `analysis::solver`, not inside `polint-ts`.** The solver is language-neutral over constraints; only the *constraint emitters* are language-specific. Putting `ts_tokens.rs`, `ts_object_model.rs`, and `go_rta.rs` as sibling drivers under the shared solver enforces this and avoids duplicating worklist/budget/identity logic.
- **`ts/inventory`, `ts/scope`, `ts/object_model` are constraint emitters, not solvers.** They walk Oxc semantic facts and emit constraints into `analysis::solver::constraint::ConstraintStore`. The same module tree under `go/semantic/lower.rs` converts sidecar JSON into constraints.
- **`adaptation` is its own module, not inside `extensions`.** v1.2's `analysis::extensions` is the Rust-code extension provider sink (typed Rust crates discovered at runtime). `analysis::adaptation` is the declarative validated-model-file path used by an agent. They emit into the same solver via different validators.

## Architectural Patterns

### Pattern 1: Provider-DAG + Constraint Emission

**What:** Language frontends and fact families produce constraints into the shared solver during the kernel's provider phase. The solver is itself a kernel provider whose inputs are the constraint set, and whose outputs are call/flow edges + token sets + budget status.

**When to use:** Whenever a fact family contributes to call-graph or value-flow precision (calls, summaries, types, points-to, frameworks, adaptation models).

**Trade-offs:**
- (+) Single solver = no duplicate worklists, single budget envelope, single cache key.
- (+) Frontends stay decoupled (Go vs TS) and can iterate independently.
- (-) Requires careful ordering: constraints from later providers must not silently invalidate earlier solver runs. Mitigated by making the solver run once after all constraint emitters complete (one solver phase, single fixpoint).

**Example:**
```rust
// Inside analysis::solver::provider
pub(crate) fn derive_unified_call_graph(
    db: &mut AnalysisDb,
    semantic_graph: &SemanticGraphStore,
    constraints: &ConstraintStore,
    roots: &RootSet,
    budget: SolverBudget,
    input_snapshot: &InputSnapshot,
) -> SolverProviderOutput {
    let result = Solver::new(constraints, &roots.reachable_seed(), budget).solve();
    db.replace_solved_call_edges(result.edges);
    SolverProviderOutput {
        cache_stats: result.cache_stats,
        output_digest: Some(solver_output_digest(input_snapshot, &result)),
        budget_status: result.budget_status,
        diagnostics: result.diagnostics,
    }
}
```

### Pattern 2: Frontend Constraint Lowering

**What:** Each language frontend has a `lower.rs` that converts language-specific facts into solver constraints. The frontend owns identity mapping; the solver owns iteration.

**When to use:** For every language. Go (via sidecar JSON), TS/JS (via Oxc).

**Trade-offs:**
- (+) Clean separation: solver code stays language-agnostic.
- (+) Adding Python or Java later means writing a `lower.rs`, not a new solver.
- (-) Requires stable constraint vocabulary up front. Mitigated by starting narrow (CopyEdge, Alloc, CallConstraint) and adding kinds with schema-version bumps.

**Example:**
```rust
// src/go/semantic/lower.rs
pub(crate) fn lower_go_semantic_facts(
    db: &AnalysisDb,
    sidecar: &GoSemanticFacts,
    constraints: &mut ConstraintStore,
    semantic_graph: &mut SemanticGraphStore,
) {
    for method in &sidecar.method_sets {
        semantic_graph.add_edge(GraphEdge::method_of(method.func_id, method.receiver_type));
        // RTA driver later uses method-set indexes to resolve interface invokes.
    }
    for callsite in &sidecar.dynamic_callsites {
        constraints.push(Constraint::CallConstraint {
            site: callsite.id,
            callee_place: callsite.callee_place,
            arg_places: callsite.args.clone(),
            algorithm: CallAlgorithm::GoRta,
        });
    }
}
```

### Pattern 3: Sidecar Process for Language-Native Semantics

**What:** When a language has authoritative tooling (Go has `go/packages`/`go/types`/`x/tools/go/ssa`), run it as a separate Go binary. Rust spawns it, passes config via flags + stdin, receives JSON on stdout.

**When to use:** When the alternative (reimplementing Go's type system in Rust, or cgo-linking) would dwarf the analysis investment.

**Trade-offs:**
- (+) Uses Go's official semantic tooling unchanged.
- (+) No `unsafe` in Rust, no cgo, no Go runtime in the polint binary.
- (+) Sidecar process can be parallelized per module root.
- (-) Process spawn overhead (~50-200ms cold). Mitigated by caching the JSON output keyed by `(go.mod digest, build tags, package patterns, sidecar binary digest)`.
- (-) JSON serialization cost. Mitigated by streaming line-delimited records (NDJSON) for large outputs.
- (-) Sidecar must be co-shipped. Mitigated by building it during `cargo build` via a `build.rs` hook (or making it a `make build-sidecars` target with a clear missing-sidecar diagnostic).

**Example:**
```rust
// src/go/semantic/sidecar_client.rs
pub(crate) fn run_polint_go_semantic(config: &SidecarConfig) -> Result<GoSemanticFacts> {
    let output = std::process::Command::new(config.binary_path())
        .args(["semantic", "--json",
               "--root", &config.root,
               "--module-roots", &config.module_roots.join(","),
               "--patterns", &config.patterns.join(","),
               "--build-tags", &config.build_tags.join(",")])
        .output()?;
    if !output.status.success() {
        return Err(SidecarError::NonZeroExit(output.status, output.stderr.into()));
    }
    let facts: GoSemanticFacts = serde_json::from_slice(&output.stdout)?;
    Ok(facts)
}
```

### Pattern 4: Cache-Family Digest Composition

**What:** Each new fact family adds a cache_key.rs that composes:
1. Provider parameters (deterministic defaults).
2. Upstream output digests (from `KernelRunReport`).
3. Lifecycle/toolchain/sidecar digests (`InputSnapshot::components`).

This matches the existing v1.2 pattern (see `analysis::refined_calls::cache_key`).

**When to use:** Every new provider that participates in the layer cache.

**Trade-offs:**
- (+) Composability with existing kernel; no new cache framework.
- (+) Conservative invalidation: any upstream change recomputes.
- (-) Cache reuse is coarse-grained. Acceptable for first slice; can be refined per-function later via existing `DependencyIndex` infrastructure.

### Pattern 5: Budget-First Solver Design

**What:** Every solver run has an explicit `SolverBudget { max_steps, max_tokens_per_var, max_objects_per_var, max_dynamic_call_fanout, max_property_buckets, max_package_depth }`. Budget exhaustion is a first-class outcome: it produces `BudgetExceeded` status, marks affected edges as `Unsupported(BudgetExceeded)`, and contributes to the unknown taxonomy.

**When to use:** Any solver loop (token propagation, points-to, RTA fixpoint, object/property propagation).

**Trade-offs:**
- (+) Deterministic upper bound on cost.
- (+) Honest precision reporting: precision is not faked by silently dropping work.
- (-) Tuning budgets per repo is a real concern. Mitigated by reporting budget-exceeded rates per benchmark run; users can override per-rule.

## Data Flow

### Top-Level Flow (One `polint check` Run)

```
polint check
  │
  ▼
LoadedConfig + AnalysisPlan (capabilities) + Cache
  │
  ▼
AnalysisKernel::run(input)
  │
  ├──► load_analysis_files (existing)
  ├──► InputSnapshot (existing + new digests for sidecar, models, budgets)
  │
  ├──► Frontend providers (parallel where safe)
  │      ├─ go/adapter (tree-sitter syntax)            ── existing
  │      ├─ go/semantic/sidecar_client → polint-go-semantic   ── NEW
  │      ├─ go/semantic/lower → semantic_graph + constraints  ── NEW
  │      ├─ ts/adapter (Oxc syntax + semantic)         ── existing
  │      ├─ ts/inventory → exact callsite/function ids ── NEW
  │      ├─ ts/scope → bindings + module graph         ── NEW
  │      ├─ ts/object_model → alloc sites + properties ── NEW
  │      └─ ts → semantic_graph + constraints          ── NEW
  │
  ├──► Semantic fact derivers (existing v1.2, mostly unchanged)
  │      ├─ analysis::mir, cfg, places, calls (direct)
  │      ├─ analysis::summaries, types, aliases
  │      └─ analysis::entrypoints, extensions
  │
  ├──► NEW: analysis::adaptation (load + validate repo-local models)
  │      └─ ModelEdge constraints + accepted/rejected report
  │
  ├──► NEW: analysis::reachability (build RootSet from entrypoints,
  │         optionally seed reachable set for RTA fixpoint)
  │
  ├──► NEW: analysis::solver (single unified fixpoint)
  │      input: ConstraintStore + SemanticGraphStore + RootSet + Budget
  │      output: SolvedCallEdges, TokenSets, BudgetStatus
  │
  ├──► analysis::refined_calls::provider (REWORKED — projects solver edges)
  │      └─ existing downstream consumers see same RefinedCallEdgeFact shape
  │
  ├──► Existing downstream: data_flow, slicing, evidence (unchanged contracts)
  │
  ├──► NEW: analysis::benchmark_identity (renders Go RelString / Jelly spans)
  ├──► NEW: analysis::unknown_taxonomy (categorize, build queue)
  │
  ├──► Validation + merge (existing kernel pass)
  │
  ▼
KernelOutput { db, diagnostics, capability_support, run_report }
  │
  ▼
runner runs rules over db using public SDK views (unchanged)
```

### Constraint Emission and Solver Detail

```
[Frontend facts]                  [Constraints]
  go::semantic::method_sets   ──►  MemberOf(func, receiver)
  go::semantic::dyn_callsites ──►  CallConstraint(site, callee_place, args, Go RTA)
  go::semantic::ssa_assigns   ──►  CopyEdge(dst, src)
  ts::scope::imports          ──►  CopyEdge(local, exported)
  ts::inventory::callsite     ──►  CallConstraint(site, callee_place, ...)
  ts::object_model::alloc     ──►  Alloc(place, object_token)
  ts::object_model::propwrite ──►  FieldStore(base, prop, src)
  ts::object_model::propread  ──►  FieldLoad(dst, base, prop)
  analysis::summaries::tito   ──►  CopyEdge(call.result, call.arg[i]) per summary
  analysis::types::declared   ──►  TypeConstraint(place, type) (RTA uses for type seeds)
  analysis::adaptation::model ──►  ModelEdge(call_site → func, validated)
                                                │
                                                ▼
                                  ┌─────────────────────────────────┐
                                  │  analysis::solver::Solver       │
                                  │  - worklist over (var, tokens)  │
                                  │  - integrated with reachability │
                                  │    (RTA: only solve reachable)  │
                                  │  - integrated with semantic     │
                                  │    graph for method-set lookup  │
                                  │  - budget-aware                 │
                                  └─────────────┬───────────────────┘
                                                ▼
                                  [SolvedCallEdge facts]
                                                ▼
                          analysis::refined_calls::provider
                                                ▼
                          db.replace_refined_call_facts(...)
                                                ▼
                          existing data flow / evidence / SDK views
```

### Key Data Flows

1. **Go RTA fixed point:** `entrypoints` (roots) → `reachability` (seed reachable set) → `solver::go_rta` iterates: for each reachable function, walk SSA constraints, propagate address-taken/type tokens through `MemberOf` edges in `semantic_graph`, expand reachable set, repeat until fixpoint or budget. Each new edge updates `semantic_graph` and triggers downstream `refined_calls` projection.
2. **JS/TS function-token propagation:** `ts::inventory` (callsites + functions as tokens) + `ts::scope` (bindings) + `ts::object_model` (property graph) → `solver::ts_tokens` propagates tokens through `CopyEdge`/`FieldLoad`/`FieldStore` constraints. Token set at a callee expression resolves the call.
3. **Adaptation feedback loop:** repo-local `.polint/models/*.toml` → `adaptation::loader` → `validator` (target symbols must exist via `semantic_graph`) → accepted `ModelEdge` constraints → solver consumes alongside derived constraints → `adaptation::debug` reports accepted/rejected counts + precision/recall delta vs default mode.
4. **Cache invalidation:** any of {file content, go.mod, package.json, tsconfig, go-sidecar binary digest, polint-go-semantic schema version, model file content, solver budget config} changes → `InputSnapshot` component changes → upstream digest changes → solver's `cache_key` differs → solver recomputes. Existing `quarantine` mechanism handles model-extension changes the same way it handles Rust extensions today.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Small repos (<1k functions) | Default budgets; everything runs cold in <5s. Sidecar invocation amortized. Solver fits in <50MB. |
| Medium repos (1k-10k functions) | Per-package solver granularity; cache reuse becomes critical. Sidecar parallelized per module root. Budget enforcement starts mattering for object/property model. |
| Large repos (10k-100k functions) | Solver becomes the bottleneck. Must use the existing `DependencyIndex` to invalidate per-package solver subgraphs. Token-set budgets enforced strictly. Adaptation models become primary recall path (broad solver becomes too expensive). |
| Very large (100k+ functions) | Default mode degrades to direct + summary-projected edges; full solver is opt-in per language or per scope. RTA on Go restricted to module roots actually rule-targeted. JS object model restricted to callsite ancestor scopes via demand queries. |

### Scaling Priorities

1. **First bottleneck — Go sidecar cold runs:** `go/packages` + `go/ssa` is the heaviest single cost. Fix by caching sidecar JSON output keyed on `go.mod` + `go.sum` + sidecar binary digest, and by parallelizing per module root.
2. **Second bottleneck — JS/TS token propagation explosion:** unbounded tokens × variables can blow memory. Fix by aggressive allocation-site abstraction + token-set caps (`max_tokens_per_var = 64` default), and by demand-driven property abstraction (compute property buckets only for properties actually read at callsites).
3. **Third bottleneck — Adaptation model fanout:** broad wildcard models can flood constraints. Fix by validator-side fanout limits (`max_targets_per_model_fact = 16`) and by rejecting models whose accepted count exceeds the cap.

## Anti-Patterns

### Anti-Pattern 1: Building a Separate `polint-graph` Crate

**What people do:** Promote the new semantic graph to its own workspace crate to "make it modular."
**Why it's wrong:** Forces public visibility on every type the solver needs. v1.2 deliberately kept everything inside `crates/polint` with `pub(crate)` access. A separate crate would either expose internals prematurely or require a parallel internal contract surface. Both break the v1.2 promotion discipline (validated benchmarks before public APIs).
**Do this instead:** Keep `analysis::semantic_graph`, `analysis::solver`, etc., as `pub(crate)` modules inside the existing `polint` crate. Promote individual SDK views only after `BenchmarkPromotionGate` proves them.

### Anti-Pattern 2: cgo-Linking Go's Type Checker into Rust

**What people do:** Use cgo or unsafe FFI to call `go/types` from Rust directly.
**Why it's wrong:** Breaks `unsafe_code = "forbid"` at the workspace level; ties polint to Go's runtime; complicates cross-compilation; makes failure modes much harder to debug.
**Do this instead:** Run the Go sidecar as a child process with line-delimited JSON. The existing `polint-go-symbols` sidecar proves this pattern works. Failure modes are clean: missing sidecar binary → structured `capability_support` diagnostic; non-zero exit → fact emission marked `SetupMissing` in the unknown taxonomy.

### Anti-Pattern 3: Per-Language Solvers with Duplicated Worklist Logic

**What people do:** Build a "Go solver" and a "JS solver" as independent modules, each with its own constraint store, worklist, and budget.
**Why it's wrong:** Doubles maintenance, makes adaptation models harder (which solver owns them?), and prevents cross-language constraints (e.g., a model that says "this Go cgo callback invokes this JS function"). Budgets become inconsistent — one solver could blow memory while the other is idle.
**Do this instead:** One `analysis::solver` with one `ConstraintStore`, one `Worklist`, one `SolverBudget`. Language-specific *drivers* (`go_rta.rs`, `ts_tokens.rs`, `ts_object_model.rs`) live as siblings under `solver/`. Each driver is a function `fn drive(solver: &mut Solver, ...)` that adds its constraint kinds; the solver does the iteration.

### Anti-Pattern 4: Letting Adaptation Models Bypass Validation

**What people do:** Accept raw "expected edge" facts from agent-authored models without checking that targets exist.
**Why it's wrong:** Recall scores become meaningless — the agent can copy the benchmark oracle and "win" with zero engine improvement. Precision degrades silently because nothing rejects bad models.
**Do this instead:** `analysis::adaptation::validator` must confirm each model fact's target resolves to a real function in the semantic graph (or carries an explicit `kind = "external"` declaration). Rejected facts are logged with reason. `benchmark adapted` mode reports `accepted_facts`, `rejected_facts`, `precision_delta`, `recall_delta`, `runtime_delta`, and a prompt hash so review is easy.

### Anti-Pattern 5: Solving Before Identity Is Stable

**What people do:** Build the solver and benchmark adapter before identity normalization works.
**Why it's wrong:** "Improvements" cannot be measured. Edges might be correct but score wrong because of span identity bugs; you'll waste cycles chasing precision regressions that are actually identity bugs.
**Do this instead:** Ship `analysis::benchmark_identity` and identity hardening (Phase 1 of the GRAPH-ENGINE-BENCHMARK-RESEARCH order, line 480) BEFORE the solver. The roadmap MUST sequence identity → reachability → JS inventory → JS scope → Go semantic → solver. Identity is the only way to make subsequent metric deltas trustworthy.

### Anti-Pattern 6: Promoting `SemanticGraph` to the Public SDK Early

**What people do:** Expose `polint::sdk::facts::SemanticGraph<'_>` so rules can walk the graph.
**Why it's wrong:** The graph schema will evolve through multiple solver iterations. Locking the shape now would either freeze a bad design or require constant SDK churn. v1.2 spent a whole phase (41) carefully selecting which views to promote; v1.3 should keep the same discipline.
**Do this instead:** Solver outputs flow into existing `RefinedCallEdgeFact` and `DataFlow` views (already public). New SDK views (e.g., `Reachability<'_>`, `Adaptation<'_>`) are promoted only after `BenchmarkPromotionGate` records stable deltas across two milestones.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Go toolchain (`go/packages`, `go/types`, `x/tools/go/ssa`) | Out-of-process sidecar (`polint-go-semantic`), JSON over stdio | Co-shipped binary; cache key includes binary digest + Go version digest. Failure → `SetupMissing` row in capability support. |
| Oxc (TypeScript/JavaScript) | In-process Rust crate (`oxc_parser`, `oxc_semantic`, `oxc_resolver`, version 0.129.0) | Already used; v1.3 extends usage to feed `ts::inventory`, `ts::scope`, `ts::object_model`. Pin Oxc version exactly because semantic IDs change across versions. |
| tree-sitter-go | In-process | Kept for cheap syntax-only Go facts and as fallback when sidecar is unavailable. |
| Repo-local model files (`.polint/models/*.toml`) | File-system scan + schema validation | Loaded by `analysis::adaptation::loader`. Validated against the live `semantic_graph`. Failures are reported as accepted/rejected facts, never silently dropped. |
| Repo-local Rust extension crates (existing) | Out-of-process via `extensions::host` (v1.2) | Unchanged. Extensions can emit `SyntheticCallableId` facts that flow into the solver as `Constraint::ExternalCallable`. |
| Benchmark fixtures (Go x/tools RTA testdata, Jelly micro suite) | `polint-bench` reads from `.context/graph-benchmarks/` | Existing infrastructure. v1.3 adds `BenchmarkIdentity::render_*` calls before scoring. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `runner` → `analysis_kernel` | Direct call (`AnalysisKernel::run`) | v1.2 contract; unchanged. |
| `analysis_kernel` → `analysis::*` providers | Provider DAG (`ProviderManifest`) | Existing pattern. New providers register manifests with inputs/outputs/cache policy. |
| Frontend (`go/`, `ts/`) → `analysis::semantic_graph` | Direct `pub(crate)` mutation during their kernel slot | Frontends emit nodes/edges; semantic_graph validates during its provider step. |
| Frontend (`go/`, `ts/`) → `analysis::solver::constraint::ConstraintStore` | Direct `pub(crate)` push of `Constraint` enums | Constraints flow into one store; solver runs once at end. |
| `go/semantic/sidecar_client` → `polint-go-semantic` binary | `std::process::Command` + JSON stdin/stdout | Spawns per module root; outputs cached by sidecar digest + lifecycle digest. |
| `analysis::adaptation::loader` → `analysis::solver` | `Constraint::ModelEdge` push after validation | Validation gate is the trust boundary; rejected facts never enter the solver. |
| `analysis::solver` → `analysis::refined_calls` | Direct (refined_calls provider reads solver output, projects edges) | v1.3 rewrites `refined_calls::provider` but keeps its public-facing fact shape (`RefinedCallEdgeFact`). |
| `analysis::*` → `db: AnalysisDb` (writes) | All writes go through kernel's validation + merge step | Existing v1.2 invariant; do not bypass. |
| `analysis::*` → public SDK views | Only through `sdk::facts::FactView` adaptors | v1.2 contract; only promote new views via `BenchmarkPromotionGate`. |

## Suggested Build Order (Dependency-Driven)

This ordering reflects the dependency analysis: each step's inputs are facts produced by earlier steps. It mirrors the `GRAPH-ENGINE-BENCHMARK-RESEARCH.md:478` Recommended Implementation Order but is mapped to concrete v1.3 module work.

| Step | Work | Depends On | Risk |
|---:|---|---|---|
| 1 | `analysis::benchmark_identity` + stable function/callsite identity hardening + dedupe + identity-vs-unsupported categorization. Plumbs `FactMeta` stable keys all the way to Go `RelString` and Jelly span renderers. | v1.2 `FactMeta`, `analysis::ids`, `analysis::calls` | Low. Mostly pure functions; high impact on metric trust. |
| 2 | `analysis::reachability` + root semantics (Go `main`/`init`, exported, tests, configured). Per-suite scoring mode. | v1.2 `entrypoints`, `calls`, `semantic_graph` skeleton | Low–medium. Per-suite mode must be honest about RTA vs full-graph scoring. |
| 3 | `analysis::semantic_graph` skeleton: node/edge stores, indexes, validation. Initially populated only by step 1's identities + step 2's reachability + existing direct-call facts. | Steps 1, 2 | Low. Foundation for everything that follows. |
| 4 | `ts::inventory` — exact JS/TS function and callsite enumeration with Oxc spans matching Jelly format. | Step 1 (identity), existing Oxc semantic pass | Medium. Span parity with Jelly is finicky across function expressions, arrows, methods, accessors. |
| 5 | `ts::scope` — bindings, imports (CJS + ESM), module graph, direct call resolution. Emits `CopyEdge` and direct `CallConstraint`s. | Steps 3, 4 | Medium. Path alias + CJS resolution edge cases. |
| 6 | `go/semantic` frontend: build `polint-go-semantic` sidecar, define JSON schema, sidecar client, lowering to `semantic_graph` + constraints (method sets, init functions, SSA-style ops, dynamic callsites). | Steps 1, 3 | High. Largest single workstream. Sidecar build/distribution + Go version compatibility. |
| 7 | `analysis::solver` core: `ConstraintStore`, `Worklist`, `SolverBudget`, deterministic worklist iteration, `BudgetStatus` reporting. Initially supports only `CopyEdge` + `CallConstraint` (folds in `points_to` as a sub-domain). | Steps 3, 5, 6 (constraints from at least one frontend) | Medium. Generalizing the existing `points_to::solver` carefully. |
| 8 | `solver::go_rta` driver: reachability fixpoint, address-taken tracking, dynamic dispatch, interface invoke resolution via `semantic_graph` method-set indexes. | Steps 2, 6, 7 | High. Correctness work. Must produce metric improvements that match expected band (recall 70-90% target). |
| 9 | `solver::ts_tokens` driver: function-token propagation through assignments, parameters, returns, closures. | Steps 5, 7 | High. Risk: budget tuning. |
| 10 | `ts::object_model` + `solver::ts_object_model` driver: allocation-site abstraction, property writes/reads, `this` binding, prototype lookup. | Steps 9, plus `ts::inventory` | Highest. Largest precision/cost tradeoff space. |
| 11 | `analysis::adaptation`: schema + loader + validator + provider. Hooks into solver as `ModelEdge` constraints. Reports accepted/rejected + delta. | Steps 7, 8, or 9 (at least one solver driver functional) | Medium. Schema design + validation rules. |
| 12 | `refined_calls::provider` rework — project solver edges into `RefinedCallEdgeFact`, retire heuristic refiners. | Steps 7–10 | Medium. Must preserve downstream `data_flow`/`evidence` contracts. |
| 13 | `analysis::unknown_taxonomy` — categorize all unresolved/unsupported/setup-missing/out-of-scope statuses; build actionable diagnostic queue. Plumb through all providers. | All earlier steps | Low–medium. Continuous; should be done concurrent with each provider. |
| 14 | Cache + budget integration: new `LayerKind` entries, cache_key digests for every new provider, budget config in `InputSnapshot`. | All earlier steps | Medium. The existing cache framework absorbs this if each new provider follows the v1.2 pattern. |
| 15 | Benchmark promotion gates: extend `polint-bench` to record default-vs-adapted deltas using `benchmark_identity` renderers; gate any new SDK view promotion on stable deltas. | All earlier steps | Low. Existing `BenchmarkPromotionGate` infrastructure. |

### Critical Path Notes

- **Identity (step 1) blocks everything downstream.** Until identity is right, metric improvements are unmeasurable.
- **Steps 4–5 (JS) and step 6 (Go) are independent** and can be parallelized across the team.
- **Step 7 (solver core) requires at least one frontend's constraints** to validate; can start with TS direct calls + existing `points_to` constraints folded in.
- **Steps 8 and 9 share the solver** but their drivers are independent; they can be parallelized.
- **Step 10 (object model) is the largest single workstream** and should ship behind a capability flag until benchmark gates approve.
- **Step 11 (adaptation) requires a working solver** — premature adaptation produces misleading metrics (see Anti-Pattern 4).

## Risk Areas (Flagged for Planning)

| Risk | Location | Mitigation |
|------|----------|------------|
| Go sidecar build/distribution | `crates/polint/go-sidecar/polint-go-semantic` | `make` target builds during CI; missing-binary diagnostic at runtime; cache sidecar digest. Document fallback to syntax-only Go in `capability_support`. |
| cgo temptation | Anywhere touching Go's type system | Workspace already enforces `unsafe_code = "forbid"`. Lint enforces no cgo deps. Anti-Pattern 2 documented above. |
| Solver budget tuning | `analysis::solver::budget` | Per-language defaults + per-rule overrides. Report budget-exceeded rate per benchmark run. |
| Cache invariants for new families | All new `cache_key.rs` | Follow v1.2 pattern verbatim (see `analysis::refined_calls::cache_key`). Each provider's parameter digest + upstream digests + InputSnapshot components. |
| JS object/property explosion | `ts::object_model` + `solver::ts_object_model` | Allocation-site abstraction + token caps + demand-driven property abstraction. Property buckets default to known-string-or-unknown only. |
| Adaptation gaming benchmarks | `analysis::adaptation::validator` | Validator rejects facts whose target does not exist in the semantic graph. Benchmark adapted mode publishes prompt hash + accepted/rejected counts + delta vs default. |
| Public-surface temptation | New modules | Keep all new modules `pub(crate)`. Promote views only through `BenchmarkPromotionGate` with two-milestone stability. |
| Refined-calls contract drift | `analysis::refined_calls::provider` rework | Keep `RefinedCallEdgeFact` shape unchanged; only the producer changes. Downstream `data_flow`, `evidence`, public SDK `CallGraph<'_>` (still unsupported) and `DataFlow<'_>` views see identical facts. |
| Sidecar JSON schema drift | `polint-go-semantic` ↔ `go::semantic::facts` | Schema version field in sidecar output; mismatch → `SetupMissing`. Sidecar binary digest in cache key catches accidental upgrades. |
| Reachability scoring mode mismatch | `analysis::reachability` + bench scoring | Per-suite mode constant in `polint-bench`. RTA mode filters unreachable edges from scoring; full-graph mode does not. Mode is recorded in the report. |

## Public vs Private Surface Decisions

| Module | v1.3 Visibility | Promotion Gate |
|--------|-----------------|----------------|
| `analysis::semantic_graph` | `pub(crate)` | Possibly never public. Most rules should consume typed views (`CallGraph`, `Reachability`) not the raw graph. |
| `analysis::solver` | `pub(crate)` | Solver internals stay private. Solver outputs (call edges, budget status) project into existing public views. |
| `analysis::reachability` | `pub(crate)` for v1.3 | `Reachability<'_>` SDK view candidate after two-milestone stability. |
| `analysis::adaptation` | `pub(crate)` for v1.3 | Public surface = the model file schema (TOML), not the Rust type. Schema versioned with deprecation rules. |
| `analysis::benchmark_identity` | `pub(crate)`, used only by `polint-bench` | Never public — benchmark-specific. |
| `analysis::unknown_taxonomy` | `pub(crate)` for v1.3 | Diagnostic queue may be exposed via `polint inspect unknowns --format json` once stable. |
| `go::semantic`, `ts::inventory`, `ts::scope`, `ts::object_model` | `pub(crate)` | Frontend internals always private. Their outputs flow only into solver constraints. |
| `sdk::facts::CallGraph<'_>` (currently `Unsupported`) | Stays `Unsupported` in v1.3 unless benchmark gates approve a stable shape | Promotion gate: two milestones of stable RTA + token-solver deltas, plus typed `CallGraph<'_>` review. |
| `sdk::facts::DataFlow<'_>` | Already public (v1.2) | Behavior improves automatically as solver-driven refined calls feed it. Contract unchanged. |
| `polint inspect adaptation --format json` | New CLI surface candidate | After step 11 ships and the schema stabilizes. |

## Sources

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` (Architectural Recommendation at line 506; Recommended Implementation Order at line 478; per-step Expected Metric Impact / Complexity / Cost sections).
- `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md` (call fact model, provider tiers, dependency direction).
- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` (kernel facade pattern, provider manifests, scheduling).
- `.planning/PROJECT.md` (v1.2 phase summaries, v1.3 milestone goal, target features).
- `crates/polint/src/analysis_kernel/{mod,provider,incremental/*}.rs` (existing kernel + cache substrate).
- `crates/polint/src/analysis/{mir,cfg,calls,summaries,points_to,types,refined_calls,entrypoints,extensions}/` (existing fact-family layout).
- `crates/polint/src/analysis/points_to/solver.rs` (existing worklist + budget pattern to generalize).
- `crates/polint/src/analysis/refined_calls/{provider,cache_key}.rs` (existing pattern for new providers).
- `crates/polint/go-sidecar/polint-go-symbols/main.go` (existing sidecar pattern to replicate for `polint-go-semantic`).
- `crates/polint/src/sdk/mod.rs` + `sdk/facts.rs` (current public surface; promotion discipline).
- `Cargo.toml` workspace (`unsafe_code = "forbid"`, pinned Oxc 0.129.0).

---
*Architecture research for: polint v1.3 Graph Engine Precision (shared semantic graph + unified call-graph solver)*
*Researched: 2026-05-27*
