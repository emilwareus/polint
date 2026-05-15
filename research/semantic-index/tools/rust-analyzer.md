# rust-analyzer

## What It Is

rust-analyzer is the strongest reference for a native Rust incremental semantic architecture. It builds a compiler-like semantic model around syntax trees, HIR, DefMaps, item scopes, expression scopes, and Salsa queries.

Primary inspected files:

- `crates/hir-def/src/nameres.rs`
- `crates/hir-def/src/nameres/collector.rs`
- `crates/hir-def/src/item_scope.rs`
- `crates/hir-def/src/expr_store/scope.rs`
- `crates/hir/src/semantics.rs`
- `crates/ide-db/src/symbol_index.rs`

## Index Shape

Important objects:

- **DefMap:** module tree plus visible items for a crate.
- **LocalDefMap:** crate-local split to reduce invalidation.
- **ItemScope:** visible items per namespace, imports, unresolved imports, declarations, impls, macro scopes.
- **ExprScopes:** lexical scopes and entries for local bindings inside bodies.
- **Semantics:** source-to-HIR facade used by IDE features.
- **Symbol index:** searchable per-root/file symbol index, using compact search structures.

`DefCollector::collect` performs a fixed-point collection loop over imports and macros. The inspected code includes recursion/fixed-point bounds such as `GLOB_RECURSION_LIMIT` and `FIXED_POINT_LIMIT`.

## Algorithm

```python
def build_rust_def_map(crate):
    raw_items = collect_raw_items(crate.files)
    def_map = seed_modules(raw_items)

    for _ in range(FIXED_POINT_LIMIT):
        changed = False
        changed |= resolve_imports(def_map)
        changed |= resolve_macros_and_expand_items(def_map)
        changed |= collect_new_items_from_expansions(def_map)
        if not changed:
            break

    if changed:
        emit_non_convergence_diagnostic()
    return def_map

def build_expr_scopes(body):
    scopes = Arena()
    root = scopes.alloc(parent=None)
    walk_body(body, root, scopes)
    return scopes
```

## Accuracy

rust-analyzer is accurate because it follows Rust's compiler semantics closely and separates:

- module/item name resolution;
- macro expansion;
- local expression scopes;
- HIR identity;
- source mapping.

Main hard cases:

- proc macros;
- conditional compilation;
- macro-generated items;
- incomplete code while editing;
- incremental stability across generated syntax changes.

## Complexity

Parsing and raw item collection are linear in source size. Name resolution is a bounded fixed point over imports/macros/items:

```text
O(iterations * (imports + macro_invocations + item_edges))
```

Expression scope construction is linear in body AST/HIR size. Symbol search index construction is roughly:

```text
O(symbols * key_cost + index_build_cost)
```

The more important cost lesson is invalidation: splitting syntax, DefMap, local DefMap, expression scopes, and symbol indexes avoids rebuilding the world after a small edit.

## Strengths

- Best Rust-native architecture reference.
- Strong typed IDs and arenas.
- Clean separation between syntax and semantic identity.
- Incrementality is designed into every layer.
- Fixed-point import/macro resolution has explicit bounds.
- Semantic facade protects callers from raw internals.

## Weaknesses

- Highly Rust-specific semantics.
- Salsa is deeply integrated; copying it prematurely would constrain polint.
- Macro/cfg behavior is not directly transferable to Go/TS/Python/Java.

## Polint Implications

Copy:

- typed ID arenas;
- source-to-semantic facade;
- DefMap-like module/scope graph;
- expression/local scope arenas;
- bounded fixed points;
- searchable symbol indexes;
- stable separation of internal facts from public SDK.

Avoid:

- adopting Salsa as a public dependency now;
- overfitting all languages to Rust module/macro semantics.

Recommended polint translation:

```text
SourceMap
  -> ItemTree / Declarations
  -> ScopeMap
  -> Import/Alias fixpoint
  -> Expr/Local scopes
  -> Semantic facade
  -> Xref index
```

This is the closest architectural blueprint for polint's native Rust implementation.
