# JS Value-Token Heap (`js_points_to`): Implementation Plan

*Design + phase plan + verification strategy. 2026-06-11.*

---

## STATUS — Phase 0 done + solver core landed (2026-06-11)

**Phase 0 spike (the riskiest unknown) is resolved, and it CHANGED the architecture.**
Empirically, via a throwaway diagnostic on a tiny JS repo under the benchmark's empty
`AnalysisPlan` (kept ignored at
`eval/external/jelly_callgraph.rs::diag_what_populates_under_empty_plan`):

1. The semantic-graph → solver → refined_calls chain **does** run under the empty plan
   (`refined_call_edges` and `solver_derived_edges` are non-empty), and the jelly adapter
   scores `refined_call_edges`. So a solver policy *could* technically reach the benchmark.
2. **But the semantic-graph builder is LOSSY exactly where we need precision.** For
   `Foo[prop] = g` with `const prop = "qwe"`, it emits
   `FieldStore { base: node(module-scope), field: "computed_bucket", src: node(0) }` — the
   constant key is **not** resolved (lumped into one `computed_bucket`), the base is
   mis-attributed to the enclosing scope rather than the `Foo` function-object, and
   function-objects / `new`-instances are under-modeled as allocation sites (4 Allocs for
   7 functions+objects+instances). The existing `TsObjectModel` solver therefore resolves
   **none** of the real failing shapes (`Foo[prop]()→g`, `inst.p2()→g`, the object-return
   depth chain). Fixing that builder is invasive to a shared, snapshot-pinned kernel
   component.

**Decision (revises §2.1):** build the heap **greenfield, AST-based**, harvesting
constraints with full fidelity (reusing `ts_value_flows`' decoders), and emit edges into
**`call_targets` via the calls provider** (where `ts_value_flows` already plugs in,
`calls/provider.rs:41`), **not** as a semantic-graph solver policy. The module lives at
`analysis/calls/js_points_to.rs` (next to `ts_value_flows`), not under `solver/`. The
solver-framework integration described in §2.1 is superseded by this finding.

**Solver core landed and validated.** `analysis/calls/js_points_to.rs` (~450 lines)
implements `Token` / `CellId` / `Constraint` / a delta-driven worklist `PointsToProgram::solve`
with field-sensitive lazily-minted property cells, deferred field-load/store/call listeners,
call wiring (args→params, return→result, this→this), budgets (steps + tokens-per-cell) with
honest exhaustion, and determinism. **7 unit tests prove the model resolves the actual
failing shapes** on hand-built constraints: `resolves_function_object_dynamic_const_key`,
`resolves_instance_dynamic_key`, `resolves_object_return_depth_chain`,
`resolves_higher_order_param_flow`, plus direct call, budget honesty, and determinism.
This retires the second unknown (does the constraint model actually capture the gap — yes).

**Remaining = the harvest + wiring + alignment** (Phases 1b–4 below): AST → `Constraint`s,
`call_targets` emission, native models, Jelly span/attribution alignment, precision/perf.
Both project risks (integration path; model adequacy) are now retired; what's left is the
large-but-lower-risk translation work.

---

Successor to the recognizer push logged in
`performance/2026-06-06-jelly-gap-closure-research.md` (iterations 52–56,
F1 75.31% → 77.26%, +39 TP / 0 FP). Targets the buckets the recognizer
architecture cannot reach (see `2026-06-08-jelly-remaining-bucket-research.md` §4
and the "what remains" analysis in
`2026-06-09-teaching-a-linter-to-read-javascript.md`).

---

## 1. Goal and success criteria

Build a whole-program, Andersen-style (inclusion-based) points-to fixpoint for
JS/TS — a *value-token heap* — as an **additional** call-edge source running
beside the existing recognizers, and use it to close the value-flow-depth and
dependency-tail recall gaps.

**Hard acceptance gates** (same discipline as iterations 52–56):

| Gate | Threshold | Measured how |
|---|---|---|
| G1 — depth bucket | ≥ 80 of the ~130 "value-flow depth" micro FN close | per-case TP/FN diff in `jelly-callgraph-micro-baseline.json` |
| G2 — Express chain | helloworld `res.send` (app.js:6) resolves to response.js:107 and the response.js interior un-prunes | the `track.py` target-edge list |
| G3 — precision floor | suite precision never < 0.94 at any kept iteration | benchmark summary |
| G4 — no regressions | full `cargo test -p polint --lib` green at every kept iteration | CI command below |
| G5 — determinism | identical output hash across two consecutive runs of the same source | benchmark `output_hash` |

**Non-goals (v1):** context sensitivity; flow sensitivity (no replaceable
prototype slots yet — same limitation as today, explicitly accepted); matching
Jelly's index-insensitive array blurring (we keep our better precision and accept
those FP); `eval`/`Function(string)`; soundness.

