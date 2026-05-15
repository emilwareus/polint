# Research Analysis: Semantic Indexes

## Main Thesis

The research strongly rejects a single-language-neutral resolver as the core design. Mature systems split semantic indexing into:

1. language-specific binding and resolution;
2. stable semantic identities;
3. searchable occurrence/reference indexes;
4. derived relation layers;
5. export/storage schemas.

polint should copy that separation.

## Architecture Patterns

### Compiler Binder Pattern

Used by TypeScript, Pyright, JDT, Ty, and Pyrefly.

```python
def binder(file):
    scopes = ScopeArena()
    symbols = SymbolArena()
    for node in walk(file.ast):
        if creates_scope(node):
            scopes.enter(node)
        if declares_name(node):
            symbols.declare(scopes.current(), node)
        if uses_name(node):
            record_unresolved_use(scopes.current(), node)
        if exits_scope(node):
            scopes.exit()
    return scopes, symbols, unresolved_uses
```

Accuracy comes from language-specific scope rules. The binder must know hoisting, class bodies, type parameters, import forms, global/nonlocal markers, declaration merging, and package/classpath semantics.

### Query/Incremental Pattern

Used by rust-analyzer and Ty, partially by TypeScript/gopls/Pyright.

```python
@tracked
def semantic_index(file):
    ast = parse(file)
    return bind_file(ast)

@tracked
def package_resolution(package):
    return resolve_imports(package, semantic_index.each_file())
```

Accuracy does not automatically improve from incrementality, but incremental design forces clear dependency boundaries.

### Package/Metadata Pattern

Used by gopls, JDT, WALA, Soot/SootUp.

```python
def load_package_graph(workspace):
    roots = discover_roots(workspace)
    metadata = load_build_system_metadata(roots)
    packages = typecheck_selected_packages(metadata)
    return packages
```

This pattern is mandatory when names are resolved through package roots, classpaths, module paths, build tags, generated code, or external binary metadata.

### Relational/Fixpoint Pattern

Used by CodeQL and should be copied in small internal form.

```python
def derive_aliases(imports, exports, declarations):
    relation = seed(imports, exports, declarations)
    while relation.changed():
        relation.add(join_alias_edges(relation))
        relation.add(join_reexports(relation))
    return relation
```

This is best for recursive imports, exports, alias chains, module graphs, overrides, and later call/data-flow closures.

### Export/Graph Pattern

Used by SCIP, LSIF, and Kythe.

```python
def export_occurrences(index):
    for file in index.files:
        for ref in file.references:
            emit_occurrence(file.path, ref.range, ref.symbol_key, ref.role)
        for sym in file.local_symbols:
            emit_symbol(sym.key, sym.documentation, sym.relationships)
```

This is an output representation, not an ideal in-memory engine.

## Accuracy Comparison

| Tool | Accuracy Strength | Accuracy Weakness |
|---|---|---|
| CodeQL | Strong multi-language derived relations where extractors/libraries are mature. | Database/extractor lifecycle; precision varies by language and query library; not low-latency. |
| rust-analyzer | Precise Rust name resolution under macro-expanded HIR and incremental queries. | Rust macros/proc macros and cfgs complicate stability and completeness. |
| TypeScript | Compiler-owned binder/checker has best TS/JS semantics. | Dynamic JS, declarations from external packages, large unions/overloads, and project references drive complexity. |
| gopls | Type-checked package facts and serializable xrefs. | Build tags, module/workspace setup, generated code, and test variants affect completeness. |
| Pyright | Mature Python scopes/imports/type-aware resolution. | Dynamic imports, monkey patching, runtime attributes, notebooks, and untyped libraries. |
| Ty | Rust-native incremental semantic index and place/use-def design. | Young project; type checker and library modeling still evolving. |
| Pyrefly | Fast Rust-native module binding graph and flow/static scope model. | Young project; project ecosystem parity still evolving. |
| JDT | Mature Java source/binary binding and incremental compiler model. | Classpath/module complexity, unresolved errors, and generics increase complexity. |
| Soot/SootUp/WALA | Strong whole-program JVM abstractions. | Incomplete-world assumptions, reflection, native code, and framework semantics create unsoundness. |
| Semgrep | Very practical many-language pattern matching. | Generic naming cannot match compiler-grade semantic indexing. |
| SCIP/Kythe | Strong stable export identities and occurrence relationships. | Do not compute semantics themselves. |
| LSIF | Useful LSP-result exchange history. | Not a symbol database; graph IDs and result-shape storage are awkward. |

## Complexity Analysis

### Binder/Scope Construction

Expected cost is linear in AST size:

```text
O(N + D + R)
```

Memory is proportional to scopes, declarations, and reference occurrences. The risk is not asymptotic complexity; it is semantic special cases.

### Import/Export/Alias Resolution

Expected cost is a bounded fixpoint:

```text
O(K * (I + A + E))
```

`K` is iteration count. In normal code it is small; cycles and star exports/imports need bounds and explicit incomplete facts.

### Type-Assisted Resolution

There is no single complexity bound across languages. Examples:

- TS overload resolution and union narrowing can be expensive.
- Python type narrowing may depend on flow and imported stubs.
- Java generic inference and overload resolution can be superlinear.
- Go package type checking is relatively predictable but build tags and module variants multiply packages.

The implementation consequence is to store `TypeAssisted` resolution separately from binder-exact resolution and make it provider-demanded.

### Global References

The best pattern is:

```text
name/symbol prefilter -> semantic verification -> sorted result
```

This keeps most queries near:

```text
O(candidate occurrences + verification cost)
```

instead of scanning every file.

## Product-Specific Insight

Classic static analyzers try to maximize generic inference. polint should maximize **validated extensibility**.

That changes semantic indexing:

- generated symbols are not edge cases; they are first-class extension facts;
- unresolved references are not failures; they are prompts for agent model work;
- ambiguous facts are not hidden; they are measurable precision gaps;
- a repo-local provider can be more accurate than a generic framework recognizer;
- the engine must make extension effects visible through default-vs-extended deltas.

## Recommended Internal Invariants

1. Every `ReferenceFact` has an enclosing scope.
2. Every exact reference has at least one `ResolutionFact` explaining the decision.
3. Every symbol has a stable key, even if it is only file-local.
4. Every generated symbol has a generator provenance.
5. Every extension-provided fact is either unvalidated or validated; never silently native.
6. Every import resolution has a status, including external/missing/dynamic.
7. Exactness is a precision label, not implied by fact presence.
8. Public SDK methods should expose uncertainty without forcing users into internal metadata tables.

## What To Research Next

Semantic indexing cannot be finished without module/package topology. The next research topic should be `research/module-graph/`, because import resolution, stable keys, external symbols, generated-code zones, package roots, and workspace boundaries all depend on it.
