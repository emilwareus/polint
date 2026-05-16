# Data-Flow Bootstrap Integration

Date: 2026-05-16

## Research Question

Given the implementation bootstrap decision in
`research/implementation-bootstrap/`, the revised call graph plan in
`research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md`, and the later
research on CFGs, summaries, types, aliases, abstract domains, framework
entrypoints, and the evaluation harness, how should the data-flow design change
so it becomes a native analysis-kernel layer rather than a standalone taint
engine?

## Decision

Data flow should be implemented as internal fact families inside the private
`analysis` module:

```text
analysis::mir
  -> analysis::places
  -> analysis::cfg
  -> analysis::calls
  -> analysis::domains
  -> analysis::summaries
  -> analysis::data_flow
  -> query-scoped evidence paths
  -> public SDK views later
```

Do **not** implement data flow first as:

- a public `DataFlow<'_>` SDK view with immature semantics;
- a taint-specific graph that later has to be generalized;
- a parallel CFG/call/summary subsystem;
- an eager whole-program source-to-sink path enumerator;
- a wrapper around CodeQL, Pysa, Joern, WALA, Soot, Semgrep, or another OSS
  analysis engine.

The first deliverable is internal:

```text
DataFlowNodeFact
DataFlowEdgeFact
DataFlowStore indexes
FlowModelSink
local value-flow graph
direct-call interprocedural edges
summary-projected edges
unknown/havoc facts
semantic cache keys
debug snapshots
evaluation fixtures
```

The public SDK view comes only after validation gates are met.

## Evidence Rechecked

