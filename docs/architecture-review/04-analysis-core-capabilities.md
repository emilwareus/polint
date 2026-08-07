# 04 — Analysis Core Capabilities

**Scope:** `crates/polint/src/analysis/` (~112k LOC, 22 submodules) — the algorithms themselves:
precision, soundness, and algorithmic ceiling. Measured against the stated bar (CodeQL / Infer /
Joern / Semgrep Pro / Sourcegraph).

**Method:** direct reading of algorithm code, not doc comments. Doc comments in this repo are
unusually high-quality and unusually *aspirational* — several describe machinery that the
implementation below them does not contain. Every claim here cites `path:line` of executable code.

**Date:** 2026-07-28. **Verdict in one line:** world-class *engineering discipline* wrapped around
an analysis core that is roughly Semgrep-tier, not CodeQL-tier — with two genuinely principled
components (an Andersen points-to solver, a lattice/abstract-domain kernel) marooned inside a
pattern-recognizer-driven pipeline that cannot express interprocedural taint.

---

## (a) Capability matrix

Sensitivity legend: **Flow** = per-program-point facts; **Ctx** = call-site/context sensitivity;
**Field** = per-field/per-access-path. `—` = not applicable.

| Analysis | Algorithm actually implemented | Flow | Ctx | Field | Soundness posture | Evidence |
|---|---|---|---|---|---|---|
| **JS/TS call graph — recognizers** | Hand-written AST abstract interpreter over a name-keyed environment; ~163 methods of per-idiom recognizers | partial (AST order) | inlining, depth-capped | by name | unsound + unbounded over-approx; silent drops | `calls/ts_value_flows.rs:116-186`, `:6881-6927` |
| **JS/TS call graph — points-to** | **Andersen inclusion-based**, delta worklist, lazy property cells, prototype chains | **no** (flow-insensitive) | **none** | **yes** (cell keyed on `(token, field)`) | honest: budget latches + reports | `calls/js_points_to/solver.rs:93-133`, `:441-454`, `:397-413` |
| **JS/TS call graph — composition** | Set **union** of 3 independent resolvers, deduped by stable key | — | — | — | union of unsound + sound = unsound | `calls/provider.rs:37-48` |
| **Go call graph** | **RTA** (CHA ∩ instantiated types) — *and* real `go/ssa` + `x/tools/callgraph/rta` in an out-of-process sidecar | no | none | — | honest: no method-set match ⇒ no edge | `solver/go_rta/mod.rs:36-48`, `go-sidecar/polint-go-frontend/internal/semantic/emit.go:18-19,263` |
| **Generic points-to** | Andersen (2nd implementation), `max_steps` **10 000** | no | none | yes | budget-honest, but budget is toy-sized | `points_to/solver.rs:23-25`, `:81-92` |
| **Unified solver core** | Worklist over `ConstraintKind`, driven by a `SolverPolicy` trait (opaque black-box policies) | — | — | — | `BudgetStatus` + 23 typed reasons, folded into cache key | `solver/engine.rs:79-129`, `solver/policy.rs:79-85`, `solver/budget.rs:174-189` |
| **Abstract interpretation** | Real `trait AbstractDomain` (bottom/top/leq/join/widen) + worklist solver | **yes** | **intraprocedural only** | no | **best-in-repo**: `Top(TopReason)` is a real lattice element | `domains/lattice.rs:71-97`, `domains/solver.rs:88-192` |
| Abstract domains (6) | Reachability, **Nilness**, Truthiness, Constant, String, Initializedness — all flat/finite | yes | no | no | saturate to `Top(BudgetExceeded)` at `LITERAL_SET_CAP=4` | `domains/core.rs:102,157,212,267,328,389`; `:7` |
| **Interprocedural dataflow** | **Graph reachability (BFS path enumeration)** over a prebuilt edge set — *not* IFDS/IDE | no | **none — unrealizable paths possible** | via `FieldProjection` edges only | statuses `Unknown`/`BudgetExceeded` are surfaced as results | `data_flow/query.rs:42-140`, `:217-221` |
| **Function summaries** | Bottom-up over SCC condensation; payload = param→return reachability triples | — | — | **no** | `SummaryStatus` tracked | `summaries/facts.rs:131-165`, `summaries/scc.rs` |
| **Taint tracking** | **Absent as an analysis.** Sources modeled; sinks/propagators/sanitizers are unused enum variants | — | — | — | n/a | see §(a).1 below |
| **CFG** | Real basic blocks; **branch shape chosen by substring-matching source text**; branch arms structurally empty | — | — | — | `UnsupportedSemanticFact` taxonomy is honest | `cfg/builder.rs:248-304`; `cfg/lower_ts.rs:244-266`; `:172-241` |
| **Dominators** | Iterative set-intersection (Allen–Cocke), `BTreeSet` order not RPO; post-dominators via virtual exit | — | — | — | correct algorithm, wrong input graph | `cfg/derived.rs:292-356`, `:358-379` |
| Control dependence | Ferrante–Ottenstein–Warren ipdom walk | — | — | — | only derived consumer of dominance | `cfg/derived.rs:163-216` |
| Dominance frontiers / SSA / φ | **Absent** | — | — | — | — | zero hits repo-wide |
| Natural loops / loop nesting | **Absent** — `LoopBack` is stamped syntactically, never discovered | — | — | — | drives widening fuel ⇒ unlabeled loops don't widen | `cfg/lower_ts.rs:190-191`; `domains/solver.rs:228-242` |
| Exceptional edges | Labels only: one synthetic `FinallyEnter` node; no handler blocks; no implicit throw; Go `defer` never runs at exit | — | — | — | unsound silently | `cfg/lower_ts.rs:309-314`; `cfg/facts.rs:157` |
| **MIR** | 9 op kinds; **no arithmetic, no comparisons, no operators**; untyped; flat op list, no blocks | — | — | — | ~40 constructs recorded as `Unsupported` with conservative action | `mir/op.rs:21-57`; `mir/lower_ts.rs:1555-1563` |
| Branch predicates | `predicate_place_key: None` **hardcoded** in both frontends | — | — | — | path-sensitive refinement structurally dead | `mir/lower_ts.rs:2535`, `mir/lower_go.rs:857` |
| Access paths | Proper projection vocabulary + `depth` for k-limiting — **but summaries don't use it** | — | — | yes | `AccessPathStatus` | `access_paths/facts.rs:7-30` |
| **Trust boundaries (taint sources)** | 14 typed source kinds derived from framework entrypoint recognizers | — | — | — | precision inherited from entrypoint | `entrypoints/facts.rs:122-137`; `entrypoints/trust_boundaries.rs:17-24` |
| Program slicing | Backward/forward/**chop**/path modes over an evidence graph, with `PathOmittedRegion` accounting | — | — | — | **exemplary** — every truncation records what was hidden | `evidence/facts.rs:217-225`; `slicing/paths.rs:126-188` |
| Unknown taxonomy | Pure projection of statuses already on facts; only consumer is the CLI | — | — | — | **cosmetic** | `unknown_taxonomy/collect.rs:105-427`; `cli/mod.rs:2296-2375` |
| Path sensitivity / SMT | **Absent.** No solver, no path conditions | — | — | — | — | zero hits for `z3`/`smt`/`PathCondition` |
| Symbolic execution | **Absent** | — | — | — | — | — |
| Concurrency / race analysis | **Absent** | — | — | — | — | — |
| Context sensitivity (any form) | **Absent everywhere.** No k-CFA, no call strings, no object sensitivity, no heap cloning | — | — | — | — | zero hits for `k-cfa`/`call.string`/`context.sensitiv`/`object.sensitiv` |

