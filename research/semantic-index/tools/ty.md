# Ty

## What It Is

Ty is Astral's modern Rust-native Python type checker. It is especially relevant to polint because it implements Python semantic indexing in Rust using tracked queries, typed arenas, scopes, symbols, places, and use-def maps.

Primary inspected files:

- `ty/ruff/crates/ty_python_core/src/lib.rs`
- `ty/ruff/crates/ty_python_core/src/symbol.rs`
- `ty/ruff/crates/ty_python_core/src/scope.rs`
- `ty/ruff/crates/ty_python_core/src/place.rs`
- `ty/ruff/crates/ty_python_core/src/use_def.rs`
- `ty/ruff/crates/ty_python_core/src/reachability_constraints.rs`
- `ty/ruff/crates/ty_python_semantic/src/reachability.rs`
- `ty/ruff/crates/ty_python_semantic/src/semantic_model.rs`

## Index Shape

Core objects:

- **SemanticIndex:** tracked per file and stores semantic facts for all scopes.
- **ScopeId / FileScopeId:** typed scope identities.
- **ScopeKind / ScopeVisibility / ScopeLaziness:** precise classification of Python scopes.
- **Symbol table:** compact symbols and flags: used, bound, declared, global, nonlocal, reassigned, parameter.
- **Place table:** tracks assignable/readable places per scope.
- **UseDefMap:** flow-sensitive use-definition information.
- **Reachability constraints:** record conditions under which bindings are reachable.

Ty separates semantic indexing from full type inference. That is an important design boundary: the semantic index records structure and use-def facts, while later semantic/type queries refine interpretation.

## Algorithm

```python
@tracked
def semantic_index(file):
    ast = parse(file)
    builder = SemanticIndexBuilder(file)

    for node in walk(ast):
        if opens_scope(node):
            builder.enter_scope(node)
        if declares_or_binds_symbol(node):
            builder.bind_symbol(node)
        if reads_symbol(node):
            builder.record_use(node)
        if writes_place(node):
            builder.record_place_write(node)
        if affects_reachability(node):
            builder.record_reachability_constraint(node)
        if closes_scope(node):
            builder.leave_scope()

    return builder.finish()

@tracked
def place_table(scope):
    return semantic_index(scope.file).place_table(scope)

@tracked
def use_def_map(scope):
    return semantic_index(scope.file).use_def_map(scope)
```

## Accuracy

Ty models Python features that generic symbol indexes usually miss:

- comprehension scopes;
- global/nonlocal declarations;
- class/function/module scope distinctions;
- flow-sensitive definitions;
- loop/header fixpoint concerns;
- implicit unbound states;
- star-import placeholders.

Hard cases remain:

- dynamic imports;
- runtime attributes;
- monkey patching;
- generated framework symbols;
- exact member resolution without type facts.

## Complexity

Semantic-index construction is linear in AST size plus use-def/place records:

```text
O(N + symbols + places + use_def_edges)
```

Flow/reachability can require fixpoint behavior around loops. Ty's comments around loop headers and widening are directly relevant: use-def for loops must avoid unbounded iteration while retaining enough precision for initializedness/type narrowing.

Tracked queries improve incremental behavior by splitting file index, scope place table, and use-def maps.

## Strengths

- Best Rust-native Python semantic-index reference.
- Strong separation of scopes, symbols, places, use-def, and type semantics.
- Tracked query design maps well to polint's future query engine.
- Explicit reachability/unbound handling.

## Weaknesses

- Project is newer than Pyright.
- Architecture is Python-specific and still evolving.
- Full ecosystem parity is not yet the same as mature products.

## Polint Implications

Copy:

- typed scope IDs;
- symbol flags;
- place/use-def fact separation;
- semantic index separate from type inference;
- tracked-per-scope invalidation idea;
- explicit loop/fixpoint handling for reachability.

Avoid:

- blocking all semantic indexing work on a full Python type checker;
- treating use-def as part of the generic symbol table.

Recommended Python semantic fact layers:

```text
Scopes
  -> Symbols
  -> Places
  -> UseDef
  -> Reachability
  -> Type-assisted reference resolution
```

Ty is the strongest source for how polint should eventually build Python semantics natively in Rust.
