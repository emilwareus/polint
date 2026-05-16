# Recommended Implementation: Native CFG And Control Dependence

## Goal

Build a native Rust CFG substrate that gives repo-local rules and agent-authored extensions accurate, explainable control-flow facts without depending on external analysis engines.

The implementation must preserve polint’s product contract:

- public rule authors consume typed SDK views;
- internal graph storage stays private;
- source-level diagnostics remain stable and explainable;
- every heuristic or unsupported edge is labeled;
- extension-provided facts are validated, cache-keyed, and provenance-labeled.

## Target Architecture

```text
crates/polint/src/cfg/
  mod.rs
  ids.rs
  facts.rs
  edge.rs
  graph.rs
  builder/
    mod.rs
    control_context.rs
    block_builder.rs
    exceptional.rs
  analysis/
    reachability.rs
    dominators.rs
    postdominators.rs
    control_dependence.rs
    loops.rs
    invariants.rs
  providers/
    go.rs
    ts.rs
  extension/
    sink.rs
    merge.rs
    validation.rs
  evidence.rs
  sdk.rs
```

Keep the module internal first. Promote only `polint::sdk::facts::Cfg<'_>` after fixtures, docs, and capability planning are stable.

## First Vertical Slice

Implement Go and TS/JS first because those adapters exist today.

```text
source discovery
  -> parser adapter facts
  -> semantic index function/body inventory
  -> language CFG provider
  -> invariant validation
  -> block graph + operation graph
  -> reachability
  -> dominators/postdominators
  -> control dependence
  -> internal query view
  -> fixture snapshots
```

### Phase 1: Internal Fact Schema

Add internal facts:

- `CfgFunctionFact`
- `CfgNodeFact`
- `BasicBlockFact`
- `CfgEdgeFact`
- `ReachabilityFact`
- `DominatorFact`
- `PostDominatorFact`
- `ControlDependenceFact`
- `UnsupportedControlFlowFact`

Acceptance:

- facts have stable IDs;
- source spans are evidence, not identity;
- every fact carries provider/provenance/precision;
- facts can be serialized into deterministic debug snapshots;
- no public SDK commitment yet.

### Phase 2: Shared Builder And Validator

Implement a generic builder that language adapters drive:

```python
class CfgBuilder:
    def start_function(function_id, span):
        entry = new_node("Entry")
        exit_normal = new_node("ExitNormal")
        exit_exceptional = new_node("ExitExceptional")
        current = new_block(entry)

    def append(kind, span, ast_anchor):
        node = new_node(kind, span, ast_anchor)
        add_edge(last_node(current), node, Normal)
        current.append(node)
        return node

    def branch(condition, then_fn, else_fn):
        cond = append("Condition", condition.span, condition.anchor)
        then_entry = new_block()
        else_entry = new_block()
        join = new_block()
        edge(cond, then_entry.first, True)
        edge(cond, else_entry.first, False)
        with current = then_entry:
            then_fn()
            edge(last(), join.first, Normal)
        with current = else_entry:
            else_fn()
            edge(last(), join.first, Normal)
        current = join
```

Acceptance:

- deterministic block/node order;
- validation catches dangling edges, duplicate edges, invalid exits, and unsupported exact claims;
- builder supports control-context stacks for break/continue/return/throw/finally/defer.

### Phase 3: TS/JS Provider

Use Oxc AST/semantic data as input, but emit polint-owned facts.

Minimum constructs:

- function/module/class-static-block bodies;
- statements and expression statements;
- `if`, ternary, logical expressions, logical assignment;
- `for`, `while`, `do`, `for-in`, `for-of`;
- `switch` with fallthrough;
- `break`, `continue`, labels;
- `return`, `throw`;
- `try/catch/finally`;
- optional chaining and nullish coalescing;
- `await`, `yield`, `yield*` as suspend markers;
- class static blocks and top-level await as body/lifecycle facts.

Do not model promise scheduling as CFG edges in the first slice.

Acceptance:

- differential fixtures against Oxc and ESLint code-path tests where possible;
- CodeQL JS finally-imprecision cases represented honestly;
- `finally` cleanup paths do not get labeled exact if impossible paths are introduced.

### Phase 4: Go Provider

Use existing tree-sitter/Oxc-style adapter patterns, but design against Go SSA semantics.

Minimum constructs:

- functions and method bodies;
- `if`, `for`, `range`, `switch`, type switch, `select`;
- `break`, `continue`, `goto`, labels, `fallthrough`;
- `return`, `panic`;
- `defer` and synthetic `RunDefers`;
- short-circuit `&&` / `||`;
- `go` as spawn fact, not intraprocedural successor;
- known no-return calls as conservative summaries.

Acceptance:

- fixtures compared against `golang.org/x/tools/go/ssa` for block shape where type/lifecycle setup exists;
- `go/cfg` syntactic cases used for simple control constructs;
- panic/recover precision mode documented.

### Phase 5: Reachability And Dominators