### (a).1 The taint finding, stated precisely

The engine has a rule-facing source→sink API (`DataFlow::forbidden(FlowQuery)`,
`sdk/facts.rs:921`) and the full vocabulary — `DataFlowNodeKind::{Source, Sink, Sanitizer, Barrier}`
(`data_flow/facts.rs:88-91`), `FlowKind::{Taint, Barrier, Sanitizer}` (`summaries/facts.rs:131-137`),
`EvidenceEdgeKind::DataTaint` (`evidence/facts.rs:203`).

**No producer emits any of them.** Repo-wide, `FlowKind::Taint` occurs exactly once — in the test
asserting the variant exists, named `flow_kind_has_five_variants_including_future_taint_barrier_sanitizer`
(`summaries/facts.rs:253`). `DataFlowNodeKind::Sink` and `::Sanitizer` occur once each, both in
`match` arms that consume them (`evidence/provider.rs:299-300`). `EvidenceEdgeKind::DataTaint` has
two consumers and zero producers (`slicing/local.rs:240`, `slicing/paths.rs:528`). The summary
builder only ever writes `FlowKind::Value` and `FlowKind::BySideEffect`
(`summaries/builder.rs:809,823,1344,1668`).

The project's own roadmap agrees: "Taint / source–sink tracking" is listed **Planned**
(`docs/ANALYSIS-ROADMAP.md`, Planned block).

What *does* exist and is genuinely valuable: `TrustBoundaryFact` gives 14 typed untrusted-input
kinds (`PathParam`, `QueryString`, `RequestBody`, `RequestHeader`, `Cookie`, `CliArgs`, `EnvVar`,
`Stdin`, `QueuePayload`, `McpArguments`, …) derived from 3 900 LOC of framework entrypoint
recognizers. **The source side of taint is built. The propagation and sink sides are not.**

### (a).2 The dataflow query is reachability, not IFDS

`find_paths` (`data_flow/query.rs:42-140`) is a BFS over `DataFlowEdgeFact`s with a `visited` set.
`PathFrame` is `{ node, edges, visited }` (`:217-221`) — **there is no call stack**. Summary TITO
edges connect a synthetic `SummaryInput` to a `SummaryOutput` keyed on the *function*, shared by all
call sites (`data_flow/summary_edges.rs:36-70`). Therefore a path may enter a callee from call site
A and exit toward call site B: **unrealizable paths are reportable**. This is precisely the defect
IFDS's balanced-parenthesis property exists to eliminate, and it is the single largest precision gap
in the dataflow layer.

