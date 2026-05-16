# Call Graph Bootstrap Integration

Date: 2026-05-16

## Research Question

Given the implementation bootstrap decision in
`research/implementation-bootstrap/`, how should the call graph design change so
it consumes the private semantic kernel instead of creating a parallel call
graph subsystem?

## Decision

The call graph should be implemented as fact families inside the private
`analysis` module:

```text
analysis::mir
  -> analysis::places
  -> analysis::calls
  -> analysis::summaries
  -> refined call providers
  -> internal graph views
  -> public SDK views later
```

Do **not** implement call graphs first as:

- a standalone `crates/polint/src/facts/calls/` subsystem;
- public `Calls<'_>` / `CallGraph<'_>` SDK views in the first slice;
- an expansion of `FunctionFact.calls: Vec<String>`;
- a generic trait-object provider graph before native fact contracts exist.

The first call graph deliverable is internal:

```text
CallSiteFact
CallTargetFact
UnresolvedCallFact/status
CallStore indexes
semantic cache keys
debug snapshots
evaluation fixtures
```

Public views come only after validation gates are met.

## Evidence Rechecked

| Source | Evidence | Design consequence |
| --- | --- | --- |
| `crates/polint/src/core/mod.rs:142-153` | `FunctionFact` stores `calls: Vec<String>`. | This is a legacy syntactic hint, not a semantic call graph substrate. |
| `crates/polint/src/go/adapter.rs:431-444`, `:549-562` | Go currently extracts sorted/deduped call names from tree-sitter nodes. | Good enough for examples and metrics; insufficient for call-site identity, arguments, receiver places, and unresolved reasons. |
| `crates/polint/src/ts/adapter.rs:1181-1303` | TS/JS recursively collects call names into strings. | Existing traversal can guide MIR lowering, but call facts must own spans, operands, receiver, and call kind. |
| `research/call-graphs/repos/golang-tools/go/callgraph/static/static.go:16-40` | Go static graph follows `StaticCallee`. | Direct call facts should be a first native tier. |
| `research/call-graphs/repos/golang-tools/go/callgraph/rta/rta.go:300-354` | RTA needs explicit roots and reaches a fixed point over functions/types. | Go RTA belongs after entrypoints, summaries, and lifecycle digests exist. |
| `research/call-graphs/repos/golang-tools/go/callgraph/vta/vta.go:5-55`, `:66-82`, `:113-134` | VTA builds a type-propagation graph, is experimental, and refines unresolved calls. | VTA should be an optional refined provider consuming places/type/value facts, not a default baseline. |
| `research/call-graphs/repos/jelly/src/analysis/operations.ts:433-454` | Jelly registers call edges when function tokens bind. | TS/JS refined calls should be value/function-token flow over `PlaceId`, not AST-string matching. |
| `research/call-graphs/repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/Nodes.qll:193-261` | CodeQL exposes potential callees plus imprecision/incompleteness predicates. | polint call facts need status/precision/uncertainty on call sites and edges. |
| `research/call-graphs/repos/pyre-check/source/interprocedural/callGraph.ml:645-664` | Pyre/Pysa stores normal, `__new__`, `__init__`, decorated, higher-order, shim, unresolved, and recognized-call information. | Call facts should separate call target roles and higher-order/recognized-call metadata. |
| `research/call-graphs/repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/TypeIterator.scala:82-132` | OPAL separates type information providers from call graph clients. | polint should separate type/value producers from call target resolvers. |
| `research/call-graphs/repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/CallGraphAnalysis.scala:267-289` | OPAL records incomplete call sites for unresolved invokedynamic. | unresolved call facts should be first-class, not hidden diagnostics only. |

## Revised Architecture

### Internal Module Placement

Use the implementation bootstrap module tree:

```text
crates/polint/src/analysis/
  calls.rs
  mir/
  places.rs
  summaries/
  extensions/
```

Later, when calls grow large, split `analysis/calls.rs` into:

```text
analysis/calls/
  mod.rs
  facts.rs
  store.rs
  direct.rs
  graph_view.rs
  unresolved.rs
  algorithms/
    go_static.rs
    go_rta.rs
    ts_function_tokens.rs
```

Keep this internal. Do not add `sdk::facts::Calls` behavior yet.

### Dependency Direction

The call graph should depend on the bootstrap layers:

```text
SourceFiles
  -> Syntax facts
  -> ModuleGraph / Symbols / References
  -> MIR
  -> Places
  -> CallSites
  -> DirectTargets
  -> DirectSummaries
  -> RefinedCallTargets
  -> Graph views
```

