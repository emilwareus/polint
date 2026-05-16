# Recommended Implementation: Native Type, Value, Points-To, And Alias Analysis

## Goal

Build a native Rust implementation that gives polint strong type, value, points-to, and alias facts across languages without embedding external analysis engines.

The design must support:

- multiple languages through language-owned providers;
- typed SDK views instead of raw internal graph exposure;
- rule-requested precision tiers;
- explicit unknown and unsupported facts;
- agent-authored Rust extensions that can improve accuracy;
- deterministic scheduling, caching, provenance, and validation;
- future sparse flow-sensitive and context-sensitive refinements.

## Core Architecture

```text
crates/polint/src/analysis/
  types/
    ids.rs
    facts.rs
    lattice.rs
    provider.rs
    narrowing.rs
    merge.rs
  values/
    facts.rs
    abstract_value.rs
    allocation.rs
    constants.rs
  places/
    place.rs
    access_path.rs
    field_key.rs
    ownership.rs
  summaries/
    function.rs
    method.rs
    module.rs
    framework.rs
    merge.rs
  points_to/
    constraints.rs
    solver.rs
    bitsets.rs
    scc.rs
    budgets.rs
    projection.rs
  alias/
    query.rs
    result.rs
    provider_stack.rs
    evidence.rs
  extensions/
    sinks.rs
    validation.rs
    conflicts.rs
```

Keep this internal first. Promote only typed SDK views after the facts have fixtures and documentation.

## Fact Layers

### 1. Place Facts

Places give all later analysis a shared vocabulary.

```rust
pub(crate) struct PlaceFact {
    id: PlaceId,
    kind: PlaceKind,
    owner: ScopeOrFunctionId,
    access_path: Option<AccessPathId>,
    type_hint: Option<TypeSetId>,
    span: Span,
    provenance: Provenance,
}

pub(crate) enum PlaceKind {
    Local,
    Parameter,
    Global,
    ModuleExport,
    Receiver,
    Field { base: PlaceId, key: FieldKey },
    Property { base: PlaceId, key: PropertyKey },
    Index { base: PlaceId, key: IndexKey },
    Captured,
    Synthetic,
    Unknown,
}
```

Do this before points-to. A solver without stable place IDs becomes impossible to debug or extend.

### 2. Type Facts

Type facts should be language-owned but normalized into a shared envelope.

```rust
pub(crate) struct TypeFact {
    subject: TypeSubject,
    type_set: TypeSetId,
    phase: TypePhase,
    precision: Precision,
    evidence: EvidenceId,
}

pub(crate) enum TypePhase {
    Declared,
    Inferred,
    Resolved,
    FlowNarrowed { cfg_node: CfgNodeId },
    ExtensionProvided,
}
```

Each language can keep its own detailed type lattice internally. The cross-language layer needs enough structure for rule queries, call graph construction, summaries, and value/alias pruning:

- nominal type IDs;
- structural/interface/protocol shape IDs;
- primitive/literal types;
- union/intersection sets;
- nullable/undefined/unknown/any distinctions;
- function/callable signatures;
- class/object/module types;
- type-variable/generic placeholders;
- unsupported/unknown reasons.

### 3. Value Facts

Value facts are cheaper and often more useful than heap points-to:

```rust
pub(crate) enum AbstractValue {
    Unknown,
    Bottom,
    Null,
    Undefined,
    Bool(BoolSet),
    Number(NumberDomain),
    String(StringDomain),
    Literal(LiteralId),
    EnumMember(SymbolId),
    Function(FunctionObjectId),
    Class(ClassObjectId),
    Module(ModuleId),
    Object(AllocationTokenId),
    Union(SmallVec<[AbstractValueId; 4]>),
}
```

Start with constants, truthiness, nullness, function objects, class objects, module objects, and allocation tokens. Add numeric/string domains later only when rules need them.

### 4. Local Flow And Narrowing

Use the CFG layer to compute local branch-sensitive facts:

```python
def local_flow(function):
    state_in[entry] = initial_state(function)
    worklist = [entry]
    while worklist:
        node = worklist.pop()
        out = transfer(node, state_in[node])
        for edge in node.out_edges:
            narrowed = apply_edge_condition(edge, out)
            if join(state_in[edge.to], narrowed):
                worklist.push(edge.to)
```

This should produce:

- narrowed types at CFG nodes;
- definite assignment/boundness;
- nullness/truthiness facts;
- literal refinements;
- function-object and class-object refinements;
- local no-alias facts where language semantics prove disjoint locals or allocations.