Sanitizers are applied as a **post-hoc filter on an already-found path**
(`barrier_covers_path`, `policy_queries.rs:90`), not as flow-killing transfer functions — so a
sanitizer on one branch suppresses a genuine finding on another.

Defaults: `FlowQuery::max_depth = 8`, `max_paths = 20` (`sdk/policy.rs:333-334`). Eight
interprocedural hops is below the depth at which real vulnerability chains live.

---

## (b) Verdict: is the call-graph / points-to core principled or heuristic?

**Both, in a way that is worse than either alone.** The call graph is the set **union** of three
resolvers with three different and mutually inconsistent semantics
(`calls/provider.rs:37-48`), deduped by stable key. There is no single soundness or precision
contract for the resulting graph — only a per-edge `CallAlgorithm` tag naming which of the 18
producers emitted it (`calls/facts.rs:123-148`).

**`js_points_to/solver.rs` is principled.** It is a genuine Andersen inclusion-based solver:
a closed `Constraint` vocabulary (`Alloc`/`Subset`/`FieldStore`/`FieldLoad`/`Call`/`Construct`/
`Inherit`, `:93-133`), token-listener deferred constraints, lazily-minted per-`(token, field)`
property cells (`:397-413`), a delta worklist run to fixpoint (`:441-454`), and honest budget
latching (`:385-395`, `:429-439`). It is field-sensitive, context-insensitive, flow-insensitive —
a textbook Andersen. ~940 LOC, unit-tested on hand-built constraints, decoupled from the AST. This
is real work and it should be the foundation.

**`ts_value_flows.rs` (11 898 LOC) is not a points-to solver. It is a pile of pattern recognizers.**
This was the key question; the evidence is unambiguous:

1. **No heap, no allocation sites, no aliasing.** The abstract state is `FlowEnv`
   (`:6881-6927`): **12 parallel `BTreeMap<String, …>` keyed on variable *name*** —
   `bindings`, `objects`, `class_bindings`, `string_bindings`, `bool_bindings`, `string_arrays`,
   `promises`, `async_functions`, `async_generators`, `async_iterators`, `generator_sequences`,
   `iterator_sequences`, plus `merge_helpers`, `forin_keys`, `object_prototypes`, `absent`. That is
   a sum type flattened into parallel maps — the canonical signature of ad-hoc accretion.
   `ObjectTargets` is `Clone`d by value (`:7157-7170`), so it is a *structural copy*, not a heap
   reference: `let b = a; b.f = fn; a.f()` cannot work in general. Object identity is the variable
   name.
2. **Transfer functions are string matches on syntax.** `collect_native_object_call` is
   `match callee_text(&call.callee) { Some("Object.assign") => …, Some("Object.defineProperty") => …,
   Some("Object.setPrototypeOf") => …, Some("Object.defineProperties") => …, _ => {} }`
   (`:3521-3620`) — and every arm requires the first argument to be a **bare identifier**
   (`expression_identifier`) which it then looks up **by name** in `env.objects`. So
   `Object.assign(this.x, y)` and `Object.assign(getTarget(), y)` are silently no-ops.
3. **Named npm packages are hardcoded.** `is_merge_descriptors_require` special-cases the
   `merge-descriptors` package (`:3406`, `:3410`) — that package exists in the recognizer because
   Express uses it to assemble `app`, and Express is ~47% of the benchmark's false negatives.
   `is_test_framework_global` (`:7790`) hardcodes `it`/`xit`/`fit`/`describe`/`xdescribe`/`suite`/
   `specify`/`setup`/`teardown`/`suiteSetup`/`suiteTeardown` (mocha + jest).
4. **No fixpoint.** The only convergence loop is `for _round in 0..MAX_MODULE_SUMMARY_ROUNDS` with
   `MAX_MODULE_SUMMARY_ROUNDS = 4` (`:31`, `:313`) — a fixed 4 rounds, with no convergence check and
   no non-convergence signal. Recursion is bounded by scattered magic numbers: `depth > 8` in nine
   places, `invocation_depth > 16` in three (`:639`, `:1323`, `:1644`, `:1681`, `:3789`, `:4103`,
   `:4268`, `:4323`, `:4441`, `:5228`, `:5619`, `:5697`, `:5816`, `:5856`).
5. **Silent unsound truncation.** `properties.truncate(8)`, `values.truncate(8)`,
   `names.truncate(8)` (`:2662`, `:2710`, `:2919`, `:8458`) drop value-set members with nothing
   recorded. In the companion harvester, four bare `return`s at `MAX_HARVEST_DEPTH = 256`
   (`js_points_to/harvest.rs:196,390,432,1241`) return a *fresh empty cell*, indistinguishable from
   a genuinely empty one.