Expected landing zone if G1+G2 hold: **+130–200 TP → recall ~74–78% → F1 ~84–86%.**

---

## 2. Architecture

### 2.1 Where it sits (one diagram)

```
                         AST (oxc, per file)
                              │
              ┌───────────────┴────────────────┐
              │                                │
   calls/ts_value_flows.rs           NEW: solver/js_points_to/
   (recognizers, KEEP AS-IS)         constraint generation (AST walk)
              │                                │
              │                     JsPointsToInputs (constraints,
              │                      tokens, cells — serializable)
              │                                │
              │                      solver/js_points_to/fixpoint.rs
              │                      (worklist solve → token sets)
              │                                │
              │                      dispatch.rs: callee-cell tokens
              │                      × call sites → DerivedEdgeFact
              │                                │
   polint.calls provider          polint.solver provider (policy #4)
       call_targets                            │
              │                    polint.refined_calls provider
              │                       refined_call_edges
              └───────────────┬────────────────┘
                              │
            eval/external/jelly_callgraph.rs adapter
            (scores call_targets ∪ refined_call_edges;
             reachability roots per algorithm — §2.5)
```

**Key decisions, with reasons:**

1. **New solver policy, not a rewrite.** Implement `JsPointsToPolicy` behind the
   existing `SolverPolicy` trait (`analysis/solver/policy.rs`) and register it in
   `solver_policies_for_db` (`analysis/solver/provider.rs:141`). This is the
   proven integration pattern (GoRtaPolicy, TsTokensPolicy, TsObjectModelPolicy
   already do exactly this), and policy edges already flow to the benchmark via
   `refined_calls` → `db.refined_call_edges()` → the jelly adapter (line 250).
   The kernel registers `polint.solver` before `polint.refined_calls`
   (`analysis_kernel/mod.rs:615,638`), so ordering is solved.

2. **Constraint generation walks the AST, not the MIR.** The TS MIR lowering is
   call-site-centric — many constructs lower as `Unsupported` (tagged templates
   did until iteration 46), and property semantics live in `Place` projections
   that don't carry the JS-specific details (getters, prototype writes,
   computed keys). The 11k-line `ts_value_flows.rs` walkers already know how to
   decode every relevant AST shape; the constraint generator reuses that
   *knowledge* (and where practical, its helper functions: `callee_identifier`,
   `argument_expression`, `expression_aliases_exports`,
   `prototype_member_assignment_name`, `constant_strings_from_expression`, …)
   while emitting constraints instead of resolving in place.

3. **Recognizers stay on.** The heap is additive. Both sources emit edges; the
   calls/refined pipeline already dedupes by stable key. Recognizer paths are
   retired one family at a time only after the heap demonstrably covers them
   (Phase 5), never preemptively.

