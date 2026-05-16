# Research Analysis: Algorithms, Accuracy, Complexity, And Decision Paths

## Research Question

Why are summaries the scaling boundary, and how should polint implement them so
call graphs, data flow, alias queries, and framework extensions do not become
local-only or whole-program-expensive?

## Algorithm Families

### Functional Approach

The classic idea is a procedure transformer:

```text
summary_f: input_state -> output_state
```

At a call site, the caller applies `summary_f` instead of reanalyzing `f`.

Pseudo-code:

```python
def analyze_proc(proc):
    transformer = identity()
    for stmt in proc.cfg:
        transformer = transfer(stmt).compose(transformer)
    return transformer

def transfer_call(call, state):
    return instantiate(summary(call.callee), call.actuals, state)
```

Strength:

- conceptually clean;
- supports reusable summaries;
- natural basis for caching and separate compilation.

Weakness:

- transformer domains can explode;
- precision depends on context abstraction;
- recursion needs fixed points.

polint adaptation:

- use the idea, not a universal transformer representation;
- each domain defines its own payload and transfer operations.

### IFDS

IFDS fits finite, distributive, subset data-flow problems. Facts are drawn from a
finite set `D`; flow functions map input facts to output fact sets. The tabulation
algorithm tracks valid paths through call/return structure.

Pseudo-code:

```python
def solve_ifds(program, seeds):
    worklist = PathEdgeSet(seeds)
    while worklist:
        edge = worklist.pop()
        for next_edge in normal_call_return_successors(edge):
            if add_path_edge(next_edge):
                worklist.push(next_edge)
    return reachable_facts()
```

Classic complexity:

```text
O(E * D^3)
```

where `E` is supergraph edge count and `D` is number of facts. Many practical
cases are better with sparse representations and small fact sets, but the bound
is a warning: fact-domain size matters.

Good polint domains:

- taint reachability when facts are source/variable/access-path ids;
- initializedness/nullness-style finite facts;
- selected data-flow queries.

Bad polint domains:

- arbitrary resource state machines with non-distributive joins;
- full heap shapes;
- numerical/path-condition domains;
- large string/value domains.

### IDE

IDE extends IFDS with edge functions that compute values in a lattice:

```text
fact path + edge function = environment transformer
```

Pseudo-code:

```python
def propagate(edge, edge_fn):
    old = value_at(edge.target)
    new = meet(old, edge_fn(value_at(edge.source)))
    if new != old:
        enqueue(edge.target)
```

Good polint domains:

- constant propagation over selected symbols;
- nullness with reason values;
- limited typestate values;
- simple taint labels/features.

Risk:

- if edge functions become too expressive, IDE loses its advantage and becomes
  a complex abstract interpreter.

### Weighted Pushdown Systems

WPDS models procedure calls/returns with pushdown automata and weights. It can
answer interprocedural questions with precise matched call/return behavior.

Good for:

- high-value demand queries;
- precise call/return-sensitive path problems;
- future advanced modes.

Recommendation:

- do not implement WPDS first;
- design summary/cache identity so a later WPDS engine can coexist.

### Abstract Interpretation Summaries

Abstract interpretation summarizes procedures in a chosen abstract domain:

```python
def summarize(proc, domain):
    state = domain.initial_proc_state(proc)
    for loop_or_scc in weak_topological_order(proc.cfg):
        state = iterate_with_widening(loop_or_scc, state)
    return domain.project_to_summary(state)
```

Good for:

- memory/resource effects;
- typestate;
- initializedness;
- numeric ranges;
- string domains;
- path-sensitive resource bugs.

Risk:

- widening can lose precision;
- domain design dominates accuracy;
- "top" can silently turn into noisy diagnostics.

polint adaptation:

- domain-specific summaries;
- explicit `BudgetExceeded` and `UnknownTop`;
- public views that preserve uncertainty.

### Demand-Driven Summaries

Demand summaries are keyed by query:

```text
(subject, domain, query, context) -> answer summary
```

Good for:

- alias questions;
- expensive path evidence;
- rare high-value rules;
- interactive agent workflows.

Risk:

- repeated queries can thrash without caching;
- invalidation is harder because query context participates in the key.

Recommendation:

- use eager local/bottom-up summaries first;
- add demand refinement for alias/path/evidence-heavy queries.

## Accuracy Tradeoffs By Implementation

### CodeQL

Accuracy strengths:

- declarative API graphs and access paths;
- separation of `value` and `taint` summary kinds;
- model packs can add sources, sinks, summaries, barriers, and guards;
- provenance/exactness appears in QL summary predicates.

Accuracy risks:

- summaries are only as good as library models and call resolution;
- dynamic JS patterns require custom QL or conservative approximations;
- global data-flow graph size can be expensive.

polint lesson:

- copy typed access-path summary declarations and model provenance;
- avoid tying model format to string-only access paths internally.

### Pysa

Accuracy strengths:

- summary-first design;
- explicit forward sources, backward sinks, and TITO;
- access-path trees;
- global fixpoint;
- model verification and debugging features.

Accuracy risks:

- unknown calls and whole-object taint increase false positives;
- broadening collapses precision;
- Python type/dynamic behavior limits precision.

polint lesson:

- copy the shape of TITO summaries and model verification;
- preserve broadening/heuristic markers in evidence.

### Infer/Pulse

Accuracy strengths:

- compositional pre/post states;
- heap attributes for allocation, invalidation, resource obligations, taint,
  initialization, copy origin;
