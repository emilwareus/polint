# Pyright

## What It Is

Pyright is the mature Python type checker and language server reference. Its semantic index is binder-driven, scope-based, symbol-table-backed, and integrated with import resolution and type evaluation.

Primary inspected files:

- `packages/pyright-internal/src/analyzer/scope.ts`
- `packages/pyright-internal/src/analyzer/symbol.ts`
- `packages/pyright-internal/src/analyzer/binder.ts`
- `packages/pyright-internal/src/languageService/referencesProvider.ts`
- `packages/pyright-internal/src/analyzer/sourceFile.ts`
- `packages/pyright-internal/src/analyzer/program.ts`
- `packages/pyright-internal/src/analyzer/service.ts`

## Index Shape

Core objects:

- **Scope:** TypeParameter, Comprehension, Function, Class, Module, Builtin.
- **Symbol:** declarations plus flags such as hidden, class/instance member, private, dunder-all/import status.
- **Declaration:** source declaration or inferred/generated declaration.
- **Program:** source files, import graph, dirty state, analysis lifecycle.
- **ReferencesProvider:** candidate file/name scan plus semantic verification.

Pyright's scope lookup handles:

- parent scopes;
- type parameter proxy scopes;
- chained module scopes;
- global/nonlocal declarations;
- externally hidden declarations;
- class/member visibility.

## Algorithm

```python
def bind_python_file(ast):
    module_scope = Scope(kind="Module")
    for node in walk(ast):
        if node.kind in ["function", "class", "comprehension", "type_parameter"]:
            push_scope(node.kind)
        if node.declares_name():
            current_scope.symbols.add_or_update(node.name, declaration(node))
        if node.uses_name():
            record_name_use(current_scope, node)
        if node.is_global_or_nonlocal():
            mark_scope_binding(node.name, node.kind)
        if node.exits_scope():
            pop_scope()
    return module_scope

def lookup_symbol_recursive(scope, name):
    if scope.proxy:
        maybe = lookup_symbol_recursive(scope.proxy, name)
        if maybe:
            return maybe
    if name in scope.symbols and visible(scope.symbols[name]):
        return scope.symbols[name]
    if name_is_global_or_nonlocal(scope, name):
        return lookup_declared_outer_scope(scope, name)
    if scope.chained_module:
        maybe = lookup_symbol_recursive(scope.chained_module, name)
        if maybe:
            return maybe
    return lookup_symbol_recursive(scope.parent, name)
```

## Accuracy

Pyright is strong for:

- lexical scopes;
- class/function/module semantics;
- imports;
- type narrowing;
- declarations and type stubs;
- member references where types are known;
- global/nonlocal behavior.

Hard cases:

- monkey patching;
- dynamic imports;
- runtime `setattr`/`getattr`;
- conditional imports depending on runtime environment;
- notebooks/chained modules;
- untyped libraries;
- generated attributes.

## Complexity

Binding is near linear in AST size. Import resolution and type evaluation depend on project import graph and stubs. References use a practical two-phase strategy:

```text
candidate files by symbol text/name -> semantic verification
```

This avoids verifying every token in every file for every query.

## Strengths

- Mature Python-specific scope and symbol model.
- Good explicit symbol flags.
- Useful references provider design.
- Dirty-state/source-file lifecycle is relevant to cache invalidation.

## Weaknesses

- TypeScript implementation style does not translate directly to Rust.
- Python dynamic behavior prevents complete static exactness.
- Some high-quality behavior in the Python ecosystem may exist in Pylance but not open source.

## Polint Implications

Copy:

- scope kinds;
- symbol flags;
- global/nonlocal modeling;
- candidate-name prefilter plus semantic verification;
- explicit dynamic/unknown results.

Avoid:

- pretending Python member references are exact without type evidence;
- treating runtime attributes as normal static declarations.

Recommended Python path:

```text
parse
  -> binder scopes/symbols
  -> imports/stubs/package graph
  -> flow-sensitive initializedness/use-def
  -> type-assisted member resolution
  -> extension facts for generated/dynamic attributes
```