4. **Fork, don't generalize, the existing Andersen solver.** `points_to/solver.rs`
   (526 lines) is the reference: same worklist shape, same field-sensitive
   `object_slots` trick. But its budgets (10k steps, 64 objects/var, 512 dynamic
   vars) and its constraint vocabulary are tuned for intraprocedural use. A JS
   heap needs call/return wiring, `this` cells, prototype edges, and ~100×
   larger budgets. Copy the ~500 lines into `solver/js_points_to/fixpoint.rs`
   and evolve freely rather than parameterizing a shared core — the two will
   diverge (prototype chains, accessor dispatch) and a shared abstraction would
   fight us.

### 2.2 Module layout

```
crates/polint/src/analysis/solver/js_points_to/
├── mod.rs          // docs + public surface (≈50 lines)
├── tokens.rs       // Token, TokenId, allocation-site interning (≈150)
├── cells.rs        // Cell, CellId, interning, display-for-debug (≈250)
├── constraints.rs  // Constraint enum + ConstraintSet (≈150)
├── harvest.rs      // AST → constraints: the big walk (≈1,500–2,500, grows by phase)
├── natives.rs      // built-in models: forEach/then/call/apply/bind/assign… (≈400–800)
├── modules.rs      // require/import wiring via oxc_resolver (reuse
│                   // module_resolution_map from ts_value_flows) (≈150)
├── fixpoint.rs     // worklist solver, forked from points_to/solver.rs (≈600–800)
├── dispatch.rs     // solved cells × call sites → DerivedEdgeFact (≈250)
├── inputs.rs       // JsPointsToInputs::from_db — orchestrates harvest (≈200)
└── tests.rs        // unit + real-kernel gate tests (grows continuously)
```

Plus three one-line-ish integrations: register the policy
(`solver/provider.rs`), a `SolverBudget` section (`solver/budget.rs`), and a new
`CallAlgorithm::PointsTo` already exists in `calls/facts.rs` — reuse it for the
edge label (it is currently unused; verify nothing special-cases it).

### 2.3 Core abstractions (mirroring Jelly's, scoped down)

**Tokens** — abstract values, interned to dense `TokenId(u32)`:

```rust
enum Token {
    Function(FunctionId),               // every FunctionFact (named, arrow, method)
    Object(AllocSite),                  // object literal / new-expression result
    Array(AllocSite),
    Class(FunctionId),                  // class as a value (constructor identity)
    Module(FileId),                     // a module's export object
    Promise(AllocSite),                 // v1: coarse; refine in P3 if FP appear
    Unknown(UnknownKind),               // budget-overflow / unmodeled native — never dispatched
}
struct AllocSite { file: FileId, span_start: u32 }   // allocation-site abstraction
```

**Cells** — places tokens live, interned to dense `CellId(u32)`:

```rust
enum Cell {
    Var { scope: ScopeId, name: SmolStr },        // lexical binding (per function scope)
    Prop { object: TokenId, name: SmolStr },      // field-SENSITIVE: per object token
    PropWildcard { object: TokenId },             // the iter-52 wildcard lane, generalized:
                                                  // computed writes with unresolvable keys
    Return(FunctionId),
    This(FunctionId),
    Param { function: FunctionId, index: u16 },
    Rest { function: FunctionId },
    Arguments(FunctionId),
    Expr { file: FileId, span_start: u32 },       // intermediate expression results
    ModuleExports(FileId),
    ProtoOf(TokenId),                             // the prototype link cell (v1: merge-only)
}
```

Field sensitivity (`Prop` keyed on the *token*, not the name alone) is what
distinguishes this from the existing field-based `ts_tokens` policy and is
non-negotiable — it's where the precision comes from.

**Constraints** — the program, compiled:

