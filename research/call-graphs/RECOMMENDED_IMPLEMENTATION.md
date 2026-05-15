# Recommended Implementation: Native Call Graph Engine

Date: 2026-05-15

## Decision

Build a native Rust call-facts and call-graph engine inside polint.

"Native" here means:

- no external program-analysis engines;
- no CodeQL, Soot, WALA, Doop, PyCG, Jelly, JVM sidecars, Python sidecars, or service-based analysis;
- no dependency on another tool's call graph as the source of truth;
- algorithms are implemented in polint and emit polint facts directly.

Existing parser frontends should still be treated as language adapters. Oxc and tree-sitter are parser infrastructure, not call-graph engines. Replacing parsers is a different project and would delay the product without improving the call-graph architecture.

## Product Goal

Support full project call graphs while being honest about precision.

The product should be able to answer:

- What calls exist in this repository?
- Which calls are direct and confidently resolved?
- Which calls are dynamic, ambiguous, unsupported, or setup-missing?
- Which functions call this function?
- Which functions can this function reach?
- Which algorithm produced this edge?
- Which repo-local model produced this edge, if any?
- Which unresolved calls are good candidates for an agent-authored model?
- How does the graph change when an experimental algorithm is enabled?
- How does the graph change when repo-specific models are enabled?

The right model is not "one exact graph." The right model is a layered fact system that can materialize several graph views from the same evidence.

## Research-Driven Precision Defaults

The reports should be read as a precision/cost recommendation, not just an implementation sketch. The research argues for this default ordering:

| Tier | Default? | Expected cost | Expected accuracy | Why |
|---|---:|---|---|---|
| Syntactic call sites | Yes | `O(AST)` | Complete for call expressions, no target accuracy. | Gives every rule and later provider a stable base. |
| Direct lexical/import/static binding | Yes | Near-linear with indexes. | High precision, limited recall. | Best first value for Go and TS/JS. |
| Unresolved/dynamic facts | Yes | Near-linear. | Not a target graph, but prevents hidden false negatives. | Research shows missing edges are the dangerous failure mode. |
| Go SSA static/RTA | Opt-in then promote | Package/SSA setup plus worklist. | Good precision when roots and module lifecycle are correct. | Go is the best first semantic target. |
| TS/JS function-token flow | Opt-in | Fixed point over assignment/property/import graph. | Better callback/dynamic call recall, more false-positive risk. | JS research shows no single precise static graph dominates. |
| Java CHA/RTA | Future opt-in | Classpath + hierarchy/reachability. | Useful only with explicit classpath/entrypoint/reflection policy. | Java research shows algorithm names are not enough. |
| Points-to/context-sensitive providers | Experimental | Potentially high memory/time. | Highest precision for selected cases, but can hit tractability walls. | Should be benchmarked, not silently enabled. |

Default CLI/rule behavior should include the first three tiers only. Higher tiers should be selected by rule capability, config, or explicit experimental flags until benchmarks show they are cheap and stable enough.

## Accuracy Reporting Requirements

Every provider run should emit debug counters:

```text
call_sites_total
edges_total
edges_by_algorithm
edges_by_status
unresolved_by_reason
runtime_ms
cache_hit_rate
```

On benchmark fixtures, also compute:

```text
precision
recall
false_positive_edges
false_negative_edges
graph_delta_from_lower_tier
```

This is directly motivated by the research: graph size is not enough, and even "more precise" algorithms can add or lose true edges depending on feature modeling.

## Core Architecture

```text
language parser
  -> declaration/scope/reference facts
  -> syntactic call-site facts
  -> language-specific resolution providers
  -> repo-local call graph models
  -> normalized call-edge facts
  -> project call graph view
  -> SDK queries, diagnostics, debug exporters
```

The implementation should have five internal pieces:

1. **Call site extraction**
   Emit every syntactic call expression, including unresolved calls.

2. **Symbol and binding resolution**
   Resolve direct lexical, import, method, constructor, and static calls.

3. **Semantic call providers**
   Add language-specific algorithms such as CHA, RTA, VTA, method-set lookup, MRO lookup, and value-flow.

4. **Repo-local call graph models**
   Bind agent-authored framework, lifecycle, generated-code, callback, router, DI, and tool-registration models to native facts.

5. **Graph materialization**
   Build whole-repo and filtered call graphs from normalized call facts.

6. **Public SDK views**
   Expose stable query views such as `Calls<'_>` and `CallGraph<'_>`, not internal graph structures.

## Fact Model