| Source | Evidence | Design consequence |
| --- | --- | --- |
| `crates/polint/src/sdk/facts.rs:650-736` | `Cfg`, `CallGraph`, and `DataFlow` are reserved fact views; `DataFlow` currently builds only a placeholder from `AnalysisDb`. | Keep the reserved public name, but do not make it semantically real until internal facts are stable. |
| `crates/polint/src/analysis_plan.rs:637-642` | `cfg`, `call_graph`, `dataflow`, and `coverage_facts` are unsupported capabilities. | Public capability promotion should be gated by facts, docs, fixtures, cache tests, and temp-repo rule tests. |
| `crates/polint/src/core/mod.rs:142-153` | `FunctionFact` still stores `calls: Vec<String>`. | Data flow must not depend on legacy call-name strings. It needs `CallSiteFact`, `CallTargetFact`, and `PlaceId`. |
| `crates/polint/src/core/mod.rs:1109-1127` | `Capabilities::dataflow` is documented as future facts built on CFG, symbols, and call graph support. | This matches the bootstrap direction: data flow is downstream of shared semantic layers. |
| `crates/polint-macros/src/lib.rs:318-327` | Macro capability inference maps `DataFlow` to `dataflow`. | The macro path is ready, but the capability should remain unsupported until the SDK view is promoted. |
| `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/DataFlow.qll:400-451` | CodeQL config exposes sources, sinks, barriers, additional flow steps, field-flow branch limits, and access-path limits. | polint should model data flow as a configurable/queryable fact family with explicit budgets and extension hooks. |
| `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/DataFlow.qll:681-723` | CodeQL separates path explanation graph nodes/edges from the global flow predicate. | polint should store compact flow facts and reconstruct evidence paths on demand. |
| `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/TaintTracking.qll:56-150` | Taint tracking is layered over data flow with default sanitizers, additional taint steps, implicit reads, flow states, and speculation limits. | Taint should be a query/domain over general data-flow facts, not the core engine. |
| `research/data-flow/repos/codeql/shared/dataflow/codeql/dataflow/internal/FlowSummaryImpl.qll:1769-1786`, `:1910-1938` | CodeQL has a large summary implementation with parameter, return, content, and local-step reasoning. | Summaries are a first-class data-flow input. Do not make path search the only global layer. |
| `research/data-flow/repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/Engine.scala:40-54`, `:192-318` | Joern explores backwards from sinks to sources, caches table entries, expands DDG edges, uses call-site stacks, and applies method semantics. | polint should make path evidence query-scoped and call-context-aware, with model semantics as validated inputs. |
| `research/data-flow/repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/queryengine/TaskCreator.scala:32-86` | Joern tracks call stacks to avoid unrealizable paths when moving between parameters and arguments. | polint path search must preserve enough call context to distinguish realizable from spurious interprocedural paths. |
| `research/data-flow/repos/joern/dataflowengineoss/src/main/scala/io/joern/dataflowengineoss/semanticsloader/Semantics.scala:8-90`, `:111-176` | Joern has composable method semantics and explicit parameter/return flow mappings. | polint's extension sink should admit parameter/return/additional-step summaries, but validate and label them. |
| `research/data-flow/repos/pyre-check/source/interprocedural_analyses/taint/taintFixpoint.ml:8-52` | Pysa runs a global fixpoint over user models and callables until sources/sinks stop propagating. | The scalable global layer is summary fixpoint, not all-pairs path enumeration. |
| `research/data-flow/repos/pyre-check/source/interprocedural_analyses/taint/taintFixpoint.ml:112-207` | Pysa analyzes a callable with CFG, call graph, callee models, forward/backward analysis, sanitizers, and previous model state. | polint data flow must consume CFG, call graph, summaries, and sanitizer/model facts in one scheduled provider. |
| `research/data-flow/repos/pyre-check/source/interprocedural_analyses/taint/callModel.ml:23-52`, `:209-260` | Pysa falls back to an obscure model for missing callee models and matches actuals to formals for generations, sinks, TITO, and sanitizers. | Missing summaries are not silent. polint should emit unknown/havoc or declared-external effects with provenance. |
| `research/data-flow/repos/pyre-check/source/interprocedural_analyses/taint/forwardAnalysis.ml:3740-3792` | Pysa runs a local forward fixpoint over CFG and extracts a forward model from the exit state. | Local data-flow and summary extraction should be separate stages with cacheable outputs. |
| `research/data-flow/repos/opengrep/src/analyzing/Dataflow_core.ml:138-180` | OpenGrep's generic fixpoint uses deterministic processing order and visit limits. | polint should make iteration order, budgets, and budget-exceeded status deterministic and cache-visible. |
| `research/data-flow/repos/opengrep/src/tainting/Dataflow_tainting.ml:55-67` | OpenGrep explicitly says its taint analysis is a MAY analysis, lacks alias analysis, and has limited field sensitivity. | polint must label precision honestly instead of presenting broad flow as exact. |
| `research/data-flow/repos/opengrep/src/tainting/Dataflow_tainting.ml:110-125`, `:2528-2684`, `:2801-2862` | OpenGrep stores lvalue environments, signatures, built-in signatures, call graph, effects, and uses CFG transfer/fixpoint. | The right internal unit is an analysis provider over shared CFG/place/call/summary facts, not direct rule-level matching. |
| `research/data-flow/repos/heros/src/heros/IFDSTabulationProblem.java:21-60` | Heros defines IFDS over flow functions, ICFG, initial seeds, and zero value. | IFDS belongs after a stable ICFG/fact domain exists. |
| `research/data-flow/repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/TabulationProblem.java:20-48` | WALA's tabulation problem depends on a supergraph, domain, flow-function map, and path-edge seeds. | A native IFDS backend is a later solver over polint facts, not the first data-flow representation. |
| `research/data-flow/repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/TabulationSolver.java:100-152`, `:201-236`, `:500-548` | WALA stores path edges, call-flow edges, summary edges, seeds, worklists, and call-to-return processing. | If polint implements IFDS, it needs compact edge stores and summary edges; the first layer should prepare these IDs. |
| `research/data-flow/repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/problems/InfoflowProblem.java:72-190`, `:431-563`, `:776-832` | FlowDroid models normal/call/return/call-to-return functions, aliases, arrays, fields, implicit taints, and kill behavior. | High precision requires language/heap/alias semantics; do not promise exact global data flow before those providers exist. |

## Revised Architecture

### Internal Module Placement

Use the implementation bootstrap module tree:

```text
crates/polint/src/analysis/
  data_flow.rs
  mir/
  places.rs
  cfg.rs
  calls.rs
  domains/
  summaries/
  extensions/
```

When data flow grows large, split `analysis/data_flow.rs` into:

```text
analysis/data_flow/
  mod.rs
  facts.rs
  store.rs
  local.rs
  direct_calls.rs
  summary_edges.rs
  models.rs
  query.rs
  paths.rs
  validation.rs
  algorithms/
    sparse.rs
    reachability.rs
    ifds.rs
    ide.rs
```

Keep this internal. Do not add behavior to `sdk::facts::DataFlow` yet.

### Dependency Direction

Data flow should depend on already-owned semantic layers:

```text
SourceFiles
  -> Syntax facts
  -> ModuleGraph / Symbols / References
  -> MIR
  -> Places
  -> CFG
  -> CallSites / DirectTargets
  -> P0 abstract domains
  -> Direct summaries
  -> DataFlow local graph
  -> Direct-call interprocedural edges
  -> Summary-projected edges
  -> Query paths
```

Avoid this cycle:

```text
data flow needs refined call graph
refined call graph needs data flow
```

The safe cycle-breaking rule is:

```text
local data flow + direct calls + direct summaries first;
refined call providers may consume summaries later;
data flow can then consume refined call targets in a later tier.
```

## Internal Fact Model

### IDs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowPathId(pub u64);
```

Use dense IDs only as runtime handles. Stable identity belongs in `FactMeta`.

### Node Fact

```rust
pub(crate) struct DataFlowNodeFact {
    pub(crate) id: DataFlowNodeId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) owner: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) op: Option<MirOpId>,
    pub(crate) cfg_node: Option<CfgNodeId>,
    pub(crate) span: Span,
    pub(crate) kind: DataFlowNodeKind,
    pub(crate) place: Option<PlaceId>,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) reference: Option<ReferenceId>,
    pub(crate) call_site: Option<CallSiteId>,
}
```

Initial node kinds:

```rust
pub(crate) enum DataFlowNodeKind {
    Place,
    Parameter { index: u32 },
    Receiver,
    Local,
    Global,
    FieldOrProperty,
    Literal,
    Temporary,
    CallArgument { index: u32 },
    CallReceiver,
    CallReturn,
    ReturnValue,
    SummaryParameter { index: u32 },
    SummaryReturn,
    Source,
    Sink,
    Sanitizer,
    Barrier,
    Unknown,
    Havoc,
}
```

Do not encode taint kinds directly into the node identity. Taint source/sink
labels belong in model facts, query predicates, or domain payloads.

### Edge Fact

```rust
pub(crate) struct DataFlowEdgeFact {
    pub(crate) id: DataFlowEdgeId,
    pub(crate) from: DataFlowNodeId,
    pub(crate) to: DataFlowNodeId,
    pub(crate) kind: DataFlowEdgeKind,
    pub(crate) owner: Option<FunctionId>,
    pub(crate) call_site: Option<CallSiteId>,
    pub(crate) call_target: Option<CallTargetId>,
    pub(crate) summary: Option<SummaryId>,
    pub(crate) guard: Option<CfgNodeId>,
    pub(crate) model: Option<ModelId>,
}
```

Initial edge kinds:

```rust
pub(crate) enum DataFlowEdgeKind {
    Assignment,
    Phi,
    Read,
    Write,
    FieldRead,
    FieldWrite,
    PropertyRead,
    PropertyWrite,
    IndexRead,
    IndexWrite,
    ArgumentToParameter,
    ParameterToArgument,
    ReceiverToSelf,
    ReturnToCallResult,
    CallToReturn,
    SummaryInputToOutput,
    SourceIntroduction,
    SinkConsumption,
    Sanitizer,
    Barrier,
    AdditionalStep,
    Capture,
    Await,
    Throw,
    UnknownEffect,
    Havoc,
}
```

`FactMeta` carries:

```text
stable_key
producer
layer_id
precision
confidence
validation
evidence
status
```

Statuses should include:

```rust
pub(crate) enum DataFlowStatus {
    ExactLocal,
    ExactSemantic,
    SummaryBased,
    ModelProvided,
    Heuristic,
    Conservative,
    Ambiguous,
    Unresolved,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}
```

### Unknown And Havoc Facts

Unknowns are not diagnostics only. They are facts:

```rust
pub(crate) struct UnknownFlowFact {
    pub(crate) node: DataFlowNodeId,
    pub(crate) owner: Option<FunctionId>,
    pub(crate) call_site: Option<CallSiteId>,
    pub(crate) missing_input: Option<FactFamily>,
    pub(crate) reason: UnknownFlowReason,
}
```

Important reasons:

```rust
pub(crate) enum UnknownFlowReason {
    UnresolvedCall,
    MissingSummary,
    DynamicPropertyWrite,
    Reflection,
    Eval,
    UnsupportedSyntax,
    MissingTypeInfo,
    MissingAliasInfo,
    BudgetExceeded,
    ExtensionRejected,
}
```

Rules and agents should be able to see:

```text
unknown_edges_total
unknown_edges_by_reason
unknown_edges_reduced_by_extension
remaining_unknown_edges_affecting_query
```

## Store And Indexes

```rust
pub(crate) struct DataFlowStore {
    nodes: FactStore<DataFlowNodeFact, DataFlowNodeId>,
    edges: FactStore<DataFlowEdgeFact, DataFlowEdgeId>,
    unknowns: Vec<UnknownFlowFact>,