Do not let refined call graph construction become a prerequisite for local MIR,
places, or direct summaries. That creates a cycle.

The safe cycle-breaking rule:

```text
direct call facts feed direct summaries;
direct summaries feed refined call providers later.
```

## Internal Fact Model

### IDs

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallSiteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallTargetId(pub u64);
```

Use dense IDs only as runtime handles. Stable identity belongs in `FactMeta`.

### Call Site Fact

```rust
pub(crate) struct CallSiteFact {
    pub(crate) id: CallSiteId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) caller: FunctionId,
    pub(crate) owner_symbol: Option<SymbolId>,
    pub(crate) body: MirBodyId,
    pub(crate) op: MirOpId,
    pub(crate) span: Span,
    pub(crate) kind: CallSyntaxKind,
    pub(crate) callee: CallCallee,
    pub(crate) receiver: Option<PlaceId>,
    pub(crate) arguments: Vec<PlaceId>,
    pub(crate) result: Option<PlaceId>,
}
```

`CallCallee` should describe the semantic shape, not just display text:

```rust
pub(crate) enum CallCallee {
    Identifier { reference: Option<ReferenceId>, name: String },
    Member { base: PlaceId, property: String },
    Index { base: PlaceId, index: Option<PlaceId> },
    Super,
    Import,
    FunctionValue { place: PlaceId },
    Constructor { reference: Option<ReferenceId>, name: Option<String> },
    Unknown { reason: UnresolvedCallReason },
}
```

Why this shape:

- direct binding can use `ReferenceId`;
- TS/JS and Python value-flow can use `FunctionValue { place }`;
- Go/JVM method dispatch can use `receiver`;
- framework and extension models can bind call sites without reparsing AST text.

### Call Target Fact

Use one target fact per possible callee.

```rust
pub(crate) struct CallTargetFact {
    pub(crate) id: CallTargetId,
    pub(crate) site: CallSiteId,
    pub(crate) caller: FunctionId,
    pub(crate) target_function: Option<FunctionId>,
    pub(crate) target_symbol: Option<SymbolId>,
    pub(crate) synthetic_target: Option<SyntheticCallableId>,
    pub(crate) edge_kind: CallEdgeKind,
    pub(crate) algorithm: CallAlgorithm,
    pub(crate) status: CallTargetStatus,
    pub(crate) reason: Option<UnresolvedCallReason>,
}
```

The sidecar `FactMeta` carries stable key, producer, precision, confidence,
validation, and evidence.

Statuses:

```rust
pub(crate) enum CallTargetStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}
```

Algorithms:

```rust
pub(crate) enum CallAlgorithm {
    SyntaxOnly,
    DirectReference,
    ImportBinding,
    StaticMember,
    GoStatic,
    GoCha,
    GoRta,
    GoVta,
    FunctionTokenFlow,
    TypeHierarchy,
    PointsTo,
    SummaryAssisted,
    FrameworkModel,
    RepoModel,
}
```

### Unresolved Facts

Do not represent unresolved calls only as `CallTargetFact { target: None }` if
that makes querying awkward. Either keep unresolved as a target status or add:

```rust
pub(crate) struct UnresolvedCallFact {
    pub(crate) site: CallSiteId,
    pub(crate) reason: UnresolvedCallReason,
    pub(crate) missing_input: Option<FactFamily>,
}
```

The important invariant is that unresolved calls are countable, queryable,
cacheable, and attributable.

## Stable Keys

Call site stable key:

```text
family = "call_site"
language
file stable key
caller stable key
span byte range
callee shape
ordinal among same-span call expressions
```

Call target stable key:

```text
family = "call_target"
call_site stable key
algorithm
target stable key or unresolved reason
provider id/version
model id if extension-provided
```

Rules:

- stable key generation must not rely on dense `FunctionId` alone;
- direct target facts should remain stable when unrelated files are added;
- extension-provided synthetic targets must include extension/model identity;
- generated/synthetic entrypoints must not collide with real source functions.

## Call Store

```rust
pub(crate) struct CallStore {
    sites: Vec<CallSiteFact>,
    site_meta: Vec<FactMeta>,
    targets: Vec<CallTargetFact>,
    target_meta: Vec<FactMeta>,
    sites_by_caller: BTreeMap<FunctionId, Vec<usize>>,
    targets_by_site: BTreeMap<CallSiteId, Vec<usize>>,
    incoming_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    outgoing_by_function: BTreeMap<FunctionId, Vec<usize>>,
    unresolved_by_reason: BTreeMap<UnresolvedCallReason, Vec<usize>>,
}
```

Use `BTreeMap` first to preserve deterministic iteration. Switch to a hash map
only after benchmark evidence and deterministic output normalization exist.

## Provider Tiers

### Tier 0: Call Site Extraction

Inputs:

- MIR bodies;
- places;
- source spans.

Outputs:

- `CallSiteFact`;
- unresolved status for syntactically unsupported call forms.

Complexity: `O(MIR ops)`.

Default: yes.

### Tier 1: Direct Target Resolution

Inputs:

- call sites;
- symbols/references;
- resolved imports;
- places for receiver/callee expression.

Outputs:

- direct/binding `CallTargetFact`;
- unresolved reasons for dynamic/interface/callable-value calls.

Complexity: near `O(call_sites * lookup_cost)`.

Default: yes.

### Tier 2: Direct Summary Integration

Inputs:

- direct call targets;
- direct summaries.

Outputs:

- summary call effects;
- summary dependency edges;
- no broad new dynamic target inference yet.

Default: yes after summaries exist.

Important: this stage should not require refined call graph output. It prevents
the call graph/summary cycle from blocking the bootstrap.

### Tier 3: Framework And Repo-Local Model Targets

Inputs:

- call sites;
- symbols/references;
- module/dependency graph;
- places;
- entrypoints/trust-boundary facts;
- extension manifests.

Outputs:

- framework dispatch edges;
- synthetic entrypoint edges;
- repo-model call targets;
- diagnostics for unbound models.

Default: built-in low-risk recognizers only. Repo models explicit.

### Tier 4: Type/Value Refined Targets

Inputs:

- places;
- type facts;
- value facts;
- allocation tokens;
- summaries;
- direct targets.

Outputs:

- Go interface/method targets;
- TS/JS function-token targets;
- future Python callable/MRO targets;
- future Java hierarchy/RTA targets.

Default: opt-in until benchmarked.

### Tier 5: Points-To / Context-Sensitive Targets

Inputs:

- points-to constraints;
- alias query service;
- summaries;
- language-specific lifecycle roots.

Outputs:

- refined target sets;
- budget-exceeded facts.

Default: no. This is experimental/research mode until evaluation proves value.

## Provider Scheduling

Do not start with a public `CallFactsProvider` trait. Start with internal
provider IDs:

```rust
pub(crate) enum ProviderId {
    Mir,
    Places,
    CallSites,
    DirectCallTargets,
    DirectSummaries,
    FrameworkCallModels,
    GoStaticCalls,
    GoRtaCalls,
    TsFunctionTokenCalls,
}
```

Native execution can be an enum `match` in the scheduler. This matches the
implementation bootstrap: static dispatch in hot native paths, dynamic behavior
only at extension boundaries.

## Cache Keys

Call-site layer key:

```text
artifact = "call-sites"
schema = "call-sites-v1"
mir layer digest
place layer digest
provider version
semantic schema
source digest
language lifecycle digest
```

Direct-target layer key:

```text
artifact = "direct-call-targets"
schema = "direct-call-targets-v1"
call-site layer digest
symbol/reference layer digest
module graph digest
provider version
semantic schema
config/lifecycle digest
```

Model-target layer key:

```text
artifact = "model-call-targets"
schema = "model-call-targets-v1"
call-site layer digest
symbol/reference layer digest
place/type/value digests used by model
extension manifest digest
model source digest
validation policy digest
```

Refined semantic target layer key:

```text
artifact = "refined-call-targets"
schema = "<algorithm>-v1"
call-site/direct-target digests
type/value/place/summary digests
entrypoint/root digest
algorithm parameters
provider version
budget settings
```

Do not include `rule_digest` unless rule options actually affect the requested
analysis tier or model activation. This follows the analysis-kernel cache
research.

## Extension Sink

The first extension contract is a sink, not dynamic loading:

```rust
pub(crate) struct CallModelSink<'a> {
    calls: &'a mut CallStoreBuilder,
    validator: &'a CallModelValidator,
}