```rust
enum Constraint {
    Alloc   { token: TokenId, into: CellId },           // {t} ⊆ cell
    Subset  { from: CellId, to: CellId },               // cell ⊆ cell
    Load    { base: CellId, name: SmolStr, into: CellId },  // x = y.f  (deferred: per token in base)
    Store   { base: CellId, name: SmolStr, from: CellId },  // y.f = x
    LoadDyn { base: CellId, into: CellId },             // x = y[unknown] → wildcard read
    StoreDyn{ base: CellId, from: CellId },             // y[unknown] = x → wildcard write
    Call    { site: CallSiteId, callee: CellId,
              args: Vec<CellId>, this_arg: Option<CellId>, result: CellId },
    ProtoLink { object: CellId, proto: CellId },        // C.prototype = …, __proto__, class extends
}
```

`Load`, `Store`, `Call`, and `ProtoLink` are *deferred* constraints: they re-fire
per token that arrives in their base/callee cell (Jelly's "token listener"
pattern; the existing `points_to/solver.rs` does the same with `field_loads`).

### 2.4 The solver (fixpoint.rs)

Standard delta-driven worklist, copied structurally from `points_to/solver.rs`:

- `sets: Vec<SmallTokenSet>` indexed by `CellId` (dense vecs, not BTreeMaps —
  this graph is 100× bigger than the existing solver's use case).
- `queue: VecDeque<(CellId, delta)>`; pop, push delta along subset edges,
  fire listeners for deferred constraints on new tokens.
- **Call listener** (the heart): for each `Token::Function(f)` newly in a callee
  cell → add `Subset(arg_i → Param{f,i})`, `Subset(Return(f) → result)`,
  `Subset(this_arg → This(f))`, record `(site, f)` as a resolved edge.
  For `Token::Class(c)` at a `new` site → same, plus result gets the instance
  token. Constructor-return-override (`return x` from a constructor) falls out
  naturally: `Return(f)` tokens win in the result cell — one of the previously
  "bespoke" tail cases that the heap closes for free.
- **Load listener**: token `t` arrives in `base` → `Subset(Prop{t,name} → into)`;
  if `Prop{t,name}` never written, chase `ProtoOf(t)` (bounded depth 8); getters
  in v1: route the getter's `Return` cell into `into` (accessor dispatch).
- **Prototype model v1 = merge-on-link** (same semantics as today's recognizers,
  same accepted FP profile; a replaceable proto slot is explicitly out of scope).
- **Budgets** (in `SolverBudget`, surfaced honestly like every other policy):
  `max_steps` ≈ 2M, `max_tokens_per_cell` ≈ 64 (overflow → cell poisoned with
  `Unknown`, which never dispatches — prefer recall loss to FP explosion),
  `max_cells` ≈ 1M, wall-clock ceiling. On any budget hit: report
  `BudgetStatus::Exhausted` with reasons; edges already derived stay valid.
- **Determinism**: dense IDs assigned in deterministic harvest order (files
  sorted by `FileId`, AST in source order); worklist is FIFO; token sets are
  sorted on output; no hashing iteration anywhere. Same rules that keep the
  benchmark hash stable today.

### 2.5 Scoring and the reachability discipline (the iteration-53 lesson)

The jelly adapter seeds reachability roots from `FunctionTokenFlow` callers
(`jelly_callgraph.rs:246`) — "a body the value flow executed." Points-to edges
are derived from a *global* fixpoint, not from walking an invoked body, so they
carry the same hazard that produced the +48 FP in iteration 53: a dead
function's internal edges must not make it reachable.

Rule: **`js_points_to` edges never seed reachability roots.** They label as
`CallAlgorithm::PointsTo`; the adapter treats them like `ThisMethodFlow` —
present in the adjacency (so they propagate and un-prune), never roots. Roots
remain module-execution functions + genuinely-executed bodies. One adapter
change, one focused test (extend
`reachability_prunes_dead_code_but_keeps_invoked_callbacks` with a points-to
case). The only true new roots a fixpoint could justify are host-invoked
callbacks, and those already work via the recognizers' `FunctionTokenFlow`.

---

## 3. Implementation phases

Each phase ends with: gate tests green → full lib suite green → release
benchmark run → numbers + hash logged in the iteration log → keep-or-revert.
Never carry a phase that moves nothing (the iteration-37/56 rule).

### Phase 0 — Spike: prove the pipe (≈1 day)

Hardcode a `JsPointsToPolicy` that emits one fabricated `DerivedEdgeFact` for a
known fixture call site. Run the benchmark. **Confirm**: (a) the solver provider
executes in the benchmark's empty-plan path, (b) the edge appears in
`refined_call_edges`, (c) the jelly adapter scores it, (d) reachability treats
it per §2.5 after the adapter tweak. If (a) fails (provider skipped under the
empty plan — the module-graph gotcha), fallback is invoking the policy from the
calls provider exactly where `resolve_ts_value_flow_targets` is invoked
(`calls/provider.rs:41`); decision recorded, then delete the fabricated edge.

*Deliverable: a one-paragraph note in the iteration log naming the integration
path. Risk retired: the single biggest unknown.*

### Phase 1 — Skeleton: tokens, cells, solver, direct flow (≈1 week)

Harvest the minimal constraint set: function declarations/expressions/arrows
(`Alloc`), variable bindings and assignments (`Subset`), object literals
(`Alloc` + `Store` per property), static property reads/writes (`Load`/`Store`),
direct and member calls (`Call`), returns (`Subset` into `Return`). Single-file
only. Fork the solver; wire dispatch → `DerivedEdgeFact` with stable keys
(follow `stable_refined_call_key_from_solver_edge` conventions); spans must use
the **existing normalized span helpers** from `ts/spans.rs` (parenthesized-call
and tagged-template lessons — do not re-derive spans).

*Gate tests:* one unit test per constraint kind on hand-built constraint sets
(no AST), plus real-kernel tests: `js_pt_resolves_var_function_call`,
`js_pt_resolves_object_property_call`, `js_pt_resolves_returned_function_call`,
`js_pt_depth_chain` (the canonical 5-hop depth case: function returns object →
stored → property read → passed as arg → returned → called).
*Benchmark expectation:* small positive or neutral movement on `simple`/`obj2`;
**zero FP regression** (G3). The depth fixtures won't fully move until P2.

### Phase 2 — Core JS semantics (≈1–2 weeks; the long pole, part 1)

In rough priority order, each its own commit + gate test:

1. **Closures**: inner functions' free variables resolve to enclosing-scope
   `Var` cells (scope chain at harvest time — cells are per-scope, so capture is
   just naming the right cell; no runtime machinery). This closes the
   `client1`-style param-capture case that was never built.
2. **Calls binding `this`**: `recv.m(...)` wires `this_arg` = recv's cell; bare
   calls wire nothing (v1: no global-object `this`).
3. **`new`**: instance token; constructor `this` = instance; `this.x = …` in the
   constructor stores into `Prop{instance, x}`; explicit object return
   overrides (falls out of `Return` wiring).
4. **Prototypes**: `C.prototype.m = f` → `Store` on the class's prototype
   object token; instance `Load` misses chase `ProtoOf`; `class extends` →
   `ProtoLink`. Port the iter-54 semantics.
5. **Destructuring + rest/spread/defaults**: lower patterns to
   `Load`s/`Subset`s at harvest (reuse `ParamPattern` decoding logic).
6. **Computed keys**: resolvable-to-constant keys (reuse
   `constant_strings_from_expression` logic env-free) → named `Store`/`Load`;
   unresolvable → `StoreDyn`/`LoadDyn` (wildcard). for-in/forEach-style writes
   land here, generalizing iterations 51–52.
7. **Accessors**: getter `Load` routes return cell; setter `Store` routes into
   param cell.

*Benchmark expectation: this is where G1 (the ~130 depth FN) substantially
closes. Track the named depth-bucket cases (`simple`, `more1`, `obj2`,
`library`, `dynamic`, `arrays5`, `srcLoc` partially) per iteration.*

### Phase 3 — Modules + natives (≈1 week; long pole, part 2)

1. **Modules**: reuse `module_resolution_map` (oxc_resolver — the benchmark's
   empty plan never populates `resolved_imports`; this is the iter-35 lesson
   encoded in memory). `module.exports` / `exports` / export-alias chains →
   `ModuleExports(file)` cell; `require('x')` / import bindings → `Subset` from
   the target's `ModuleExports`. Replaces the bounded 4-round summary fixpoint
   with the global one — re-export chains just propagate.
2. **Natives** (`natives.rs`), v1 set chosen by benchmark evidence:
   `Object.assign`/`create`/`defineProperty`/`defineProperties` +
   `merge-descriptors` (port iter-47/48/52 semantics as constraint emitters);
   array element cells (one summary element cell per array token — Jelly-style
   blurring *within* the heap only) + `push`/`map`/`filter`/`forEach` callbacks
   (callback `Call` constraints with element cells as args);
   `Function.prototype.call/apply/bind` (port iter-31);
   `Promise.then/catch/finally` v1 = fulfilled-value cell per promise token.
3. **Test-framework + interop wrappers**: `describe/it/...` invoke their
   callback argument (port iter-55); `__importDefault`/`__importStar` identity.

*Benchmark expectation: helloworld dependency tail starts moving (`depd`,
`debug`, `http-errors`); G2 becomes reachable: `createApplication`'s return now
carries the mixin-merged app via pure constraint flow, and the route-handler
model (one small native hook: `app.get(path, handler)` registers `handler` as
called with `(req-token, res-token)` from `app.request`/`app.response` proto
cells) resolves `res.send`. The handler hook is the single
framework-specific piece; it's ~30 lines and gated by its own fixture.*

### Phase 4 — Alignment, precision, performance (≈1 week, iterative)

- **Caller attribution**: edges attribute to Jelly's nodes (class node for
  constructor-body calls — the reverted-experiment lesson; reuse the
  caller-override conventions from the recognizers when mapping dispatch
  results to `DerivedEdgeFact.caller`).