Keep the persisted internal facts small, stable, and explicit about uncertainty.

```rust
struct CallSiteFact {
    id: CallSiteId,
    language: LanguageId,
    file_id: FileId,
    span: TextRange,
    enclosing_symbol: Option<SymbolId>,
    callee_text: StringId,
    receiver_text: Option<StringId>,
    call_kind: CallSyntaxKind,
}

enum CallSyntaxKind {
    Function,
    Method,
    Constructor,
    Super,
    DynamicImport,
    Require,
    JSXComponent,
    CallableObject,
    Unknown,
}

struct CallEdgeFact {
    site_id: CallSiteId,
    caller: Option<SymbolId>,
    target: Option<SymbolId>,
    edge_kind: CallEdgeKind,
    resolution: ResolutionStatus,
    algorithm: CallAlgorithm,
    confidence: Confidence,
    provider: ProviderId,
    model_id: Option<ModelId>,
    provenance: Provenance,
    validation: ValidationStatus,
    reason: Option<UnresolvedReason>,
}

enum ResolutionStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    Unsupported,
    SetupMissing,
}

enum CallAlgorithm {
    Syntax,
    Binding,
    Cha,
    Rta,
    Vta,
    ValueFlow,
    Mro,
    Heuristic,
    RepoModel,
}

enum Provenance {
    Native,
    BuiltinModel,
    RepoModel,
    AgentGeneratedModel,
}

enum ValidationStatus {
    Native,
    Validated,
    Unvalidated,
    Failed,
}
```

Important rule: a call site may have zero, one, or many target edges. Many targets are not a bug. They are how conservative static analysis represents dynamic dispatch.

## Graph Model

The full call graph is a view over facts.

```rust
struct ProjectCallGraph {
    nodes: SymbolGraph,
    edges: CallEdgeIndex,
    unresolved: UnresolvedCallIndex,
}
```

Recommended graph queries:

```rust
impl<'a> CallGraph<'a> {
    pub fn call_sites(&self) -> impl Iterator<Item = CallSite<'a>>;
    pub fn outgoing(&self, caller: SymbolId) -> impl Iterator<Item = CallEdge<'a>>;
    pub fn incoming(&self, callee: SymbolId) -> impl Iterator<Item = CallEdge<'a>>;
    pub fn targets(&self, site: CallSiteId) -> impl Iterator<Item = CallTarget<'a>>;
    pub fn unresolved(&self) -> impl Iterator<Item = CallSite<'a>>;
    pub fn by_algorithm(&self, algorithm: CallAlgorithm) -> CallGraph<'a>;
}
```

The graph should include unknown targets explicitly:

```text
src/api/handler.go:42      -> UserService.Save
src/api/handler.go:51      -> <unresolved: interface dispatch needs semantic provider>
src/web/routes.ts:88       -> <unresolved: dynamic import path>
src/web/controller.ts:114  -> validateUser
```

This gives users a real full-project graph without pretending dynamic languages are exact.

## Provider Contract

Use a provider registry keyed by language and algorithm.

```rust
trait CallFactsProvider {
    fn language(&self) -> LanguageId;
    fn algorithm(&self) -> CallAlgorithm;
    fn required_capabilities(&self) -> CapabilitySet;

    fn emit_call_sites(
        &self,
        ctx: &AnalysisCtx,
        file: FileId,
        sink: &mut CallSiteSink,
    ) -> ProviderResult<()>;

    fn resolve_edges(
        &self,
        ctx: &AnalysisCtx,
        sites: &CallSiteStore,
        sink: &mut CallEdgeSink,
    ) -> ProviderResult<()>;
}
```

Providers must be additive. A binding provider and an RTA provider can both emit candidates for the same call site. The fact model records which algorithm produced which edge.

## Repo-Local Model Contract

Repo-local models are native polint inputs, not external analyzer plugins. They should be parseable, validated, digestible, and explainable.

```rust
struct CallGraphModel {
    id: ModelId,
    language: LanguageId,
    scope: GlobSet,
    resolvers: Vec<ModelResolver>,
    evidence: Vec<ModelEvidence>,
    validation: Vec<ModelValidationCase>,
}

enum ModelResolver {
    RouteRegistration {
        receiver: SymbolMatcher,
        methods: Vec<NameId>,
        handler_arg: usize,
        entrypoint_kind: EntrypointKind,
    },
    DecoratorRegistration {
        decorator: SymbolMatcher,
        entrypoint_kind: EntrypointKind,
    },
    DependencyInjection {
        interface: TypeMatcher,
        implementations: Vec<TypeMatcher>,
    },
    CallbackRegistry {
        register_call: SymbolMatcher,
        callback_arg: usize,
    },
    GeneratedClient {
        client_symbol: SymbolMatcher,
        remote_boundary: BoundaryKind,
    },
}
```