impl<'a> CallModelSink<'a> {
    pub(crate) fn add_target(&mut self, spec: CallTargetSpec) -> Result<(), ModelError>;
    pub(crate) fn add_synthetic_entrypoint(&mut self, spec: SyntheticEntrypointSpec) -> Result<(), ModelError>;
    pub(crate) fn unresolved(&mut self, site: CallSiteId, reason: UnresolvedCallReason);
}
```

Validation rules:

- referenced `CallSiteId` must exist;
- target function/symbol/synthetic target must exist or be created through a
  synthetic target sink;
- native exact targets cannot be deleted or silently shadowed;
- contradictory extension exact targets become conflicts unless the edge kind is
  explicitly additive;
- every emitted fact carries provider/model provenance;
- suppressive or negative facts are not allowed in the first implementation.

## Language-Specific Revised Path

### Go

1. Internal call sites from MIR.
2. Direct reference/import/static targets from symbols/references.
3. Reuse the existing Go lifecycle contract for module roots, package patterns,
   build tags, and tests.
4. Optional official Go toolchain/x/tools provider for SSA static/RTA when
   lifecycle setup is available.
5. Go VTA only after `PlaceId`, type/value facts, and evaluation fixtures exist.

Rationale: Go `static` and `RTA` source code shows the algorithm boundary
depends on SSA program roots and reachable function/type fixed points. That is
not the first bootstrap layer.

### TypeScript / JavaScript

1. Internal call sites from MIR lowered from Oxc.
2. Direct lexical/import/static member targets from symbols/references.
3. Keep dynamic member/property calls unresolved with explicit reasons.
4. Add bounded function-token flow after value facts and summaries exist.
5. Built-in framework recognizers and repo models can add high-value edges
   earlier than broad whole-program value-flow.

Rationale: Jelly and CodeQL both show that function/value flow is the core hard
part, and CodeQL exposes uncertainty instead of pretending every call is exact.

### Python

Future path:

1. AST/MIR call sites;
2. module/name binding;
3. function-token/name points-to;
4. decorated/higher-order/shim/unresolved facts;
5. application-centered roots.

Rationale: Pyre/Pysa's call representation separates decorated targets,
higher-order parameters, shim targets, and unresolved status. polint should
mirror that separation instead of a flat edge list.

### Java / JVM

Future path:

1. source/bytecode declaration and method facts;
2. hierarchy facts;
3. type producer abstraction;
4. CHA/RTA as call target resolvers;
5. points-to/context-sensitive providers later.

Rationale: OPAL's `TypeIterator`/call resolver split is the right modular
shape. It also keeps algorithm variation out of the base call-site layer.

## Public SDK Promotion

Do not expose `Calls<'_>` or functional `CallGraph<'_>` in the first
implementation.

Promotion order:

1. internal debug snapshots;
2. internal evaluation fixtures;
3. `Calls<'_>` for call-site iteration only;
4. `CallGraph<'_>` for direct/binding targets and unresolved sites;
5. tier filters and refined providers.