- latent issue summaries propagate bugs to callers.

Accuracy risks:

- path-sensitive summaries can be expensive;
- compositionality requires careful precondition inference;
- manifest-bug focus may miss potential policy concerns.

polint lesson:

- copy pre/post and invalidation vocabulary for resource/heap domains;
- do not copy full symbolic execution as the first engine.

### LLVM/MLIR

Accuracy strengths:

- compact lattices;
- explicit resource/memory locations;
- conservative unknown top;
- effect-free queries usable by optimizers.

Accuracy risks:

- LLVM memory locations are IR-level, not source-level policy resources;
- MLIR side effects intentionally delegate alias details elsewhere.

polint lesson:

- use product lattices: access kind x resource kind;
- keep alias refinement separate.

### JVM Systems

Accuracy strengths:

- WALA synthetic method summaries can represent library bodies;
- Soot read/write sets use points-to and transitive call targets;
- Doop has explicit relations and model packs for reflection/native/open
  programs;
- OPAL separates properties and schedules fixed points.

Accuracy risks:

- classpath/reflection/native modeling dominates correctness;
- points-to precision and call graph precision are tightly coupled;
- high-precision JVM analysis can be expensive.

polint lesson:

- make setup gaps first-class diagnostics;
- separate summary domains as properties;
- require monotonicity tests when "more precise" modes are enabled.

### Go, TypeScript, Python Official Tooling

Accuracy strengths:

- Go toolchain/x/tools is the compatibility authority for packages/types/SSA;
- TypeScript compiler is the compatibility authority for narrowing,
  assertions, `never`, and declaration files;
- Python typing specs define `TypeGuard`, `TypeIs`, `NoReturn`/`Never`,
  generators, async functions, decorators, and callable signatures.

Accuracy risks:

- sidecars introduce lifecycle and cache boundaries;
- official tools may not expose every internal fact cleanly;
- optional providers must not leak unstable raw internals into the SDK.

polint lesson:

- official language tools are allowed provider inputs;
- normalize everything into polint-owned facts with provenance.

## Thought Experiments

### Thought Experiment 1: No Summaries

Rule: "No request handler can reach shell execution."

Without summaries:

```text
handler -> validate -> service -> generated client -> helper -> shell
```

The engine either:

- only sees local calls and misses the shell path;
- tries to inline every callee, including generated clients and framework
  callbacks;
- guesses that unresolved calls may do everything, producing noisy reports.

With summaries:

```text
helper: ExternalEffects(Process)
generated client: DataFlowTito(arg0 -> request body), ExternalEffects(Network)
service: CallEffects(helper), ExternalEffects(Process)
handler: summary closure reaches Process
```

The rule asks `Effects.may_call_external(handler, Process)` and receives
evidence.

### Thought Experiment 2: Bad Agent Summary

An agent writes:

```rust
summary("sanitize").removes_all_taint()
```

But the function only validates JSON shape, not authorization.

Required response:

- summary is accepted only as `DeclaredExternal`;
- validation requires fixture proving the sanitizer kind;
- it cannot erase all taint kinds unless the model declares exactly which
  source/sink kind is sanitized;
- if it conflicts with observed flows, emit model diagnostics.

This prevents agent extensions from hiding real paths.

### Thought Experiment 3: Recursive Wrapper

```text
walk(node):
  if node.leaf: return node.value
  return walk(node.left) + walk(node.right)
```

A summary scheduler without SCC handling either loops or returns empty.

Correct behavior:

- initialize SCC summaries to bottom;
- iterate until stable;
- widen access paths/trace length if needed;
- mark summary as `SummaryBased`;
- mark `BudgetExceeded` if convergence fails.

### Thought Experiment 4: Framework Lifecycle

An Express/FastAPI/Django/Go router wrapper registers handlers dynamically.
Generic analysis cannot know the project convention. An agent can.

Correct design:

- framework extension adds synthetic entrypoints and call summaries;
- these have `FrameworkModeled` precision and extension provenance;
- summaries are cache-keyed by route model code and fixture inputs;
- default mode reports unresolved framework dispatch facts.

## Decision Log

| Decision | Why |
|---|---|
| Use typed domains, not one summary bag. | Domains have different lattices, merge semantics, precision, and validation requirements. |
| Keep raw summaries internal first. | Public SDK stability would freeze the wrong shape too early. |
| Add SCC fixed point from day one. | Recursion is common; empty/incomplete recursion summaries are dangerous. |
| Preserve unknown as top/status. | Empty unknown creates false negatives. |
| Allow official language tool providers. | Compatibility precision sometimes requires the language's own toolchain. |
| Treat random OSS analyzers as references/oracles. | Runtime dependency risk conflicts with native implementation goal. |
| Let agents write Rust summary providers. | Max capability requires code, not only config. |
| Validate extension summaries before activation. | Agent-authored facts can improve or corrupt analysis. |
| Measure default-vs-agent-extended deltas. | This is the product differentiator. |

## Open Research Risks

- Exact public shape of `Effects<'_>` should wait until the first internal
  summaries exist.
- Demand-driven alias/path summaries need their own cache-key strategy.
- Cross-language summaries need a shared callable identity model.
- Summary conflict diagnostics need careful UX or they will overwhelm users.
- "ExactSemantic" trust policy must be conservative.
- Framework model validation needs benchmark fixtures, not only schema checks.