**The decisive evidence is the benchmark itself.** With the reachability filter on, the suite scores
1219 TP / 49 FP / 260 FN — 96.1% precision. With `POLINT_JELLY_NO_PRUNE=1` (filter bypassed) the
same resolvers score **1295 / 1137 / 184** (`performance/2026-06-17-jelly-fn-categorization-and-wins.md:27-28`)
— **precision 53.2%**. The resolver emits 1 137 false edges; a post-hoc reachability filter, written
by the project and tuned to mimic Jelly's demand-driven behavior, removes ~96% of them. **The
headline precision is a property of the filter, not of the analysis.** A principled points-to
solver does not need a 23× false-edge filter bolted downstream.

The project's own internal review reaches the same conclusion independently and states it more
bluntly than this document does: *"the recognizer layer increasingly reverse-engineers the 76-fixture
Jelly oracle … benchmark modeling, not language modeling"*
(`research/static-analysis-2.0/00-critical-review.md:64-66`).

**Go is the counter-example and the model to copy.** `polint-go-frontend` shells out to the real
`go/types` + `go/ssa` + `x/tools/go/callgraph/rta`
(`crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go:17-21,263`). It reuses the
language's own frontend instead of reimplementing it, and gets a correct RTA call graph for ~200 LOC
of glue. That is the right instinct, applied to one of two languages.

### (b).1 Evidence quality — read this before trusting any number

- All headline accuracy comes from **one 76-fixture micro-suite** whose oracle is another tool's
  committed JSON (`eval/external/jelly_callgraph.rs:309-341`). The adapter's own limitation string
  says it "scores edge agreement with that oracle rather than executing Jelly itself" (`:336`).
- The Jelly clone is git-ignored, results are never committed
  (`research/evaluation-harness/baselines/persisted-graph-accuracy.json` commits
  `"recall": null, "precision": null, "graph_edges_expected": 0`), and the external benchmark test
  **silently no-ops when the clone is missing** (`eval/external/mod.rs:27-29`) — which is the CI case.
- **There is no accuracy regression gate in CI.** `.github/workflows/ci.yml` gates determinism,
  public-surface leak, polyglot canary, and store RSS/wall-clock — no F1/precision floor anywhere.
- The latest quoted figures (89.06% F1 / 97.05% P) appear only in a planning doc with no measurement
  log; the last figure with a documented run behind it is 88.75% F1 / 96.14% P.
- There is an unresolved internal contradiction about whether the oracle is dynamic execution traces
  or Jelly's static output (`jelly_callgraph.rs:336` vs
  `performance/2026-06-15-jelly-fn-decomposition.md:8-10`).
- Go's "92.50% F1 / 100% recall" is **37 edges across 5 fixtures** against an admittedly partial
  oracle.
- **Zero** measured numbers exist for taint, points-to precision, or any competitor comparison.
  OWASP Benchmark was explicitly removed; no Juliet, SecuriBench, PointerBench, or DroidBench.
  `gosec-samples` and `secbench-js-smoke` adapters exist with no recorded scores.

### (b).2 Termination and scale

- Budgets are pervasive, typed, and folded into cache keys so a truncated run can never share a key
  with a complete one (`solver/budget.rs:174-189`, `:218-337`; `solver/provider.rs:205-206`). This
  is genuinely better than most commercial tools.
- **But there is no wall-clock timeout anywhere in `analysis/`** — only iteration counters. The only
  `Duration` deadlines in the tree guard the *extension subprocess* (`extensions/host.rs:19`) and
  the *Go sidecar* (`go/semantic/client.rs:59`).
- **No memory ceiling.** Peak RSS is measured for benchmarks (`eval/baseline.rs:45-55`), never
  bounded at runtime. The 30 GB+ OOM (`.planning/REQUIREMENTS.md:28`) was fixed architecturally
  (capability gating, rule-scoped discovery), not by a limit. It can recur.
- **No global resource envelope.** `DomainSolver::max_iterations = 10 000` is *per function body*
  (`domains/solver.rs:67`). 50 000 functions ⇒ 5×10⁸ iterations with no aggregate ceiling. This is
  the structural reason the engine can go superlinear without ever tripping a budget.
- Two fixpoints have **no iteration cap at all**: `cfg/derived.rs:313` and `summaries/scc.rs:241`
  (`while changed { … }`). Both terminate by monotonicity argument, not by construction.

### (b).3 What the CFG/MIR audit means for everything above

This is the part with the widest blast radius, because every downstream analysis reads these facts.

- **Branch shape is decided by substring-matching raw source text**
  (`cfg/lower_ts.rs:244-266`): `if contains_token(&evidence, &["for ", "while", …])
  { BranchShape::Loop }`. `operation_evidence` slices the source file bytes (`:346-349`). An `if`
  whose body contains a comment `/* while */` or a string `"for "` is classified as a **loop**. Go
  is identical (`cfg/lower_go.rs:245-254`). Go `panic`/`recover` detection is
  `text.contains("panic(")` (`mir/lower_go.rs:767-779`).