Model resolvers should bind to existing source facts before they can emit edges. A model that cannot bind should produce a model diagnostic, not silent graph changes.

```python
for model in repo_models:
    bindings = bind_model_to_symbols(model, symbols, calls, types)
    if bindings.has_errors():
        emit_model_diagnostic(model, bindings.errors)
        continue

    for edge in model_edges(model, bindings):
        emit_edge(edge, algorithm="RepoModel", provenance="RepoModel", model_id=model.id)
```

High-confidence model promotion should require fixtures or an explicit unvalidated status. Debug output must separate native edges from model edges.

## Shared Native Algorithms

Implement reusable algorithm building blocks once, then let each language adapter provide semantic hooks.

### Worklist Fixed Point

Used by RTA, VTA, JavaScript value-flow, and Python name-points-to.

```python
worklist = initial_constraints()

while worklist:
    item = worklist.pop()
    changed = apply(item)
    if changed:
        worklist.extend(dependents(item))
```

### CHA

Class Hierarchy Analysis resolves virtual/interface calls by type hierarchy.

```python
def cha_targets(call):
    receiver_type = static_receiver_type(call)
    method_name = call.method_name

    for subtype in all_subtypes(receiver_type):
        if subtype.defines_or_inherits(method_name):
            emit_edge(call, subtype.method(method_name), algorithm="Cha")
```

CHA is conservative and often over-approximates. It is useful before a precise reachability or allocation model exists.

### RTA

Rapid Type Analysis restricts dynamic dispatch to allocated reachable concrete types.

```python
reachable_functions = roots()
allocated_types = set()
worklist = reachable_functions

while worklist:
    function = worklist.pop()

    for allocation in allocations(function):
        allocated_types.add(allocation.type)

    for call in calls(function):
        for target in dispatch_targets(call, allocated_types):
            if target not in reachable_functions:
                reachable_functions.add(target)
                worklist.push(target)
```

RTA is the best first "serious" native algorithm for Go and Java-like languages.

### VTA

Variable Type Analysis propagates possible receiver types through variables, fields, params, returns, and assignments.

```python
for assignment in assignments:
    add_constraint(type_set(assignment.rhs) <= type_set(assignment.lhs))

for call in calls:
    for target in method_targets(type_set(call.receiver), call.method):
        emit_edge(call, target, algorithm="Vta")
```

VTA is more precise than RTA but more expensive and more implementation-heavy. Treat it as experimental until the graph harness can compare outputs.

### Function-Token Value Flow

Used for TS/JS and Python callable values.

```python
for declaration in function_declarations:
    tokens[var(declaration.name)].add(FunctionToken(declaration.symbol))

for assignment in assignments:
    add_constraint(tokens[assignment.rhs] <= tokens[assignment.lhs])

for call in calls:
    for token in tokens[call.callee]:
        emit_edge(call, token.symbol, algorithm="ValueFlow")
```

This should be bounded. Do not try to model the entire JavaScript or Python runtime in the first native implementation.

## Shared Language Semantics Interface

To make new languages easy, separate generic algorithms from language-specific semantics.

```rust
trait LanguageCallSemantics {
    fn declarations(&self) -> DeclarationIndex;
    fn scopes(&self) -> ScopeIndex;
    fn references(&self) -> ReferenceIndex;
    fn call_sites(&self) -> CallSiteIndex;

    fn resolve_reference(&self, reference: ReferenceId) -> Resolution;
    fn direct_callable(&self, call: CallSiteId) -> Option<SymbolId>;
    fn receiver_type(&self, call: CallSiteId) -> Option<TypeId>;
    fn allocated_types(&self, symbol: SymbolId) -> impl Iterator<Item = TypeId>;
    fn method_candidates(&self, receiver: TypeId, name: NameId) -> CandidateSet;
    fn dynamic_reason(&self, call: CallSiteId) -> Option<UnresolvedReason>;
}
```

A new language should get basic call support by implementing:

1. call-site extraction;
2. declarations;
3. lexical scopes;
4. reference binding;
5. direct target resolution.

It should get advanced graph support by adding:

1. type hierarchy;
2. receiver type extraction;
3. allocation extraction;
4. method lookup semantics;
5. callable value-flow hooks.

## Recommended Module Layout