- **Precision sweep**: diff every FP the heap introduces; classify (smearing
  via over-wide cells? missing shadowing? span mismatch?); fix or constrain.
  The wildcard lane must keep the "explicit property wins" rule from iter-52.
- **Performance**: helloworld (81 files) must solve within the benchmark's
  runtime envelope (current total ≈ 90–105 s; budget the policy ≤ 20 s). Knobs:
  token-set smallvec, cycle collapsing (port Nuutila SCC from Jelly's design or
  the simpler online merging in `points_to/solver.rs`), per-cell caps.
- **Budget honesty tests**: synthetic blow-up program → policy reports
  exhausted, emits partial edges, no panic, deterministic.

### Phase 5 — Migration and retirement (ongoing, optional)

For each recognizer family the heap demonstrably covers (benchmark identical
with the recognizer disabled): delete the recognizer path, keep its gate tests
pointed at the heap. Start with module export summaries and the
function-return summaries (the heap subsumes both by construction). Never
delete ahead of evidence. End state: `ts_value_flows.rs` shrinks to the
harvest helpers + the genuinely syntactic recognizers (spans, direct calls).

---

## 4. Verification and testing

Five layers, cheapest first. All run in `cargo test -p polint --lib`.

**L1 — Solver unit tests (no AST).** Build `ConstraintSet`s by hand; assert
solved token sets per cell. One test per constraint kind; plus: subset cycles
terminate; token-cap poisoning works and `Unknown` never dispatches; FIFO
determinism (solve twice, byte-equal output).