    nodes_by_place: BTreeMap<PlaceId, SmallVec<[DataFlowNodeId; 2]>>,
    nodes_by_symbol: BTreeMap<SymbolId, SmallVec<[DataFlowNodeId; 2]>>,
    nodes_by_call: BTreeMap<CallSiteId, SmallVec<[DataFlowNodeId; 4]>>,
    edges_by_from: BTreeMap<DataFlowNodeId, SmallVec<[DataFlowEdgeId; 4]>>,
    edges_by_to: BTreeMap<DataFlowNodeId, SmallVec<[DataFlowEdgeId; 4]>>,
    edges_by_call: BTreeMap<CallSiteId, SmallVec<[DataFlowEdgeId; 4]>>,
    edges_by_summary: BTreeMap<SummaryId, SmallVec<[DataFlowEdgeId; 4]>>,
    sources_by_kind: BTreeMap<SourceKind, SmallVec<[DataFlowNodeId; 4]>>,
    sinks_by_kind: BTreeMap<SinkKind, SmallVec<[DataFlowNodeId; 4]>>,
}
```

The store must be deterministic:

- sorted input iteration;
- stable node/edge allocation order;
- canonical deduplication by stable key;
- deterministic tie-breaking for equivalent edges;
- no hash-map iteration in snapshots.

## Provider Tiers

### Tier 0: Local Sparse Value Flow

Build from MIR operations, CFG node anchors, and `PlaceId`.

Cost:

```text
build: O(ops + local_edges)
query: O(V + E) for a bounded local reachability query
```

Default: yes.

This tier handles:

- assignment;
- destructuring when lowered;
- local variable and temporary flow;
- parameter and return nodes;
- call argument and call return boundary nodes;
- field/property/index read/write at bounded access-path depth;
- explicit unknown/havoc edges for unsupported operations.

### Tier 1: Direct-Call Interprocedural Edges

Consume `CallSiteFact` and high-confidence `CallTargetFact`.

Cost:

```text
O(number_of_direct_call_targets * average_argument_count)
```

Default: yes, after direct call facts exist.

Edges:

- actual argument to callee parameter;
- receiver to callee receiver/self;
- callee return summary to call result;
- call-site unknown/havoc if target is unresolved, ambiguous, unsupported, or
  setup-missing.

### Tier 2: Summary-Projected Edges

Consume `DataFlowSummary` payloads from the summary kernel.

Cost:

```text
O(iterations * (local_summary_cost + call_edges * summary_size))
```

Default: yes after direct summaries exist, with strict budgets.

Initial summary payload:

```rust
pub(crate) struct DataFlowSummary {
    pub(crate) callable: FunctionId,
    pub(crate) input_output: Vec<SummaryFlowEdge>,
    pub(crate) sources: Vec<SummarySource>,
    pub(crate) sinks: Vec<SummarySink>,
    pub(crate) sanitizers: Vec<SummarySanitizer>,
    pub(crate) barriers: Vec<SummaryBarrier>,
    pub(crate) unknown_effects: Vec<UnknownFlowReason>,
}
```

Summary edges should be compact facts, not stored full paths.

### Tier 3: Repo-Local Model Edges

Consume validated extension facts:

- source models;
- sink models;
- sanitizer models;
- barrier models;
- additional flow steps;
- parameter/return summaries;
- external API summaries;
- framework entrypoint/trust-boundary facts.

Model facts augment the native graph. They do not overwrite native facts unless a
specific merge policy says so.

### Tier 4: Query-Scoped Path Evidence

Do not compute every global source-to-sink path eagerly.

```python
def paths_between(query):
    graph = assemble_graph(
        include_local=True,
        include_direct_calls=query.direct_calls,
        include_summaries=query.summaries,
        include_models=query.models,
        include_heuristic=query.heuristic,
        max_depth=query.max_depth,
    )
    return bounded_search_with_call_context_and_provenance(graph, query)