Exact file paths should follow the existing crate layout, but the ownership boundaries should look like this:

```text
crates/polint/src/facts/calls/
  mod.rs
  model.rs
  provider.rs
  store.rs
  graph.rs
  sdk.rs
  algorithms/
    binding.rs
    cha.rs
    rta.rs
    vta.rs
    subset_flow.rs
  languages/
    go.rs
    ts_js.rs
    python.rs
    java.rs
```

Keep most of this `pub(crate)`. The supported public surface should be the SDK fact views and documented behavior under `docs/facts/`.

## Language Plan

### Go

Go is the best first language for serious native semantic call graphs.

Build in this order:

1. syntax call sites from current Go parser;
2. package/file declaration index;
3. lexical scopes and identifier references;
4. import path and package member resolution;
5. method declarations and receiver type index;
6. interface declarations and method sets;
7. direct/static call resolution;
8. CHA for interface dispatch;
9. RTA using reachable functions and allocated concrete types;
10. VTA as experimental refinement.

Native Go semantic support needs a real internal model:

- packages and import paths;
- file scopes, package scopes, function scopes, block scopes;
- named types, pointers, structs, interfaces, aliases, and type parameters;
- method sets for value and pointer receivers;
- embedded fields;
- interface satisfaction;
- allocation sites from composite literals, `new`, address-taking, and constructors;
- call roots from `main`, tests, exported package APIs, and configured entrypoints.

The first high-value version is Go direct binding plus CHA. The first strong version is Go RTA.

### TypeScript and JavaScript

TS/JS should start with complete call-site coverage and bounded value-flow.

Build in this order:

1. syntax call sites from Oxc;
2. lexical scope and binding index;
3. module import/export resolution;
4. direct function and constructor resolution;
5. class/static method table;
6. object literal method table;
7. function-token propagation for assignments, imports, exports, params, returns, and callbacks;
8. limited `call`, `apply`, and `bind`;
9. dynamic unresolved reasons for property access, `eval`, proxies, decorators, dynamic imports, and framework reflection.

Do not promise exact JS/TS call graphs. Promise complete call-site graphs and progressively resolved edges.

### Python

Python should come after the shared call engine and TS/JS value-flow are mature.

Build in this order:

1. AST call sites;
2. module/import/name binding;
3. class declarations and MRO;
4. function-token propagation through assignments and imports;
5. method lookup through `self`, class constructors, and known instances;
6. callable object handling through `__call__`;
7. unresolved facts for dynamic imports, monkey patching, decorators, metaclasses, and reflective access.

Python will be useful if it is honest. It will be misleading if it claims exactness.

### Java

Java should be added after the native type-hierarchy infrastructure exists.

Build in this order:

1. source or bytecode declaration index;
2. packages, imports, classes, interfaces, methods, fields;
3. overload and override resolution;
4. class hierarchy;
5. CHA;
6. RTA;
7. optional points-to-style refinement.

If the implementation must be fully native, Java requires native classpath/type-hierarchy handling. Do not start here unless Java is the business priority.

## Configuration

Make algorithms selectable per language.

```toml
[facts.call_graph.go]
enabled = true
algorithm = "rta" # syntax | binding | cha | rta | vta

[facts.call_graph.typescript]
enabled = true
algorithm = "value_flow" # syntax | binding | value_flow
max_iterations = 8
record_dynamic_unknowns = true

[facts.call_graph.python]
enabled = false
algorithm = "binding"

[facts.call_graph.java]
enabled = false
algorithm = "cha"

[[facts.call_graph.models]]
path = ".polint/models/call_graph.toml"
required_validation = "warn" # off | warn | error
```

Config that affects results must participate in deterministic cache digests:

- selected algorithm;
- max iterations;
- language lifecycle settings;
- entrypoints;
- include tests;
- build tags;
- module roots;
- package patterns;
- dependency/module resolution settings;
- repo model file contents;
- model validation policy;
- enabled/disabled model ids.

## Public SDK

Rule authors should consume typed fact views.

```rust
#[polint::rule]
fn no_unsafe_calls(ctx: &mut RuleCtx<'_>, graph: CallGraph<'_>) -> RuleResult {
    for edge in graph.edges() {
        if edge.confidence() >= Confidence::Medium && edge.target_name().ends_with(".Unsafe") {
            ctx.diagnostic(edge.span(), "avoid unsafe call");
        }
    }

    Ok(())
}
```

Do not expose:

- internal AST nodes;
- provider implementations;
- raw graph storage;
- mutable call graph builders;
- broad fact access through `RuleCtx`.