**L2 — Harvest snapshot tests.** Small JS source → harvested constraints
rendered as sorted strings → assert exact set. Catches silent harvest
regressions without running the solver. One per Phase-2/3 feature.

**L3 — Real-kernel gate tests** (the iteration-52–56 pattern: `db_with_file` /
tempdir multi-file, full `ts::analyze` + MIR + extraction, then assert
`(site, target)` pairs). Every phase feature gets one, named `js_pt_*`. Critical
named ones: `js_pt_depth_chain`, `js_pt_closure_param_capture` (client1 shape),
`js_pt_cross_file_mixin_chain` (must reproduce iteration 52's chain through
pure constraints), `js_pt_route_handler_res_send` (G2 fixture),
`js_pt_dead_function_edges_stay_pruned` (G5 of §2.5 — a never-called function
whose interior the heap resolves must contribute zero scored edges).

**L4 — Differential tests vs the recognizers.** For a corpus of fixtures the
recognizers already resolve, assert the heap's edge set is a **superset modulo
an explicit allowlist** of known divergences. Guards against the heap silently
losing covered ground during Phase-5 retirement; the allowlist documents every
intentional difference.

**L5 — The benchmark protocol** (unchanged from iterations 52–56):

```
POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release \
  cargo test --release -p polint --lib \
  eval::external::tests::external_graph_baseline_reports_can_be_generated \
  --locked -- --nocapture
```