### 5. Summary Facts

Summaries are the interprocedural boundary.

```rust
pub(crate) struct FunctionSummaryFact {
    callable: CallableId,
    parameters: Vec<ParameterSummary>,
    returns: Vec<ReturnSummary>,
    throws: Vec<ThrowSummary>,
    effects: EffectSummary,
    value_flows: Vec<SummaryFlowEdge>,
    points_to_constraints: Vec<PointsToConstraint>,
    call_targets: Vec<ModeledCallTarget>,
    precision: Precision,
    provenance: Provenance,
}
```

Summaries should be emitted by native language providers and repo-local extension providers. This is the main scalability tool and the main agent-extensibility tool.

### 6. Points-To Constraints

Use an inclusion-based internal representation.

```rust
pub(crate) enum PointsToConstraint {
    AddressOf { dst: PtVar, object: ObjectToken },
    Copy { dst: PtVar, src: PtVar },
    Load { dst: PtVar, pointer: PtVar },
    Store { pointer: PtVar, src: PtVar },
    FieldLoad { dst: PtVar, base: PtVar, field: FieldKey },
    FieldStore { base: PtVar, field: FieldKey, src: PtVar },
    ElementLoad { dst: PtVar, base: PtVar, index: IndexModel },
    ElementStore { base: PtVar, index: IndexModel, src: PtVar },
    CallReturn { dst: PtVar, call: CallSiteId },
    SummaryFlow { dst: PtVar, src: PtVar, summary: SummaryId },
}
```

Implement this as an internal provider. Rules should not see raw constraints by default; they should query typed views.

### 7. Alias Query Service

Alias answers should carry evidence and uncertainty:

```rust
pub(crate) enum AliasAnswer {
    NoAlias { evidence: EvidenceId },
    MayAlias { evidence: EvidenceId },
    MustAlias { evidence: EvidenceId },
    PartialAlias { evidence: EvidenceId },
    Unknown { reason: UnknownReason },
}
```

The query service should be provider-stack based:

```python
def alias(a, b, budget):
    for provider in providers:
        answer = provider.query(a, b, budget)
        if answer.is_definitive():
            return answer
        budget = budget.remaining()
    return Unknown("budget/provider exhausted")
```

Provider order:

1. identity/disjointness provider;
2. language ownership provider;
3. local flow provider;
4. extension no-alias/must-alias provider;
5. points-to provider;
6. sparse refinement provider later.

## Native Implementation Phases

### Phase 1: Shared IDs And Fact Envelopes

Implement:

- `PlaceId`, `AccessPathId`, `TypeSetId`, `AbstractValueId`, `AllocationTokenId`, `PtVar`, `ObjectToken`;
- `Precision`, `Provenance`, `UnknownReason`;
- deterministic debug serialization;
- fact validation invariants.

Acceptance:

- no public SDK exposure yet;
- stable IDs do not depend on traversal nondeterminism;
- debug snapshots are stable across runs.

### Phase 2: Go And TS/JS Place + Value Facts

Go:

- locals, parameters, receivers, package globals, selectors, fields, indexes;
- function objects, method values, interface values, composite literals, address-taken values;
- nil/nullness and basic constants.

TS/JS:

- bindings, parameters, imports/exports, `this`, object properties, class fields, optional-chain access paths;
- functions, arrows, classes, object/array literals, module namespace objects;
- `null`/`undefined`, truthiness, string literal keys, discriminant values.

Acceptance:

- snapshot fixtures for place/access-path extraction;
- no points-to solver required yet;
- unknown dynamic keys are explicit facts.

### Phase 3: Type Facts And Local Narrowing

TS/JS:

- implement enough declared/inferred/narrowed facts to support guards, discriminants, null checks, `typeof`, `instanceof`, `in`, strict equality, optional chaining, and type predicates where available.
- Use TypeScript/Pyright/Oxc findings as references but keep native Rust facts.

Go:

- implement package/type/method set facts sufficient for selector resolution, interface implementation, aliases, pointer receiver/value receiver differences, and generics placeholders.
- Validate against `go/types` behavior in fixtures.

Acceptance:

- `NarrowedTypeFact` keyed by CFG location;
- rule/debug output can explain why a type was narrowed;
- unsupported cases produce `Unknown` rather than silently widening to exact.

### Phase 4: Summaries

Implement native summary emission for:

- pure direct functions;
- methods;
- constructors/factory-like functions;
- getters/setters/property accessors where applicable;
- known builtins and standard library primitives;
- extension-provided framework/API summaries.

Acceptance:

- summaries have cache keys based on source, config, language mode, dependency graph, and extension code digest;
- conflicts between native and extension facts are reported with deterministic merge behavior.

### Phase 5: Bounded Andersen Solver

Implement:

- dense integer IDs;
- bitset points-to sets;
- delta propagation;
- SCC collapse for copy constraints;
- field-sensitive object-field variables;
- type filters;
- worklist budgets;
- query-scoped solving when possible;
- `Unknown` on budget exhaustion.

Pseudo:

```python
def solve(constraints, budget):
    graph = build_copy_graph(constraints)
    collapse_sccs(graph)
    pt = BitsetMap()
    worklist = seed_address_of_constraints(pt)

    while worklist and budget.ok():
        var = worklist.pop()
        delta = take_delta(var)

        for dst in copy_succ[var]:
            if pt[dst].add_all(delta):
                worklist.push(dst)

        for load in loads_from[var]:
            for obj in delta:
                if add_copy_edge(obj_var(obj), load.dst):
                    propagate_existing(obj_var(obj), load.dst)

        for store in stores_to[var]:
            for obj in delta:
                if add_copy_edge(store.src, obj_var(obj)):
                    propagate_existing(store.src, obj_var(obj))

        for field_load in field_loads_from[var]:
            for obj in delta:
                field_var = object_field(obj, field_load.field)
                if add_copy_edge(field_var, field_load.dst):
                    propagate_existing(field_var, field_load.dst)
```

Acceptance:

- fixture cases for address/copy/load/store/field flow;
- solver can explain why an object appears in a points-to set;
- memory and time budgets are tested;
- precision labels distinguish flow-insensitive results.

### Phase 6: Alias Query SDK View

Expose only after facts stabilize:

```rust
pub struct Aliases<'a> { /* internal */ }

impl<'a> Aliases<'a> {
    pub fn may_alias(&self, a: PlaceRef, b: PlaceRef) -> AliasResult;
    pub fn must_alias(&self, a: PlaceRef, b: PlaceRef) -> AliasResult;
    pub fn points_to(&self, place: PlaceRef) -> PointsToView<'a>;
    pub fn precision(&self, result: AliasResultId) -> Precision;
}
```

The SDK should prefer domain queries over raw graph dumping.

## Extension Surface

Agent-authored Rust extensions should be able to provide:

```rust
trait AnalysisExtension {
    fn contribute_types(&self, sink: &mut TypeSink<'_>);
    fn contribute_values(&self, sink: &mut ValueSink<'_>);
    fn contribute_summaries(&self, sink: &mut SummarySink<'_>);
    fn contribute_points_to(&self, sink: &mut PointsToSink<'_>);
    fn contribute_alias_facts(&self, sink: &mut AliasSink<'_>);
}
```

Important constraints:

- extensions can add facts, not silently delete native facts;
- replacement facts require explicit conflict policy;
- extension facts cannot claim higher precision than validation supports;
- extension code digest participates in cache keys;
- extension outputs are validated before merging;
- every extension fact has provenance.

## What Not To Do First

- Do not expose a raw global points-to graph as the public SDK.
- Do not make "alias analysis" a mandatory whole-repo pass.
- Do not rely on Ty, Pyright, CodeQL, WALA, Soot, SVF, or Go tools at runtime.
- Do not let extension facts erase unknowns without preserving evidence.
- Do not claim exact Python/JS dynamic dispatch or reflection support from heuristic type/value facts.
- Do not build high-k context sensitivity before a measured use case requires it.

## First Public Capability Target

The first public-facing target should be narrow:

```text
Types<'_>:
  declared_type(symbol)
  inferred_type(expr/symbol)
  narrowed_type_at(place, cfg_node)
  precision(...)

Values<'_>:
  abstract_value(expr/place)
  allocation_token(expr)
  known_function_object(expr/place)

Aliases<'_>:
  may_alias(place_a, place_b)
  must_alias(place_a, place_b)
  points_to(place)
  precision(...)
```

Only promote these once docs can honestly describe limits.

## Implementation Order Relative To Existing Roadmap

This track depends on:

- semantic index;
- module graph;
- CFG/control dependence;
- analysis kernel;
- evaluation harness;
- extension surface.

After this track, the next strongest research topic is function effects and summaries, because summaries are the bridge from local type/value/alias facts to scalable interprocedural data-flow and call graph precision.
