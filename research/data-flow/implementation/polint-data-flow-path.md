# Polint Data-Flow Implementation Path

This document maps the research into a native polint implementation. It assumes the existing product direction:

- rule authors consume typed SDK fact views;
- internal analysis modules stay private or `pub(crate)`;
- unsupported or setup-missing facts produce capability diagnostics;
- repo-local analysis models can extend native facts, but must bind to static facts and keep provenance;
- future analysis families, including data flow, are exposed as typed views rather than broad `RuleCtx` accessors.

## Current Codebase Hook

The codebase already reserves a `DataFlow<'_>` fact view, but it is unsupported today:

- `crates/polint/src/sdk/facts.rs`
- `crates/polint/src/core/mod.rs`
- `crates/polint/src/analysis_plan.rs`

The implementation should turn that placeholder into a real SDK view without exposing solver internals.

## Target Pipeline

```text
file discovery
  -> parser adapters
  -> symbols/references/imports
  -> CFG facts
  -> call facts and call graph facts
  -> local data-flow facts
  -> repo-local data-flow models
  -> function summaries
  -> interprocedural data-flow facts
  -> typed SDK views and rule queries
```

Data flow must depend on call graph quality. Missing call edges and unresolved dynamic dispatch should be represented as `unknown` or `havoc` flow facts instead of silently disappearing.

## Internal Module Shape

Suggested private module layout:

```text
crates/polint/src/dataflow/
  mod.rs
  model.rs
  provider.rs
  store.rs
  query.rs
  path.rs
  summary.rs
  access_path.rs
  models.rs
  validation.rs
  algorithms/
    local.rs
    summaries.rs
    ifds.rs
    abstract_domain.rs
    relational.rs
  languages/
    go.rs
    ts_js.rs
    java.rs
    python.rs
```

Only the curated SDK view should be public:

```text
polint::sdk::facts::DataFlow<'_>
polint::sdk::facts::DataFlowQuery
polint::sdk::facts::DataFlowPath
polint::sdk::facts::DataFlowNode
polint::sdk::facts::DataFlowEdge
```

If public names are added, document them under `docs/facts/` and keep the internal engine hidden.

## Fact Model

```python
DataFlowNodeFact(
    id,
    language,
    file_id,
    span,
    enclosing_callable_id,
    symbol_id=None,
    reference_id=None,
    cfg_node_id=None,
    kind="parameter|local|field|property|literal|call_arg|call_return|return_value|receiver|capture|global|unknown",
    place=None,
    precision="exact_local|exact_semantic|summary|module_linked|conservative|heuristic|unknown",
    status="proven|partial|ambiguous|unresolved|setup_missing|unsupported",
    provenance=None,
    model_id=None,
    validation="native|validated|unvalidated|failed",
)

DataFlowEdgeFact(
    id,
    from_node,
    to_node,
    kind="assignment|read|write|field_read|field_write|property_read|property_write|arg_bind|return|call_return|capture|phi|guard|sanitizer|barrier|havoc|unknown",
    call_edge_id=None,
    guard_id=None,
    precision="exact_local",
    status="proven",
    provenance=None,
    model_id=None,
    validation="native|validated|unvalidated|failed",
)
```

Function summaries should be compact facts, not full paths:

```python
FunctionSummaryFact(
    callable_id,
    param_to_return,
    param_to_param,
    param_to_receiver_field,
    receiver_to_return,
    receiver_mutations,
    global_reads,
    global_writes,
    source_returns,
    sink_reaches,
    sanitizer_effects,
    unknown_effects,
    precision,
    provenance,
    model_id,
    validation,
)
```

## SDK View Shape

The SDK should support direct graph inspection and higher-level path queries:

```rust
impl<'a> DataFlow<'a> {
    pub fn nodes(&self) -> impl Iterator<Item = DataFlowNode<'a>>;
    pub fn edges(&self) -> impl Iterator<Item = DataFlowEdge<'a>>;

    pub fn nodes_for_symbol(&self, symbol: SymbolId) -> impl Iterator<Item = DataFlowNode<'a>>;
    pub fn nodes_for_reference(&self, reference: ReferenceId) -> impl Iterator<Item = DataFlowNode<'a>>;
    pub fn incoming(&self, node: DataFlowNodeId) -> impl Iterator<Item = DataFlowEdge<'a>>;
    pub fn outgoing(&self, node: DataFlowNodeId) -> impl Iterator<Item = DataFlowEdge<'a>>;

    pub fn reaches(&self, source: DataFlowNodeId, sink: DataFlowNodeId, query: DataFlowQuery) -> bool;
    pub fn paths_between(&self, source: DataFlowNodeId, sink: DataFlowNodeId, query: DataFlowQuery) -> impl Iterator<Item = DataFlowPath<'a>>;
}
```

For AI-agent-authored rules, add matcher helpers after the graph primitives are stable:

```rust
DataFlowQuery::new()
    .with_sources(source_matcher)
    .with_sinks(sink_matcher)
    .with_sanitizers(sanitizer_matcher)
    .include_interprocedural(true)
    .include_heuristic(false)
    .max_depth(80)
```

Rules should also be able to inspect path provenance and choose whether to include repo-model facts:

```rust
DataFlowQuery::new()
    .include_repo_models(true)
    .require_model_validation(ModelValidation::Warn)
    .include_heuristic(false)
```

## Provider Interface