Per kept iteration, append to `2026-06-06-jelly-gap-closure-research.md`:
TP/FP/FN, P/R/F1, runtime, output hash, per-case movers, and what was reverted.
`.context/phase-a/track.py` (extended with the depth-bucket case list and the
G2 edges) is the per-iteration dashboard. Keep-or-revert rules: precision
< 0.94 → fix or revert; byte-identical hash → revert (the iteration-56 lesson:
two no-op mechanisms were reverted on exactly this evidence); per-case wins
must be explainable, unexplained movement = investigate before keeping.

**Soak**: after Phase 3, run the policy on the three largest repos in the eval
harness with budgets enabled; assert no panic, budget reasons surfaced,
wall-clock within ceiling. (This anticipates the known polint memory-scaling
concern — budgets are the contract.)

---

## 5. Risks and mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Solver provider skipped under benchmark's empty plan | medium | Phase 0 spike; fallback = invoke from calls provider (1-line, precedented by ts_value_flows) |
| FP explosion from token smearing | medium-high | per-cell caps + `Unknown` poisoning (recall loss over FP); §2.5 root rule; G3 floor enforced per iteration |
| Reachability inflation (the iter-53 trap, fixpoint edition) | high if ignored | `PointsTo` edges never seed roots — designed in from day one, with a dedicated gate test |
| Span/attribution mismatches with Jelly | certain (small) | reuse `ts/spans.rs` normalizers + recognizer attribution conventions; expect a short iter-38-style alignment pass in Phase 4 |
| Performance on 81-file helloworld | medium | dense IDs + deltas + SCC collapsing; 20 s policy budget; measured every benchmark run anyway |
| Effort creep in natives | medium | natives are added only on benchmark evidence (a named FN bucket), never speculatively |
| Determinism leaks (hash instability) | low | no hash-iteration, FIFO worklist, sorted outputs; L1 determinism test + G5 |

## 6. Effort summary

| Phase | Duration | New code (approx) |
|---|---|---|
| P0 spike | 1 day | throwaway |
| P1 skeleton | ~1 week | ~1,800 lines |
| P2 core semantics | 1–2 weeks | ~1,500 lines |
| P3 modules + natives | ~1 week | ~1,200 lines |
| P4 alignment/perf | ~1 week | ~400 lines + tuning |
| **Total to G1–G5** | **4–6 weeks** | **~5,000 lines + tests** |

Reused outright: solver engine + policy trait + budget framework (+ the
benchmark/eval harness, span normalizers, resolver map, and the recognizers'
decoded knowledge of every JS construct). The expensive lessons — empty-plan
module resolution, span conventions, caller attribution, reachability
discipline, keep-or-revert hygiene — are already paid for and encoded above.