```

Path search should record:

- source span;
- sink span;
- intermediate call sites;
- summary hops;
- sanitizers/barriers crossed;
- unknown/havoc hops;
- precision/status per hop;
- provider/model provenance.

### Tier 5: IFDS/IDE Later

IFDS/IDE should be an internal solver over the same facts:

```text
ICFG = CFG + CallTargetFact + Summary edges
Domain = finite DataFlowFact labels
Seeds = query/source/model seeds
Flow functions = built from DataFlowEdgeKind and domain transfer
```

Use it for finite distributive facts once ICFG and fact domains are stable:

- taint labels;
- initializedness;
- nilness/nullness;
- typestate states;
- selected protocol states.

Do not expose IFDS/IDE in the public SDK.

### Tier 6: Alias And Points-To Refinement Later

Consume alias/points-to facts to refine:

- field/property precision;
- indirect calls;
- object/heap flow;
- escaping closures;
- callback flows.

Until then, label heap/property edges as bounded or conservative.

## Extension Sink

Data-flow extension code should not mutate the store directly. It writes through
a validating sink:

```rust
pub(crate) trait DataFlowModelSink {
    fn add_source(&mut self, model: SourceModelFact) -> Result<ModelFactId, ModelError>;
    fn add_sink(&mut self, model: SinkModelFact) -> Result<ModelFactId, ModelError>;
    fn add_sanitizer(&mut self, model: SanitizerModelFact) -> Result<ModelFactId, ModelError>;
    fn add_barrier(&mut self, model: BarrierModelFact) -> Result<ModelFactId, ModelError>;
    fn add_additional_step(&mut self, model: AdditionalStepFact) -> Result<ModelFactId, ModelError>;
    fn add_summary(&mut self, model: DataFlowSummaryFact) -> Result<ModelFactId, ModelError>;
}
```

Validation gates:

- endpoints bind to existing symbols, references, call sites, places, CFG nodes,
  or stable external symbols;
- spans are valid;
- model IDs and extension IDs participate in cache keys;
- provider precision ceiling is enforced;
- barriers/sanitizers cannot silently delete native facts;
- conflicting models produce deterministic diagnostics;
- unvalidated high-impact models can run only under an explicit policy.

Merge policy:

| Model fact | Default merge | Reason |
| --- | --- | --- |
| Source | Additive | Missing sources are common false negatives. |
| Sink | Additive | Missing sinks are common false negatives. |
| Sanitizer | Path-filtering, not edge deletion | Over-broad sanitizers cause false negatives. |
| Barrier | Path-filtering, not edge deletion | Barriers need evidence and validation. |
| Additional step | Additive edge with provenance | Agents can model builders/wrappers/frameworks. |
| Summary | Additive or competing by model key | Multiple providers may model the same API. |
| Unknown reduction | Derived metric only | Keep original unknown fact for auditability. |

## Cache Keys And Invalidation

Data-flow cache keys must include:

```text
provider id and version
data-flow schema version
input layer digests:
  MIR
  CFG
  places
  symbols/references
  call sites
  call targets
  summaries
  abstract-domain facts used by transfer
language lifecycle/setup digest
access-path depth
field/property precision settings
unknown/havoc policy
query-independent model manifest digest
extension binary/source digest
extension validation policy
polint version
```

Path query result keys additionally include:

```text
source matcher digest
sink matcher digest
sanitizer/barrier matcher digest
included edge tiers
max path depth
max paths
include heuristic/conservative flags
include unknown/havoc flags
call-context policy
```

Do not include rule code in local data-flow graph keys unless rule options
change the graph. Rule code belongs in path/query cache keys.

## Public SDK Promotion Gates

`DataFlow<'_>` is already reserved. The first implementation should still keep
it unsupported.

Promote it only when all of these are true:

- internal data-flow facts have deterministic debug snapshots;
- Go and TS/JS local flow fixtures pass;
- direct-call interprocedural fixtures pass;
- summary-projected fixtures pass;
- extension source/sink/sanitizer/barrier/additional-step fixtures pass;
- cache invalidation tests cover source, config, model, extension, summary, and
  call-target changes;
- unknown/havoc facts are visible in debug output and capability diagnostics;
- docs under `docs/facts/` state precision and language limits;
- temp-repo rule tests import only `polint::sdk::prelude::*` and request
  `DataFlow<'_>`;
- evaluation harness reports default-vs-extension deltas for at least one data
  flow suite.

Initial SDK should expose high-level queries and precision labels, not solver
internals:

```rust
impl<'a> DataFlow<'a> {
    pub fn sources(&self, query: DataFlowQuery<'_>) -> impl Iterator<Item = DataFlowNode<'a>>;
    pub fn sinks(&self, query: DataFlowQuery<'_>) -> impl Iterator<Item = DataFlowNode<'a>>;
    pub fn reaches(&self, source: DataFlowNodeId, sink: DataFlowNodeId, query: DataFlowQuery<'_>) -> bool;
    pub fn paths_between(
        &self,
        source: DataFlowNodeId,
        sink: DataFlowNodeId,
        query: DataFlowQuery<'_>,
    ) -> impl Iterator<Item = DataFlowPath<'a>>;
}
```