- **Branch arms and loop bodies are structurally empty.** `lower_branch`
  (`cfg/lower_ts.rs:172-241`) creates `then_block`/`else_block`/`body` containing exactly one
  *synthetic* node each, then calls `start_block(Join)` — so every subsequent real operation lands
  in the **join block**, i.e. *outside* the branch. Consequence: **no operation is control-dependent
  on any branch**. The Ferrante–Ottenstein–Warren control-dependence computation at
  `cfg/derived.rs:163-216` is textbook-correct and computes control dependence over a graph in which
  nothing is control-dependent on anything.
- **The MIR has no arithmetic and no comparisons.** Nine op kinds (`mir/op.rs:21-57`); `a + b`
  lowers to a bare anonymous temporary with the operator discarded and no `Assign` linking it to its
  operands (`mir/lower_ts.rs:1555-1563`). It is a def/use + call skeleton, not three-address code,
  and it is untyped (`places.rs:10-20`, `mir/op.rs:10-18` carry no type field;
  `analysis/types/` is never imported by `analysis/mir/`).
- `predicate_place_key: None` is hardcoded (`mir/lower_ts.rs:2535`, `mir/lower_go.rs:857`), so
  `branch_assumption` in the domain solver (`domains/solver.rs:480-501`) can only ever return
  `Some((None, sense))`. **Path-sensitive refinement is structurally dead**, independent of any
  future SMT work.
- No SSA, no φ, no dominance frontiers, no natural-loop detection, no loop nesting. Redefinitions
  overwrite the same `PlaceId` (one node per *name* per function, `places.rs:24-27`).
- Exception paths are labels, not paths: one synthetic `FinallyEnter` node with no handler block
  (`cfg/lower_ts.rs:309-314`); `CfgEdgeKind::ImplicitThrow` and `CfgNodeKind::RunDefers` are declared
  and never constructed — Go `defer` bodies **never execute at function exit**.

To be fair: the `UnsupportedSemanticFact` mechanism (`mir/op.rs:79-129`) records every lowering gap
with `construct`, `source_evidence`, `affected_domains`, and a `conservative_action`
(`SkipOperation` / `HavocAffectedPlaces` / `PreserveWithUnknownValue` / `StopLowering`). ~40 TS/JS
constructs are recorded this way. Most commercial tools silently lie here. This engine mostly does
not — which is exactly what makes it fixable.

### (b).4 Soundness posture: one real mechanism, one cosmetic one

**Real.** `domains/lattice.rs:71-97` defines `trait AbstractDomain` with
`bottom() / top(TopReason) / is_top() / leq / join / widen(site, fuel)`, and `TopReason` has 8
variants (`:34-44`) including `BudgetExceeded`, `DynamicWrite`, `UnresolvedCall`, `Widened`. This is
propagated correctly: budget exhaustion sets *every* block state to `Top(BudgetExceeded)`
(`domains/solver.rs:503-520`); joining two different `Top`s yields `Top(ConflictingFacts)`
(`domains/core.rs:535-540`). **Unsoundness as a first-class lattice element, done right.** This is
the best thing in the codebase and almost nothing uses it.

**Cosmetic.** `unknown_taxonomy/` (1 574 LOC) contains zero analysis logic. `collect.rs:105-427` is
pure projection: it walks fact tables and re-labels statuses that are already on the facts. Its only
callers in the entire crate are two CLI subcommands (`cli/mod.rs:2296-2375`). Deleting it would not
change a single analysis result.

**The gap that matters most for trust:** a rule cannot ask whether an answer was *complete*.
`RuleCtx`'s public surface (`core/mod.rs:7584-7643`) has no `unknowns()`, no `is_complete()`, no
`budget_status()`. `PolicyStatus::BudgetExceeded` rides on **emitted violations only**
(`sdk/policy.rs:143-150`) — so a rule that finds nothing receives an empty `Vec` and cannot
distinguish "provably clean" from "the solver blew its budget and never explored the path".
**Absence of evidence is silently presented as evidence of absence.** For a security engine, this is
the highest-severity soundness defect in the design, above any individual missing analysis.
(`PolicyConfidence` is worse: settable as a query filter at `sdk/policy.rs:184`, with no accessor on
the result.)

---

## (c) Missing analyses, ranked

Ranked by (distance to the stated goal) × (leverage on everything else). The first three are
prerequisites, not features — nothing above them can be built correctly first.