Before `Calls<'_>`:

- docs under `docs/facts/calls.md`;
- temp-repo rule test using only `polint::sdk::prelude::*`;
- capability support diagnostics;
- cache digest tests;
- deterministic snapshots;
- uncertainty wording.

Before `CallGraph<'_>`:

- graph edge fixture schema;
- target status/precision API;
- default-vs-extension debug delta;
- unresolved query API;
- benchmark gates for any non-direct tier.

## Revised First Implementation Task

Implement only the internal direct-call bootstrap:

1. Add `analysis::calls` store types and IDs.
2. Add stable-key generation for call sites and call targets.
3. Add MIR call op lowering hooks for Go and TS/JS.
4. Build `CallSiteFact` from MIR call ops.
5. Build direct/binding `CallTargetFact` where `ReferenceId`/`SymbolId` is
   known.
6. Emit unresolved facts for everything else.
7. Add debug snapshots and evaluation fixtures.
8. Add cache key tests.
9. Add extension sink validation tests.

Do not expose public SDK views in this task.

## Risks And Watchpoints

| Risk | Mitigation |
| --- | --- |
| Call graph duplicates MIR place/call logic. | Make call sites derive from MIR ops and `PlaceId`, not parser AST strings. |
| Summary/call graph cycle blocks implementation. | Direct calls feed direct summaries; refined calls consume summaries later. |
| Dynamic JS/Python calls look like false negatives. | Emit unresolved facts with reasons and extension suggestions. |
| Go official tooling creates sidecar-like dependency concerns. | Treat official Go toolchain/x/tools as optional language-authority provider input, not as polint's core engine. |
| Public SDK freezes weak model. | Keep internal until docs, fixtures, cache tests, and temp-repo tests exist. |
| Extension models create unsound certainty. | Merge through sink validation, provenance, confidence, and conflict diagnostics. |