Graph-wide node/edge iteration can wait unless real rule authors need it.

## Revised Implementation Sequence

### 1. Internal Store And Snapshots

Add `analysis::data_flow` with IDs, facts, store, metadata, validation, and
debug snapshot serialization.

Acceptance:

- no public SDK behavior;
- deterministic snapshots for hand-built synthetic facts;
- invalid references rejected by validation;
- duplicate stable keys normalize deterministically.

### 2. Local MIR-To-Flow Builder

Build sparse local value-flow from MIR and places.

Acceptance:

- assignments, params, returns, call args, call returns, and bounded
  field/property paths work in Go and TS/JS fixtures;
- unsupported constructs emit unknown/havoc facts;
- path search inside one function works for synthetic queries.

### 3. Direct-Call Edges

Consume revised call graph direct target facts.

Acceptance:

- actual-to-formal, receiver-to-self, and return-to-result edges work;
- unresolved calls produce queryable unknown/havoc facts;
- ambiguous calls are labeled and budgeted.

### 4. Summary Projection

Consume `DataFlowSummary` from the summary kernel.

Acceptance:

- TITO, param-return, source-return, param-sink, sanitizer, and unknown-effect
  summaries project into edges or query filters;
- recursive/SCC summaries use deterministic iteration and widening/budget
  status;
- summary version and dependency digests invalidate correctly.

### 5. Model Sink

Implement repo-local model sink and merge policy.

Acceptance:

- source/sink/sanitizer/barrier/additional-step/summary model facts bind to
  static facts;
- invalid models are rejected with diagnostics;
- default-vs-extension delta metrics show paths added, paths pruned, and
  unknowns reduced.

### 6. Query-Scoped Paths

Add internal path search with call-context checks and provenance.

Acceptance:

- bounded search is deterministic;
- path output distinguishes local edges, direct-call edges, summary hops, model
  edges, sanitizer/barrier decisions, and unknown/havoc hops;
- path explosion produces budget facts instead of silent truncation.

### 7. SDK Promotion

Only after the above: implement `DataFlow<'_>` methods and capability support.

Acceptance:

- external-rule temp-repo tests;
- docs;
- cache tests;
- evaluation harness baseline;
- public API review.

## Rejected Paths

### Start With A Public SDK View

Rejected because it freezes the wrong concepts early. The current placeholder
can remain reserved while the internal graph matures.

### Build Taint First And Generalize Later

Rejected because taint-specific concepts would leak into node/edge identity.
Taint is a query/domain over a general value-flow graph.

### Implement IFDS First

Rejected for the first slice. IFDS needs stable ICFG, facts, summaries, and
finite domains. Without those, the solver would drive architecture instead of
consuming architecture.

### Eager Whole-Program Paths

Rejected because path count explodes and most rules need specific
source/sink/model queries. Store compact edges and summaries; search paths on
demand.

### Let Extensions Delete Native Facts

Rejected. Extensions can add model facts, add barriers/sanitizers, and influence
query filtering under validation. Native facts remain auditable.

## Open Questions

- Should `DataFlowNodeFact` and `PlaceFact` remain separate long-term, or should
  some place nodes be generated lazily by the query engine?
- How much call context should Tier 4 path search keep before IFDS exists:
  exact stack up to depth, summary-token stack, or CodeQL-like context features?
- Should sanitizer/barrier semantics live in data-flow query options only, or
  also as summary-domain facts for reuse by effects summaries?
- What is the first public rule that justifies `DataFlow<'_>` promotion:
  secrets-to-log, request-to-SQL, unchecked nil/error flow, or tool-boundary
  taint?
- Should model validation require fixture tests for high-impact barrier and
  sanitizer models before CI allows them?

## Final Recommendation

Implement data flow as the first serious consumer of the private semantic
bootstrap, not as a separate product feature.

The powerful path is:

```text
stable places
  + local sparse flow
  + direct call facts
  + compact summaries
  + extension model sinks
  + query-scoped evidence paths
  + evaluation deltas
```

That gives polint an engine that can grow toward IFDS/IDE, alias-aware heap
flow, framework-specific data flow, and agent-authored Rust extensions without
locking the public API to immature internals.