Start with simple deterministic algorithms.

```python
def compute_reachable(graph, entry):
    seen = set()
    stack = [entry]
    while stack:
        n = stack.pop()
        if n in seen:
            continue
        seen.add(n)
        stack.extend(sorted(graph.successors(n), reverse=True))
    return seen
```

```python
def dominators(graph, entry):
    nodes = graph.reachable_nodes()
    dom = {n: set(nodes) for n in nodes}
    dom[entry] = {entry}
    changed = True
    while changed:
        changed = False
        for n in nodes - {entry}:
            preds = graph.predecessors(n)
            new = {n} | intersection(dom[p] for p in preds if p in nodes)
            if new != dom[n]:
                dom[n] = new
                changed = True
    return dom
```

For function-sized graphs, this is acceptable as a first version. Upgrade only after the evaluation harness shows it matters.

Acceptance:

- immediate dominators derived deterministically;
- unreachable nodes do not poison dominator facts;
- graph view selection participates in cache keys.

### Phase 6: Postdominators And Control Dependence

Postdominance requires a graph view and exit policy.

```python
def postdominators(cfg, view):
    g = cfg.selected_view(view)
    exit = g.synthetic_unified_exit()
    reverse = reverse_graph(g.with_exit_edges(exit))
    return dominators(reverse, exit)
```

Then derive control dependence:

```python
def control_dependence(cfg, postdom, ipdom):
    for edge in cfg.edges:
        a, b = edge.from_block, edge.to_block
        if postdom.dominates(b, a):
            continue
        runner = b
        stop = ipdom[a]
        while runner is not None and runner != stop:
            emit_control_dependence(
                controller=a,
                controlled=runner,
                via_edge=edge,
                precision=edge.precision.join(postdom.precision),
            )
            runner = ipdom[runner]
```

Acceptance:

- artificial exit policy documented per graph view;
- infinite loops and exceptional exits produce explicit precision notes;
- control dependence facts retain the controlling edge kind.

### Phase 7: Extension Overlay

Allow repo-local Rust providers to emit limited CFG-related facts through typed sinks:

- no-return summaries;
- additional exceptional summaries for known APIs;
- generated/synthetic operation nodes with source evidence;
- framework/lifecycle dispatch overlay facts;
- guard/assertion summaries used by path evidence.

First sink should be additive:

```rust
pub trait CfgExtensionSink {
    fn add_no_return_summary(&mut self, summary: NoReturnSummary) -> Result<()>;
    fn add_synthetic_edge(&mut self, edge: SyntheticCfgEdge) -> Result<()>;
    fn add_cleanup_summary(&mut self, summary: CleanupSummary) -> Result<()>;
    fn add_guard_summary(&mut self, summary: GuardSummary) -> Result<()>;
}
```

Rules:

- no direct mutation of native graph storage;
- native exact edges cannot be removed without a replacement API, validation fixture, and conflict record;
- extension edges are separate fact layer and can be requested by view;
- cache keys include extension binary/source digest and declared input dependencies.

## SDK Path

Initial public API:

```rust
#[polint::rule]
fn no_unreachable_handlers(
    ctx: &mut RuleCtx<'_>,
    cfg: Cfg<'_>,
) -> RuleResult {
    for function in cfg.functions() {
        let graph = cfg.graph(function, CfgView::AbruptAware)?;
        for block in graph.blocks() {
            if block.is_unreachable() && !block.is_generated() {
                ctx.diagnostic(...);
            }
        }
    }
    Ok(())
}
```

Later:

- `ControlDependence<'_>`
- `Loops<'_>`
- `PathEvidence<'_>`
- `Guards<'_>`

Do not expose these until fixtures and docs describe unsupported behavior.

## Cache Keys

CFG layer keys must include:

```text
source content digest
parser version
language adapter version
semantic index digest
module/source-set digest
language lifecycle config
cfg provider version
cfg schema version
graph view / precision mode
extension provider digest
extension input declarations
```

Derived facts should use layer-specific keys:

- `cfg/reachability`
- `cfg/dominators`
- `cfg/postdominators/{view}`
- `cfg/control-dependence/{view}`

## Implementation Order

1. Add internal fact structs, IDs, and snapshot debug format.
2. Implement CFG invariant validator.
3. Implement TS/JS provider for straight-line, branches, loops, returns, throws, and finally.
4. Implement Go provider for straight-line, branches, loops, returns, panic, defer, and select.
5. Add reachability and dominators.
6. Add postdominators and control dependence.
7. Add extension overlay sinks for no-return and synthetic edges.
8. Promote minimal `Cfg<'_>` SDK view.
9. Integrate with evaluation harness.

## Non-Goals For First Slice

- full path-sensitive solver;
- promise scheduling semantics;
- Go goroutine interleaving semantics;
- Python and Java implementation before adapters exist;
- bytecode CFG support;
- public graph export contract;
- exact exception modeling for every implicit runtime exception;
- merging call graph edges into CFG.
