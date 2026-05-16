# Recommended Native Data-Flow Implementation

## Recommendation

Build a native Rust **analysis fact engine** with data flow as a first-class internal fact family. Do not build a taint-specific feature, and do not wrap an external engine. The target should be:

```text
language adapters
  -> MIR / places / CFG / symbols / references / calls
  -> common value-flow graph
  -> summaries and P0 abstract domains
  -> repo-local data-flow models
  -> summary fixed points
  -> query-scoped evidence paths
  -> typed SDK views
  -> AI-agent-authored repo rules
```

The winning architecture is CodeQL-style query ergonomics, Pysa-style summary fixed points, Checker-style local abstract interpretation, Joern-style graph provenance, and FlowDroid/Heros-style IFDS later, implemented natively inside polint.

## 2026-05-16 Bootstrap Integration Decision

The implementation should now follow
`implementation/BOOTSTRAP-INTEGRATION.md`: build `analysis::data_flow` as a
private consumer of the semantic bootstrap before promoting the reserved public
`DataFlow<'_>` view.

Revised dependency order:

```text
analysis::mir
  -> analysis::places
  -> analysis::cfg
  -> analysis::calls
  -> analysis::domains
  -> analysis::summaries
  -> analysis::data_flow
  -> query-scoped evidence paths
  -> public DataFlow<'_> later
```

This avoids three implementation traps:

- freezing an immature `DataFlow<'_>` API before the internal graph is stable;
- rebuilding places, CFG, calls, summaries, and abstract domains inside a
  parallel data-flow subsystem;
- designing for taint first and then trying to generalize it into value flow,
  nilness, typestate, secret flow, dependency flow, and agent/tool-boundary
  policies.

The first concrete deliverable is not a user-facing SDK view. It is an internal
fact store with stable IDs, provenance, precision/status labels,
unknown/havoc facts, semantic cache keys, deterministic debug snapshots, and
evaluation fixtures.

## Research-Driven Precision Defaults

The recommended order is based on accuracy and cost, not convenience:

| Tier | Default? | Expected cost | Expected accuracy | Why |
|---|---:|---|---|---|
| Local CFG/value-flow | Yes | `O(E * H)` for local data-flow; sparse graph reachability `O(V + E)`. | High for modeled local semantics. | Lowest risk, immediate rule value. |
| Bounded access paths | Yes, low depth | Multiplies fact space by tracked paths. | Essential for field/property precision; too deep explodes. | FlowDroid/CFTaint/YASA all show heap/field modeling drives results. |
| Direct-call interprocedural edges | Yes when call facts exist | Linear in direct call edges. | High precision, limited recall. | Useful before global fixed points. |
| Function summaries | Yes after local flow | Fixed point over summary lattice. | Best first global tradeoff. | Pysa and CFTaint show summaries are production-practical. |
| Source/sink/sanitizer queries | Yes | Query-scoped path search. | Accuracy depends on specs. | CodeQL/Semgrep/Pysa/SemTaint all converge here. |
| IFDS/IDE | Later opt-in | General IFDS `O(E * D^3)`, locally separable `O(E * D)`. | Precise for finite distributive facts. | Requires stable ICFG and fact domain. |
| Points-to/heap refinement | Later opt-in | Classic worst-case cubic, practical cost depends on abstraction. | Improves aliases/calls/heap, can be expensive. | Needed for advanced precision, not v1 default. |
| Relational/incremental engine | Internal | Rule/join dependent; incremental affected-SCC cost. | Good for whole-program derived facts. | IncIDFA argues to design dependencies early. |

Default behavior should be local flow + low-depth access paths + direct-call edges + summaries where cheap. Expensive global path search should be query-scoped, not computed for every rule.

## Accuracy Reporting Requirements

Every provider should report:

```text
cfg_nodes_edges
dataflow_nodes_edges
summary_count
summary_iterations
access_path_count
interprocedural_edges
unknown_havoc_edges
repo_models_loaded
repo_model_facts_added
unknown_havoc_reduced_by_models
runtime_ms
cache_hit_rate
```

On fixtures, report:

```text
precision
recall
false_positive_paths
false_negative_paths
max_path_depth_hit
unsupported_constructs
default_vs_extended_precision
default_vs_extended_recall
model_validation_failures
```

This matters because the research repeatedly shows that accuracy comes from modeling choices: lifecycle/entrypoints in FlowDroid and MCP-BiFlow, field/access-path design in CFTaint and YASA, summaries in Pysa/CFTaint, and source/sink specs in CodeQL/SemTaint.

## Why This Is The Right Shape

The strongest tools converge on the same pattern:

- local flow is cheap, precise, and broadly useful;
- global flow must be query-scoped or summary-based to stay tractable;
- source/sink/sanitizer/barrier models are product-critical;
- call graph precision determines interprocedural data-flow quality;
- access paths and points-to decide heap/property precision;
- unknown dynamic behavior must be explicit;
- repo-local models can encode internal framework semantics that generic analyzers cannot know;
- path evidence is as important as the yes/no result.

For AI agents, the engine should provide stable facts and explainable queries. Agents should author rules like "this source reaches this sink unless this sanitizer appears" without understanding IFDS tabulation, CFG transfer functions, or language-specific AST quirks.

## Core Engine Layers

### 1. Fact Substrate

Create stable internal facts for:

- files, spans, symbols, references;
- imports/modules/packages;
- call sites and call edges;
- CFG nodes and edges;
- MIR operations and places;
- data-flow nodes and edges;
- function summaries;
- diagnostics, precision, and provenance.

Every fact should have deterministic IDs and cache digest participation.

### 2. Local Flow Graph

Lower each language into a small common operation model:

```python
Operation =
    Assign(lhs, rhs)
  | Read(place)
  | Write(place, value)
  | Call(result, callee, receiver, args)
  | Return(value)
  | Capture(closure, value)
  | Branch(condition)
  | UnknownEffect(reason)
```

Then emit a value-flow graph:

```python
for op in operations:
    if op is Assign(lhs, rhs):
        edge(value_node(rhs), place_node(lhs), "assignment")

    if op is Call(result, callee, receiver, args):
        edge(arg_node(args[i]), call_arg_node(call, i), "arg_bind")
        edge(call_return_node(call), value_node(result), "call_return")
        if callee_unresolved(callee):
            edge(call_arg_node(call, "*"), unknown_node(call), "havoc")
```

This gives immediate value before whole-program analysis exists.

### 3. Function Summaries

Compute compact summaries:

```python
summary(fn) = {
    "param_to_return": ...,
    "param_to_sink": ...,
    "source_to_return": ...,
    "receiver_mutations": ...,
    "field_mutations": ...,
    "tito": ...,
    "unknown_effects": ...,
}
```

Iterate summaries over the call graph:

```python
while worklist:
    fn = worklist.pop()
    new = summarize(fn, local_flow[fn], summaries_of_callees(fn))
    if new != old[fn]:
        old[fn] = new
        worklist.extend(callers(fn))
```

This should be the first global engine.

### 4. Repo-Local Data-Flow Models

Repo-local models are native polint inputs that bind to the fact substrate. They should cover sources, sinks, sanitizers, barriers, additional flow steps, summaries, entrypoints, and trust boundaries.

```rust
struct DataFlowModel {
    id: ModelId,
    language: LanguageId,
    scope: GlobSet,
    sources: Vec<SourceModel>,
    sinks: Vec<SinkModel>,
    sanitizers: Vec<SanitizerModel>,
    barriers: Vec<BarrierModel>,
    additional_steps: Vec<FlowStepModel>,
    summaries: Vec<SummaryModel>,
    entrypoints: Vec<EntrypointModel>,
    validation: Vec<ModelValidationCase>,
}
```

Model facts must keep provenance:

```rust
struct DataFlowEdgeFact {
    from: DataFlowNodeId,
    to: DataFlowNodeId,
    kind: DataFlowEdgeKind,
    algorithm: DataFlowAlgorithm,
    provider: ProviderId,
    model_id: Option<ModelId>,
    provenance: Provenance,
    validation: ValidationStatus,
    confidence: Confidence,
}
```

Binding and validation should be explicit:

```python
for model in repo_models:
    bound = bind_model_to_symbols_calls_and_places(model)
    if not bound.ok:
        emit_model_diagnostic(model, bound.errors)
        continue

    emit_model_facts(bound, provenance="RepoModel", model_id=model.id)
```

The engine should report default-vs-extended deltas so agents can see whether a model improved precision or created noise.

### 5. Query and Path Engine

Store compact edges and summaries. Reconstruct evidence paths only when rules ask:

```python
def paths_between(source, sink, query):
    graph = assemble_graph(
        local_edges=True,
        summary_edges=query.include_interprocedural,
        heuristic_edges=query.include_heuristic,
        max_depth=query.max_depth,
    )
    return bounded_bfs_with_provenance(graph, source, sink)
```

Rules get paths with:

- source span;
- sink span;
- intermediate calls;
- summary hops;
- sanitizers/barriers;
- unknown/havoc hops;
- precision/status labels.

### 6. Advanced Solvers

Add IFDS/IDE only after the graph and summaries are stable:

- IFDS for finite facts such as taint labels, initializedness, nilness/nullness, typestate states.
- IDE for facts with values such as constants, ranges, or small abstract domains.
- A native relation engine for whole-program derived facts and incremental updates.

These solvers should feed the same `DataFlow<'_>` SDK view.

## Public SDK Promotion

`DataFlow<'_>` is already reserved in the SDK, but the first implementation
should keep the capability unsupported. Promote it only after internal
`analysis::data_flow` facts have deterministic snapshots, docs, cache tests,
extension validation tests, evidence-path tests, and temp-repo rule tests.

When promoted, expose a typed view:

```rust
#[polint::rule]
fn unsafe_data_flow(ctx: &mut RuleCtx<'_>, data: DataFlow<'_>) -> RuleResult {
    let sources = data.nodes().filter(|n| n.matches_call("request", "body"));
    let sinks = data.nodes().filter(|n| n.matches_call("sql", "query"));

    for path in data.source_to_sink_paths(sources, sinks, DataFlowQuery::default()) {
        if !path.crosses_sanitizer("validate_sql_param") {
            ctx.report(path.sink().span(), "request data reaches SQL query");
        }
    }

    Ok(())
}
```

That exact API is illustrative, not final. The key is that rule authors work with typed facts, matchers, paths, and precision labels, not solver internals.

## Language Rollout

### Go First

Start with Go because package boundaries, imports, methods, and build tags are more controlled than JS/Python.

1. Syntax-level local flow with tree-sitter facts.
2. Package-aware semantic enrichment using the Go lifecycle contract.
3. Direct call edge composition.
4. Method/interface precision as call graph improves.
5. Goroutine/channel facts as explicit future precision tiers.

### TS/JS Second

Start with Oxc scopes and AST:

1. local lexical flow;
2. imports/exports and common Node module patterns;
3. object/property access paths;
4. callbacks, closures, async/promise continuations;
5. dynamic import/eval as explicit unknowns;
6. framework models as config or rule-provided semantics.

### Java Third

Java needs a semantic/classpath story before high-quality data flow:

1. class/method/field symbol facts;
2. virtual dispatch and exceptions;
3. bytecode-like or source-lowered CFG;
4. IFDS/IDE clients once ICFG quality is high.

### Python Fourth

Python needs strong uncertainty modeling:

1. import/name binding;
2. local flow and type hints;
3. decorators and class/MRO modeling;
4. first-class function calls and callback summaries;
5. dynamic feature diagnostics;
6. optional hybrid/concrete-library modeling later if the project wants it.

## AI-Agent Rule Authoring

The engine should make agents good at writing rules by giving them:

- discoverable typed views: `DataFlow<'_>`, `Calls<'_>`, `Symbols<'_>`, `References<'_>`;
- source/sink/sanitizer matcher helpers;
- path evidence with code spans;
- precision labels so generated rules can decide whether to include heuristic paths;
- examples that import only `polint::sdk::prelude::*`;
- docs that state limits per language and per algorithm.

Add tests that behave like external users: generated `.polint/rules`, public SDK imports only, real facts consumed, and diagnostics asserted through `polint check --format json`.

## Minimum Viable Powerful Version

The first version worth implementing internally should support:

- intraprocedural Go and TS/JS flow;
- direct-call interprocedural flow;
- bounded access paths;
- function summaries;
- source/sink/sanitizer/barrier queries;
- repo-local data-flow models with provenance and validation status;
- path explanations;
- unknown/havoc facts for unresolved calls and dynamic features;
- deterministic debug snapshots;
- cache invalidation for source/config/model/summary/call-target changes.

The first version worth shipping publicly should additionally include:

- `DataFlow<'_>` SDK docs under `docs/facts/data-flow.md`;
- temp-repo tests using only `polint::sdk::prelude::*`;
- evaluation harness reports for default mode and extension-enabled mode;
- documented per-language precision limits.

That is enough for real repo-local policies such as:

- request input reaches shell/SQL/file APIs;
- secret values reach logs;
- unchecked errors or nil-like values flow into dangerous operations;
- untrusted config reaches agent/tool invocation;
- test-only helpers flow into production code.

## Long-Term Goal

The long-term engine should be an extensible analysis platform:

```text
DataFlow = local value graph
         + function summaries
         + call graph
         + bounded heap/access paths
         + repo-local models
         + query-scoped path search
         + IFDS/IDE clients
         + incremental relation evaluation
         + precision/provenance everywhere
```

That can become a full call-and-data-flow engine. It will not be "complete" in the mathematical sense for dynamic languages, but it can be exact where modeled, conservative where useful, and honest where unknown.