Document this under `docs/facts/call-graph.md` before advertising it.

## Testing and Validation

Add a graph harness before adding sophisticated algorithms.

Required tests:

- fixture snapshots for call sites;
- fixture snapshots for resolved edges;
- unresolved call snapshots;
- algorithm comparison snapshots;
- temp-repo rule-author tests using only public SDK imports;
- cache digest regression tests for every config input;
- panic containment tests for malformed source and parser errors.

Recommended debug output:

```bash
polint debug call-graph --algorithm binding --format json
polint debug call-graph --algorithm rta --diff binding
polint debug call-sites --format json
```

Keep debug commands hidden or explicitly unstable until the output format is intentionally public.

## Performance Strategy

Use IDs, arenas, interning, and compact indexes.

Recommended internal indexes:

- `CallSiteId -> CallSiteFact`;
- `SymbolId -> outgoing CallEdgeId`;
- `SymbolId -> incoming CallEdgeId`;
- `CallSiteId -> candidate CallEdgeId`;
- `FileId -> CallSiteId`;
- `Algorithm -> CallEdgeId`;
- `ResolutionStatus -> CallSiteId`.

Run extraction per file, then resolution per package/module, then graph assembly at repository scope.

Use deterministic parallelism:

- parse files in parallel;
- extract declarations and call sites in parallel;
- merge facts in stable order;
- run fixed-point algorithms with deterministic worklist ordering when outputs are snapshotted.

## Milestones

### Milestone 1: Native Call Fact Foundation

Deliver:

- internal `CallSiteFact`;
- internal `CallEdgeFact`;
- provider registry;
- graph store and indexes;
- `Calls<'_>` SDK view;
- `CallGraph<'_>` SDK view;
- docs for current limits;
- fixture snapshots.

This milestone enables full syntactic call-site graphs.

### Milestone 2: Go and TS/JS Binding Graphs

Deliver:

- Go syntax call sites;
- TS/JS syntax call sites;
- direct lexical binding;
- import/export resolution where existing facts support it;
- unresolved facts for unsupported dynamic cases;
- rule-author temp-repo tests.

This milestone enables useful full project call graphs for direct calls.

### Milestone 3: Native Go Semantic Graph

Deliver:

- Go package/declaration/type model;
- method sets;
- interface satisfaction;
- direct method resolution;
- CHA;
- graph snapshots on real Go fixtures.

This milestone enables conservative Go whole-program call graphs.

### Milestone 4: Native Go RTA

Deliver:

- entrypoint configuration;
- allocation extraction;
- reachable function worklist;
- RTA dynamic dispatch filtering;
- comparison output against CHA.

This milestone is the first strong native call-graph algorithm.

### Milestone 5: Native TS/JS Value Flow

Deliver:

- function-token model;
- subset constraint solver;
- assignment/import/export propagation;
- class/object method support;
- bounded callback support;
- explicit dynamic unknowns.

This milestone makes TS/JS call graphs meaningfully better than syntax and binding.

### Milestone 6: Experiment Harness

Deliver:

- hidden debug exporters;
- algorithm diffing;
- precision/recall fixture labels where practical;
- performance benchmarks;
- corpus regression tests.

This milestone makes it easy to change algorithms without guessing.

### Milestone 7: Python or Java

Choose based on product need.

Pick Python if dynamic-language coverage matters more.
Pick Java if enterprise static-language precision matters more.

Do not add both until the shared engine and harness are stable.

## What Not To Do

- Do not build Java or Python before the shared engine proves itself on Go and TS/JS.
- Do not expose raw graph internals as public API.
- Do not hide unresolved calls.
- Do not claim exact graphs for TS/JS or Python.
- Do not make semantic setup mandatory for rules that only need call-site names.
- Do not make one language's lifecycle model leak into another language.
- Do not couple the public SDK to any one algorithm.
- Do not call an algorithm "state of the art" unless the docs state the actual limits.

## Recommended First Implementation Task

Implement Milestone 1 and the smallest useful slice of Milestone 2:

1. add the internal call fact model;
2. add call-site extraction for Go and TS/JS;
3. add `Calls<'_>` as a public SDK view;
4. add `CallGraph<'_>` with unresolved and direct-bound edges only;
5. add honest docs under `docs/facts/call-graph.md`;
6. add temp-repo tests that consume the SDK like an external rule author;
7. add fixture snapshots for graph output.

That creates the permanent architecture for full call graphs while keeping the first shippable version small enough to review and verify.
