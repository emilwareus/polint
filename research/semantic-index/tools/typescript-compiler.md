# TypeScript Compiler

## What It Is

The TypeScript compiler is the authoritative semantic index for TS/JS. It builds ASTs, binds declarations to symbols, resolves names and modules in the checker, and provides language-service APIs for references, definitions, diagnostics, and refactors.

Primary inspected files:

- `src/compiler/binder.ts`
- `src/compiler/types.ts`
- `src/compiler/checker.ts`
- `src/services/findAllReferences.ts`
- `src/compiler/builder.ts`
- `src/compiler/builderState.ts`

## Index Shape

Core objects:

- **Node:** AST node, often annotated with symbol/locals.
- **Symbol:** semantic entity with flags, declarations, exports, members.
- **SymbolTable:** map from escaped names to symbols.
- **Type:** checker-level type object.
- **Program:** source files, compiler options, module resolution, type checker.
- **BuilderState:** incremental state for file versions, signatures, affected files.

The binder populates symbol tables and connects declarations to symbols. The checker resolves names, types, modules, exports, members, overloads, and control-flow-dependent types.

## Algorithm

```python
def bind_source_file(file):
    container = file
    block_scope = file
    for node in walk(file.ast):
        if creates_container(node):
            container = node
        if creates_block_scope(node):
            block_scope = node
        if declares_symbol(node):
            table = choose_symbol_table(container, block_scope, node)
            declare_symbol(table, node.name, node.flags, node)
        if creates_exports_or_members(node):
            initialize_exports_or_members(node.symbol)
    return file.locals

def resolve_name(location, name, meaning):
    scope = enclosing_scope(location)
    while scope:
        sym = lookup(scope.locals_or_members, name, meaning)
        if sym:
            return checker_filter_and_merge(sym, meaning)
        scope = scope.parent
    return lookup_globals_or_external_modules(name)
```

## Accuracy

TypeScript is accurate for TS/JS because the semantic index is the compiler. It handles:

- declaration merging;
- namespaces;
- ambient declarations;
- type/value namespace separation;
- ES imports/exports;
- CommonJS patterns where supported;
- type-only imports;
- overloads;
- contextual typing and narrowing.

Weak cases:

- dynamic property access;
- runtime mutation of exports;
- imprecise JS without types;
- `any`;
- generated declaration files that do not match runtime;
- large unions/overloads can become expensive.

## Complexity

Binding is mostly linear in AST size:

```text
O(N + D)
```

Checking and resolution are language-specific and can become superlinear due to generics, overloads, union/intersection operations, conditional types, and project references.

Incremental builder state separates:

- source file version/content;
- public signature shape;
- semantic diagnostics cache;
- affected files.

The key lesson is that not all changes are equal. A private implementation edit should not invalidate all dependents if the exported signature is unchanged.

## Strengths

- Best practical TS/JS semantic source.
- Mature binder/checker split.
- Deep handling of declaration merging and namespaces.
- Incremental builder model is directly relevant to cache keys.
- Language service references show candidate search plus semantic verification.

## Weaknesses

- AST mutation/annotation style is not ideal for polint's immutable fact layers.
- TS-specific global `Program` shape does not generalize cleanly.
- JavaScript dynamic behavior remains inherently hard.

## Polint Implications

Copy:

- binder before checker;
- symbol tables per scope/container;
- separate type/value namespaces;
- declaration merging as an explicit fact;
- public signature digest for cache invalidation;
- reference search using name candidates then semantic validation.

Avoid:

- exposing mutable AST nodes to rules;
- assuming TS/JS import/export semantics fit Go/Python/Java;
- claiming exactness for dynamic CommonJS/runtime mutation.

Recommended polint TS/JS ladder:

```text
syntax imports/exports
  -> binder scopes/symbols
  -> static module resolution
  -> alias/reexport closure
  -> checker/type-assisted resolution where available
  -> dynamic/unsupported facts for runtime patterns
```