| # | Gap | Status | Why it ranks here |
|---|---|---|---|
| **1** | **Trustworthy CFG/MIR** — real branch/loop structure from the AST, ops placed inside their arms, typed IR with operators, branch predicates wired | **broken, not merely absent** | Every downstream analysis reads a graph in which no statement is control-dependent on any branch and no branch carries a condition. Path sensitivity is structurally dead until this is fixed. Cost: bounded — the lowerers already visit the right AST nodes; they discard the structure. |
| **2** | **Interprocedural taint (IFDS/IDE) with sources/sinks/sanitizers** | **absent** (vocabulary + sources only) | This is the product. Sources are done (14 typed kinds, `entrypoints/facts.rs:122-137`); propagation and sinks are not. Requires #1 and #3. |
| **3** | **Realizable-path discipline (call/return matching)** | **absent** | `PathFrame` has no call stack (`data_flow/query.rs:217-221`); summary nodes are shared across call sites. Every interprocedural result today may be unrealizable. IFDS solves #2 and #3 together — do them as one. |
| **4** | **Context sensitivity (k-CFA / object sensitivity)** | **absent everywhere** | Zero hits repo-wide. The single largest precision lever for JS/TS points-to; object-sensitivity is the known best fit for JS. Enables retiring the reachability filter. |
| **5** | **Access-path-sensitive summaries** | **partial** | `AccessPathFact` has the right vocabulary with k-limiting (`access_paths/facts.rs:7-30`) but summaries use `FlowRoot = Param(u16)\|Receiver\|Return` (`summaries/facts.rs:140-151`) — whole-parameter granularity. Cannot express `param0.body.name → return.html`, which is most real taint. |
| **6** | **Alias analysis for JS/TS recognizers** | **absent** | `ObjectTargets` is copied by value; no heap. Fixed for free by making `js_points_to` the primary resolver (see §e). |
| **7** | **SSA + dominance frontiers + natural loops** | **absent** | Prerequisite for sparse dataflow and for correct widening (unlabeled loops currently never widen, `domains/solver.rs:228-242`). Cheap once #1 lands. |
| **8** | **Interprocedural constant propagation / nullability** | **partial** | `ConstantDomain` and `NilnessDomain` exist and are correct (`domains/core.rs:267,157`) but the solver is intraprocedural. Lift via IDE (constant propagation is the canonical IDE problem) once #2's solver exists. |
| **9** | **Exceptional control flow** | **absent** | try/catch/finally, implicit throw, Go `defer`-at-exit. Security-relevant: cleanup-on-error paths are exactly where resource and auth bugs live. |
| **10** | **Path sensitivity / SMT** | **absent** | Genuinely valuable but correctly *last*: worthless until branch predicates exist (#1). Do not start here. |
| **11** | **Wall-clock + memory envelope** | **absent** | No timeout in `analysis/`; no heap ceiling; 30 GB OOM history. Blocks the "very large repos" goal regardless of algorithms. |
| **12** | **Rule-facing completeness query** | **absent** | Cheap to add, disproportionate trust value. See §(b).4. |
| **13** | Concurrency / race analysis | absent | Genuinely out of scope for now. Correctly deprioritized. |
| **14** | Additional language frontends | absent | Only Go + TS/JS (`core/mod.rs:184-191`). Scale question, not a capability-ceiling question. |

---

## (d) Target analysis-core architecture

### The choice: an IFDS/IDE solver over a repaired ICFG, with Andersen points-to underneath — not Datalog

The project already evaluated and rejected Datalog as the core
(`research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md:23` — "Do not build a Datalog
database as the cache"; `research/incremental-query-engine/RESEARCH-ANALYSIS.md:45` — "Why A Pure
Datalog Design Is Too Early"). **That decision was correct and should stand.** Datalog (Doop/CodeQL
style) buys declarative rule authoring at the cost of a relational engine that fights the existing
incremental fact/cache-key architecture, and CodeQL's own experience is that the Datalog layer is
where the memory goes. This engine's differentiator is repo-local incremental analysis with honest
budgets; a materializing relational engine is the wrong substrate for that.

The recommended kernel is **three layers, in this order**:

**Layer 1 — A real ICFG (prerequisite; no algorithm choice can compensate for its absence).**
Basic blocks with a single terminator (`Goto` / `SwitchInt(place, targets)` / `Return` /
`Unwind` / `Call { target, unwind }`), statements placed inside their arms, natural loops discovered
from back edges over the dominator tree, and a typed three-address MIR with actual operators and a
`predicate_place` on every branch. This is not research; it is the standard rustc/Soot/WALA shape,
and the current lowerers already walk the right AST nodes — they throw the structure away
(`cfg/lower_ts.rs:172-241`). Everything else in this section is blocked on it.

**Layer 2 — An IFDS/IDE tabulation solver as *the* interprocedural framework.**
Why IFDS/IDE specifically, over the alternatives:
- It is the *only* one of the candidates that structurally guarantees **realizable paths** — the
  defect at `data_flow/query.rs:217-221`. That is the current engine's biggest interprocedural
  precision hole, and IFDS eliminates it by construction rather than by filtering.
- **Distributivity buys tractability**: O(E·D³) general, O(E·D) locally separable
  (already recorded in `research/data-flow/RESEARCH-ANALYSIS.md:83-87`) — with a *provable* bound
  rather than a magic-number budget. That is the honest answer to the "no global resource envelope"
  problem in §(b).2.
- Taint (#2), interprocedural constant propagation and nullability (#8) are all instances of the
  *same* solver — IDE's environment transformers give constant propagation for free. One engine,
  many analyses, which is exactly the "new analysis plugs in" requirement.
- The **summary edges IFDS computes are reusable and per-callee**, which is precisely the
  incremental unit this codebase's cache-key architecture already wants — and it replaces the
  current shared-across-call-sites summary nodes (`data_flow/summary_edges.rs:36-70`).
- The repo's own prior research already reached this conclusion and deferred it only on sequencing
  grounds: *"IFDS/IDE is a later internal solver once the ICFG and finite fact domains are
  [stable]"* and *"A native IFDS/IDE engine should wait until CFG and ICFG inputs are stable"*
  (`research/data-flow/VALIDATION.md:167`, `:213`). **Layer 1 is that gate. Nothing else blocks it.**

The public shape should mirror Heros/WALA's `IFDSTabulationProblem`: a plugged-in analysis provides
`normalFlow`, `callFlow`, `returnFlow`, `callToReturnFlow`, a zero fact, and (for IDE) edge
functions with `compose`/`meetWith`. That is the first-class transfer-function abstraction the
engine currently lacks.

**Layer 3 — Keep the abstract-interpretation kernel for non-distributive domains.**
`trait AbstractDomain` (`domains/lattice.rs:71-97`) already has the right shape. IFDS handles
distributive, finite-domain problems; intervals, strings, and shape/type domains are not
distributive and belong in the AI kernel. Lift its solver from intraprocedural to interprocedural
via bottom-up summaries over the existing SCC condensation (`summaries/scc.rs`) — the condensation
machinery is already built. Give `widen` a real default (the current default is `join`,
`lattice.rs:91-93`, which is a no-op — safe only because all six current domains are finite-height,
and a trap for the first infinite-height domain added).

**Underneath all three: promote `js_points_to` to the sole JS/TS call-graph authority**, and add
context sensitivity (object-sensitivity, k=1→2) to it. Points-to and the call graph must be solved
*together* (on-the-fly call-graph construction), not as a union of independent producers.

**Extensibility (question 8): today, no.** `SolverPolicy` (`solver/policy.rs:79-85`) is the only
plug-in seam for solvers, and it is `pub(crate)` — plus it is too coarse to be a framework:
`fn solve(&self, budget) -> PolicyOutcome` treats each policy as an opaque black box that runs its
own hand-rolled fixpoint. Nothing is shared but the budget. The `extensions/` module is a
**subprocess protocol** (JSON handshake + provider-run over stdio, `extensions/protocol.rs:5-47`,
30 s timeout at `host.rs:19`) that lets a third party contribute **facts** in a fixed set of
families (`extensions/sinks.rs:12-19`) — it cannot contribute an *analysis*. A third party today
cannot add an interprocedural analysis without forking the core. An IFDS `TabulationProblem` trait
is the smallest change that makes the answer yes.

---

## (e) Keep vs rewrite

### Keep — this is real and hard-won

| Component | Why |
|---|---|
| `calls/js_points_to/` (solver + harvest + provider) | Genuine field-sensitive Andersen. **Promote to primary**, add context sensitivity. `solver.rs:93-133,441-454` |
| Go sidecar (`go-sidecar/polint-go-frontend`) | Reuses real `go/types` + `go/ssa` + `x/tools` RTA. Correct instinct; extend the pattern. |
| `domains/lattice.rs` + `domains/core.rs` | Real `AbstractDomain` with a propagated `Top(TopReason)`. Best soundness engineering in the repo. |
| `solver/budget.rs` + `BudgetStatus` folded into cache keys | 23 typed reasons; truncated runs can't share a cache key with complete ones. Better than most commercial tools. |
| `slicing/paths.rs` `PathOmittedRegion` | Every truncation records what was hidden. Make this the repo-wide pattern. |
| `access_paths/facts.rs` | Right vocabulary with k-limiting. Currently unused by summaries — wire it up. |
| `entrypoints/` + `trust_boundaries.rs` | 3 900 LOC of framework recognizers producing 14 typed untrusted-source kinds. This is the taint source side, already done, and it is the expensive part. |
| `mir/op.rs` `UnsupportedSemanticFact` | Honest gap taxonomy with conservative actions. Extend, don't remove. |
| Provider DAG + capability gating + determinism gates | The reason the OOM was fixable and the reason results are reproducible. Architecturally load-bearing. |

### Rewrite

| Component | Disposition |
|---|---|
| **`calls/ts_value_flows.rs` (11 898 LOC)** | **Retire, do not refactor.** Port each recognizer *down* into `js_points_to` harvest constraints, then delete. The recognizers encode real knowledge of JS idioms — that knowledge is worth keeping; the name-keyed environment, the value-copied `ObjectTargets`, and the 4-round pseudo-fixpoint are not. Delete `is_merge_descriptors_require` (`:3406`) rather than porting it. Sequence by benchmark category (`performance/2026-06-17-jelly-fn-categorization-and-wins.md:42-56`) so accuracy is monitored throughout. |
| **`cfg/lower_ts.rs` + `cfg/lower_go.rs` branch lowering** | **Rewrite.** Replace `branch_shape`-by-substring (`lower_ts.rs:244-266`) with structural AST dispatch, and place operations inside their arms (`:172-241`). Highest leverage change in the codebase. |
| **`mir/lower_*.rs`** | **Rewrite to a typed 3AC** with operators, comparisons, block structure, and a real `predicate_place`. Keep the `Unsupported` taxonomy; keep the class-member body collection (`mir/lower_ts.rs:415-479`), which is the best-executed part. |
| **`data_flow/query.rs` `find_paths`** | **Replace** with the IFDS solver. BFS path enumeration cannot be made realizable-path-correct. |
| **`data_flow/summary_edges.rs`** | **Replace** — shared per-function summary nodes are the unrealizable-path mechanism. IFDS summary edges supersede them. |
| **`summaries/facts.rs` `FlowRoot`** | **Widen** `Param(u16)\|Receiver\|Return` to access paths (reuse `AccessPathProjection`). Whole-parameter granularity cannot express real taint. |
| `unknown_taxonomy/` (1 574 LOC) | **Delete or demote.** Pure projection with two CLI consumers. Its *purpose* — completeness reporting — should be re-implemented as a rule-facing query (§(b).4), which is the thing that actually matters and does not exist. |
| Dead vocabulary | Remove or implement: `MirTerminator*` (`mir/body.rs:49-63`, never constructed), `CfgNodeKind::{RunDefers, FinallyExit}`, `CfgEdgeKind::{Extension, Unreachable, Synthetic, ImplicitThrow}`, `CfgView::{AbruptAware, ExceptionConservative}` (zero rows; `builder.rs:421` hardcodes `NormalControl`), `FlowKind::{Taint, Barrier, Sanitizer}`, `DataFlowNodeKind::{Sink, Sanitizer}`, `EvidenceEdgeKind::DataTaint`. Declared-but-unproduced enum variants read as capability to anyone auditing this tree, including future maintainers. |

### Fix now (small, disproportionate value)

1. **Rule-facing completeness query** — `ctx.completeness()` / a `CompletenessReport` on empty
   result sets. Without it every "no findings" is untrustworthy (§(b).4).
2. **CI accuracy gate** — the benchmark currently no-ops when the clone is absent
   (`eval/external/mod.rs:27-29`) and asserts only `> 0` edges. Vendor a fixture subset, commit
   real baselines (they are `null` today), and enforce an F1 floor. Also gate the **unpruned**
   number so the 53% precision cannot silently regress further.
3. **Wall-clock deadline + heap ceiling** across `analysis/`, threaded like `SolverBudget`.
4. **Cap the two unbounded fixpoints** (`cfg/derived.rs:313`, `summaries/scc.rs:241`).
5. **Record the silent drops** — `truncate(8)` in `ts_value_flows.rs` and the four bare depth
   returns in `js_points_to/harvest.rs` should emit `Top`/`Unknown`, following the
   `slicing/paths.rs` pattern.
6. **Resolve the oracle contradiction** (dynamic traces vs Jelly static output) and record the
   answer next to the numbers.

---

## Bottom line

The engineering is genuinely excellent: determinism gates, typed budgets folded into cache keys, an
honest unsupported-construct taxonomy, a real Andersen solver, a real lattice kernel, and a
capability-gated provider DAG that survived a 30 GB OOM. The project's own internal critique
(`research/static-analysis-2.0/00-critical-review.md`) is sharper than most external reviews would
be, which is the strongest possible signal that this is fixable.

But **impressive engineering is not principled analysis**, and the distinction is load-bearing here:

- The JS/TS call graph is a recognizer bank whose 96% precision is produced by a downstream filter
  masking a 53%-precision resolver.
- The interprocedural dataflow layer is BFS reachability that can report unrealizable paths.
- The CFG's control-dependence math is exactly correct over a graph where nothing is
  control-dependent on anything, and branch shapes are chosen by grepping source text.
- Taint — the headline capability for the stated goal — exists as enum variants with no producers.

Against CodeQL / Infer / Joern the gap is not one of polish or coverage; it is that those tools have
a *single principled interprocedural engine* (Datalog tabulation, bi-abduction, CPG traversals) that
every analysis is expressed in, and this engine has eighteen tagged edge producers unioned together.

The good news is that the ordering is unusually clear and the prerequisites are cheap relative to
the payoff: **repair the ICFG (Layer 1), build IFDS/IDE on it (Layer 2), promote `js_points_to` to
primary and give it context sensitivity.** Those three moves convert most of the existing 112k LOC
from liability to asset — and the project's own research already identified them and deferred them
only on sequencing grounds (`research/data-flow/VALIDATION.md:167,213`). The gate it was waiting for
is Layer 1.