```python
class LanguageDataFlowProvider:
    def required_inputs(self):
        return ["symbols", "references", "cfg", "calls"]

    def emit_local_flow(self, file_or_package):
        # cheap and cacheable
        return list_of_nodes, list_of_edges

    def emit_function_summaries(self, local_flow, call_graph):
        # summary facts only
        return summaries

    def emit_interprocedural_edges(self, summaries, call_graph):
        # optional expensive layer
        return edges

    def bind_repo_models(self, models, facts):
        # validate source/sink/sanitizer/summary/additional-step models
        return model_facts

    def diagnostics(self):
        return setup_and_precision_diagnostics
```

Providers must be deterministic, cacheable, and able to return partial facts with diagnostics.

## Implementation Milestones

### 1. Foundation

Implement the internal data-flow model, stable IDs, store, diagnostics, cache digest participation, and the `DataFlow<'_>` SDK view with no global solver yet.

Deliverable: local graph inspection over synthetic fixtures.

### 2. CFG and Local Flow

Add minimal CFG facts and intraprocedural flow for Go and TS/JS:

- assignments;
- parameter reads;
- return values;
- call arguments and call results;
- local variables;
- field/property reads and writes with bounded access paths;
- closures/captures where easy;
- explicit unknown/havoc edges for unsupported constructs.

Deliverable: local `paths_between` works inside one function and reports precision labels.

### 3. Call Graph Integration

Consume call facts and direct/resolved call facts from the call graph work. Connect:

- actual arguments to formal parameters;
- callee return values to call results;
- receiver to method receiver;
- closures to captured environments;
- unresolved calls to `havoc` facts.

Deliverable: direct-call interprocedural paths for Go and TS/JS.

### 4. Function Summary Fixed Point

Implement Pysa-style summaries:

- parameter to return;
- parameter to sink;
- source to return;
- receiver/field mutation summaries;
- TITO summaries;
- unknown effects.

Iterate over the call dependency graph until summaries converge. Reanalyze callers when callee summaries change.

Deliverable: multi-function and cross-file flow without recomputing full paths globally.

### 5. Rule Query Ergonomics

Add source/sink/sanitizer/barrier matchers on top of typed facts. These should be plain Rust APIs that AI agents can assemble in generated rules.

Deliverable: external-rule temp repo tests where generated `.polint/rules` imports only `polint::sdk::prelude::*` and asks `DataFlow<'_>` for source-to-sink paths.

### 6. Repo-Local Model Layer

Add a native model layer for high-ceiling repo-specific precision.

Model capabilities:

- source and sink declarations;
- sanitizer and barrier declarations;
- additional flow steps for builders, fluent APIs, wrappers, context bags, and generated clients;
- function summaries for wrappers and adapters;
- entrypoints and trust boundaries;
- call graph model dependencies.

```python
for model in repo_dataflow_models:
    bound = bind_model(model, symbols, calls, cfg, places)
    if not bound.ok:
        emit_model_diagnostic(model, bound.errors)
        continue

    emit(bound.sources, provenance="RepoModel", model_id=model.id)
    emit(bound.sinks, provenance="RepoModel", model_id=model.id)
    emit(bound.sanitizers, provenance="RepoModel", model_id=model.id)
    emit(bound.summaries, provenance="RepoModel", model_id=model.id)
    emit(bound.additional_steps, provenance="RepoModel", model_id=model.id)
```

Deliverable: debug output comparing default paths to extended paths, including paths added by models, paths pruned by sanitizers/barriers, and unknown/havoc reduction.

### 7. Semantic Precision

Improve languages incrementally:

- Go: package loading, method sets, interfaces, goroutines/channels, build tags, tests, module roots.
- TS/JS: imports/exports, lexical scopes, classes, callbacks, promises/async, property shape summaries, dynamic import/eval unknowns.
- Java: classpath, bytecode/source lowering, virtual dispatch, exceptions, fields.
- Python: imports, name binding, decorators, MRO, calls through first-class functions, dynamic feature diagnostics.

Deliverable: every language can opt into a precision tier without changing the SDK.

### 8. IFDS/IDE Engine

After local flow, CFG, call graph, and summaries are stable, implement an internal IFDS engine for finite facts and an IDE extension for value domains.

Deliverable: exact finite data-flow clients such as taint labels, initializedness, nilness/nullness, and selected typestate facts.

### 9. Relational/Incremental Engine

Add internal relation facts and semi-naive fixed-point scheduling for expensive whole-program analyses. Use the IncIDFA and FlowLog/Souffle research as architecture input, but keep it native Rust and private.

Deliverable: changed-file invalidation for summaries and affected callers.

## Configuration

The first public configuration should be conservative:

```toml
[facts.dataflow]
enabled = true
max_path_depth = 80
access_path_depth = 4
include_heuristic = false
include_ambiguous = true

[facts.dataflow.go]
algorithm = "local_summary"

[facts.dataflow.typescript]
algorithm = "local_summary"
record_dynamic_unknowns = true

[[facts.dataflow.models]]
path = ".polint/models/data_flow.toml"
required_validation = "warn" # off | warn | error
```

Language lifecycle settings, such as Go module roots and build tags, should stay in the existing language configuration model.

Cache digests must include model files, enabled model ids, and model validation policy.

## What Not To Do

- Do not build a taint-only engine. Taint is one query family over a general flow graph.
- Do not expose IFDS, Datalog, or solver internals as the public rule API.
- Do not claim "exact global flow" when call graph, points-to, or dynamic language support is incomplete.
- Do not hide unresolved calls. Emit unknown or havoc facts with provenance.
- Do not merge repo-model sources, sinks, summaries, or flow steps into native facts without `model_id` and validation status.
- Do not compute every global path eagerly. Store compact facts and reconstruct paths on demand.
- Do not add one-off language flags for data-flow lifecycle. Reuse the adapter and language setup contracts.
