# Pyrefly

## What It Is

Pyrefly is Facebook/Meta's Rust-based Python type checker. It is relevant because it takes a fast, module-centric approach to binding and graph calculation, rather than a fine-grained Salsa-first architecture.

Primary inspected files:

- `ARCHITECTURE.md`
- `pyrefly/lib/binding/scope.rs`
- `pyrefly/lib/binding/binding.rs`
- `pyrefly/lib/binding/bindings.rs`
- `pyrefly/lib/binding/table.rs`
- `crates/pyrefly_graph/src/index.rs`
- `crates/pyrefly_graph/src/calculation.rs`

## Index Shape

Core objects:

- **Module exports:** solved first, including transitive `import *`.
- **Bindings:** per-module conversion from syntax to binding keys and binding values.
- **Static scopes and flow scopes:** separate lookup modes for Python's binding behavior.
- **Binding table:** typed macro-generated table of key/value families.
- **Graph index:** compact interned `Idx<K>` values.
- **Calculation cache:** recursive computation cache with cycle/SCC handling.

Pyrefly architecture explicitly expects dependency cycles and large SCCs. It optimizes for parallel module-level throughput rather than maximal fine-grained invalidation.

## Algorithm

```python
def pyrefly_pipeline(project):
    exports = solve_module_exports(project.modules)
    bindings = parallel_map(project.modules, lambda m: bind_module(m, exports))
    solved = solve_bindings_with_graph(bindings)
    return solved

def bind_module(module, exports):
    table = BindingTable()
    scope = StaticScope(module)
    flow = FlowScope(module)

    for stmt in module.ast:
        bind_statement(stmt, scope, flow, table)

    return table

def solve_bindings_with_graph(bindings):
    graph = CalculationGraph()
    for key in bindings.keys():
        graph.compute(key)
    graph.commit_scc_results()
    return graph.results
```

## Accuracy

Pyrefly's binding model is valuable because it makes Python lookup states explicit:

- `NameReadInfo::Flow`
- `NameReadInfo::Anywhere`
- `NameReadInfo::NotFound`
- initialized-in-flow states such as yes, conditional, no, deferred check
- static styles such as anywhere, mutable capture, single def, implicit global, delete, mergeable import

This explicit state is exactly what polint needs for honest precision reporting.

Hard cases are the same Python dynamic cases:

- runtime import tricks;
- monkey patching;
- dynamic attributes;
- framework-generated names.

## Complexity

Pyrefly's module binding is roughly linear in module AST size. Export solving and binding solving depend on the module import graph and SCC sizes:

```text
O(module_ast_size) + O(export_graph_sccs) + O(binding_dependency_edges)
```

The calculation graph is designed to detect cycles and batch commit SCC results. This is a good architecture for large monorepos where fine-grained invalidation may be less important than predictable parallel throughput.

## Strengths

- Rust-native.
- Fast module-centered architecture.
- Explicit static/flow scope split.
- Typed key/value binding tables.
- SCC-aware recursive calculation cache.

## Weaknesses

- Newer ecosystem maturity.
- Less fine-grained incremental than Salsa-style designs.
- Python-specific semantics.

## Polint Implications

Copy:

- typed binding tables;
- compact interned indexes;
- module export solving before per-reference final resolution;
- SCC-aware calculation for import/export cycles;
- explicit flow/static lookup states.

Avoid:

- assuming import graphs are acyclic;
- mixing static and flow-sensitive facts in one table.

Design lesson:

```text
Ty gives polint the fine-grained query reference.
Pyrefly gives polint the high-throughput module/SCC reference.
polint should start module-batched, but keep fact dependencies explicit enough to refine later.
```
