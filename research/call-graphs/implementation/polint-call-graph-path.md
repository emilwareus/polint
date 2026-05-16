# Polint Call Graph Implementation Path

Revision: 2026-05-16. The implementation path is now aligned with
`research/implementation-bootstrap/` and
`implementation/BOOTSTRAP-INTEGRATION.md`.

## Product Decision

Do not ship "the call graph" as if it were exact. Ship typed fact views that expose call sites, resolved targets, algorithm provenance, and uncertainty.

Also do not treat framework and lifecycle precision as something the native engine must always auto-discover. polint should support repo-local call graph models that agents can author for the specific repository, with validation and provenance.

Recommended SDK shape:

- `Calls<'_>`: cheap syntactic call-site view.
- `CallGraph<'_>`: resolved call-edge view.
- `CallTargets<'_>` or methods on `CallGraph<'_>`: target lookup by call site, caller, callee, symbol, and confidence.

These are promotion targets, not the first implementation step. The first step
is internal `analysis::calls` facts derived from MIR and `PlaceId`.

## Internal Fact Model

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
    Protocol,
    Unknown,
}

struct CallTargetFact {
    site_id: CallSiteId,
    caller: Option<SymbolId>,
    target: Option<SymbolId>,
    edge_kind: CallEdgeKind,
    resolution: ResolutionStatus,
    algorithm: CallGraphAlgorithm,
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

enum CallGraphAlgorithm {
    Syntax,
    Binding,
    GoSsaStatic,
    GoCha,
    GoRta,
    GoVta,
    JsValueFlow,
    JavaCha,
    JavaRta,
    JavaPointsTo,
    PythonNamePointsTo,
    PythonMro,
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

## Provider Contract

Bootstrap revision: do not start with this as a public or extension-facing
trait. Use internal provider IDs and enum dispatch first. Treat this trait shape
as a later refactoring target if native provider variation needs it.

```rust
trait CallFactsProvider {
    fn language(&self) -> LanguageId;
    fn algorithm(&self) -> CallGraphAlgorithm;
    fn required_capabilities(&self) -> &[Capability];

    fn emit_call_sites(
        &self,
        ctx: &AnalysisCtx,
        file: FileId,
        sink: &mut dyn FactSink,
    ) -> ProviderResult<()>;

    fn resolve_edges(
        &self,
        ctx: &AnalysisCtx,
        facts: &FactStore,
        sink: &mut dyn FactSink,
    ) -> ProviderResult<()>;
}
```

Providers should be additive. Multiple providers can emit candidates for the same call site, as long as their algorithm and confidence differ.

Repo-local model providers are also additive, but their facts must keep `model_id` and validation status. A model edge should never be indistinguishable from a native binding edge.

## Phase Plan

### Phase A: Direct Call Sites

Goal: every supported language emits call-site facts.

Implementation:

```python
for file in supported_files:
    ast = parse(file)
    for call in language_adapter.call_expressions(ast):
        emit(CallSiteFact(...))
        emit(CallTargetFact(
            site_id=call.id,
            target=direct_syntax_target_if_trivial(call),
            algorithm="Syntax",
            confidence="high" if target else "unknown",
            resolution="Resolved" if target else "Unresolved",
        ))
```

Go:
- tree-sitter direct extraction now;
- optional SSA static provider later.

TS/JS:
- Oxc extraction;
- direct lexical/import binding when available.

### Phase B: Symbol-Bound Direct Edges

Depends on symbol/reference facts. Resolve:

- local functions;
- imported functions;
- class/static methods when receiver type is syntactically known;
- Go direct SSA static callees;
- TS/JS exported/imported functions.

```python
def bind_call_site(site, symbols, references, imports):
    if site.callee_is_identifier:
        return resolve_reference(site.callee_ref, symbols, imports)
    if site.callee_is_static_member and receiver_symbol_known(site):
        return lookup_member(receiver_symbol(site), site.member_name)
    return unresolved("needs_dynamic_dispatch")
```

### Phase C: Go Semantic Provider

Add `go/packages` and `go/ssa` provider behind explicit capability diagnostics.

Modes:

- `static`: exact direct SSA calls.
- `cha`: library/partial program mode.
- `rta`: configured main/test roots.
- `vta`: experimental.

Config sketch:

```toml
[languages.go.call_graph]
enabled = true
algorithm = "static" # static | cha | rta | vta
package_patterns = ["./..."]
include_tests = true
build_tags = []
```

Cache digest must include:

- module roots;
- package patterns;
- include tests;
- build tags;
- algorithm;
- Go version/toolchain if available;
- dependency graph metadata if captured;
- repo model file contents;
- enabled model ids;
- model validation policy.

### Phase D: JS/TS Value-Flow Provider

Start with a small function-token propagation pass:

```python
def js_function_token_flow(files):
    tokens = defaultdict(set)
    constraints = []

    for decl in function_decls(files):
        tokens[var(decl.name)].add(FunctionToken(decl.symbol))

    for assignment in assignments(files):
        constraints.append((expr_var(assignment.rhs), lvalue_var(assignment.lhs)))

    for export in exports(files):
        constraints.append((local_var(export.local), export_var(export.name)))

    for import_ in imports(files):
        constraints.append((module_export_var(import_), local_var(import_.binding)))

    solve_subset_constraints(tokens, constraints)

    for call in call_sites(files):
        for token in tokens[callee_var(call)]:
            emit_edge(call, token.symbol, algorithm="JsValueFlow", confidence="medium")
```

Do not model all JavaScript up front. Mark unknown features explicitly.

### Phase E: Repo-Local Call Models

Add a native model-loading layer before broad semantic algorithms become expensive.

Model capabilities:

- router and handler registration;
- decorators and annotations;
- dependency injection bindings;
- callback registries and event buses;
- generated clients and service stubs;
- MCP/tool registration;
- test, job, and framework lifecycle entrypoints.

```toml
[[facts.call_graph.models]]
path = ".polint/models/call_graph.toml"
required_validation = "warn" # off | warn | error
```

```python
for model in repo_call_models:
    bound = bind_model(model, symbols, call_sites, types)
    if not bound.ok:
        emit_model_diagnostic(model, bound.errors)
        continue

    for edge in bound.edges:
        emit(CallTargetFact(
            site_id=edge.site_id,
            caller=edge.caller,
            target=edge.target,
            algorithm="RepoModel",
            confidence=edge.confidence,
            provider="repo_model",
            model_id=model.id,
            provenance="RepoModel",
            validation=model.validation_status,
        ))
```

Deliverable: debug output comparing native default graph to extended repo-model graph.

### Phase F: Graph SDK

Expose typed query APIs:

```rust
impl<'a> CallGraph<'a> {
    pub fn call_sites(&self) -> impl Iterator<Item = CallSite<'a>>;
    pub fn outgoing(&self, caller: SymbolId) -> impl Iterator<Item = CallEdge<'a>>;
    pub fn incoming(&self, callee: SymbolId) -> impl Iterator<Item = CallEdge<'a>>;
    pub fn targets(&self, site: CallSiteId) -> impl Iterator<Item = CallTarget<'a>>;
    pub fn unresolved(&self) -> impl Iterator<Item = CallSite<'a>>;
}
```

Rule authors should be able to filter by confidence and algorithm:

```rust
for edge in call_graph.outgoing(function.id()) {
    if edge.confidence() >= Confidence::Medium && edge.target_name().ends_with(".Unsafe") {
        ctx.diagnostic(edge.span(), "avoid unsafe call");
    }
}
```

### Phase G: Future Language Providers

Java:
- bytecode/classpath provider;
- CHA first;
- RTA second;
- points-to optional.

Python:
- AST + symtable provider;
- PyCG-style name points-to;
- MRO/class construction/provider dispatch;
- explicit heuristic labeling.

## What To Avoid

- Do not expose full AST nodes through the public SDK.
- Do not make unresolved dynamic calls disappear.
- Do not merge repo-model edges into native facts without `model_id` and validation status.
- Do not put call graph methods directly on broad `RuleCtx`.
- Do not claim exact coverage for JS/TS or Python.
- Do not require heavy semantic setup for rules that only need call-site names.
- Do not make one language's lifecycle flags leak into another language.

## Recommended Next Implementation Task

Implement the internal direct-call bootstrap for the currently supported
languages:

1. Add `analysis::calls` IDs, facts, stable keys, store, and indexes.
2. Derive `CallSiteFact` from MIR call operations and `PlaceId`.
3. Add direct/binding `CallTargetFact` where symbols/references/imports resolve.
4. Emit unresolved facts with explicit reasons for everything else.
5. Add model/provenance fields now, even before repo-local model loading ships.
6. Add deterministic debug snapshots and evaluation fixtures.
7. Add cache key tests for call-site and direct-target layers.
8. Add extension sink validation tests.
9. Delay `Calls<'_>` / `CallGraph<'_>` SDK views until docs and temp-repo tests exist.
