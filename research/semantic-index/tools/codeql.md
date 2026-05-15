# CodeQL

## What It Is

CodeQL builds a queryable database of source-code facts, then evaluates QL libraries and queries over those relations. For semantic indexing, CodeQL is the best reference for a **multi-language relational fact layer** and recursive derived predicates.

Primary inspected files:

- `go/ql/lib/semmle/go/Scopes.qll`
- `python/ql/lib/semmle/python/Scope.qll`
- `python/ql/lib/semmle/python/Variables.qll`
- `javascript/ql/lib/semmle/javascript/Variables.qll`
- `javascript/ql/lib/semmle/javascript/internal/NameResolution.qll`

## Index Shape

CodeQL semantic facts are split into:

1. extractor-produced base relations, such as nodes, scopes, objects, references, bindings, locations;
2. language QL libraries that wrap base relations in classes/predicates;
3. derived relations computed by QL recursion and joins;
4. user/security queries over those semantic abstractions.

Examples from the inspected libraries:

- Go `Scope extends @scope`, with scope nesting and `Entity extends @object`.
- Python `Scope` models modules, classes, functions, lambdas, and comprehensions.
- Python `Variable extends @py_variable` links variable loads/stores/scopes and escape behavior.
- JavaScript variables and scopes model global/module/function/catch/block/for/comprehension/namespace behavior.
- JS `NameResolution.qll` models module/value flow and external module normalization.

## Algorithm

```python
def codeql_semantic_index(source):
    trap = extractor_emit_base_relations(source)
    db = import_trap_relations(trap)

    scopes = ql_class("@scope").wrap(db.scope_rows)
    objects = ql_class("@object").wrap(db.object_rows)
    refs = derive_references(db, scopes, objects)

    # QL predicates behave like derived relations.
    while recursive_predicates_change():
        derive_scope_nesting()
        derive_import_aliases()
        derive_module_exports()
        derive_value_flow_edges()

    return relational_database(scopes, objects, refs)
```

## Accuracy

CodeQL can be very precise where:

- the extractor has complete source/build information;
- language libraries model the feature;
- queries use precise predicates rather than broad approximations.

The precision is not uniform across languages. A QL class may expose a useful abstraction while still hiding extractor/language incompleteness. For example, JS name resolution comments and structure show substantial handling for module normalization and value flow, but JS dynamic behavior and TypeScript namespace/module interactions remain difficult.

## Complexity

Base extraction is roughly linear in source/build artifacts plus compiler/build overhead. Query evaluation cost depends on relation sizes, joins, recursion, and indexing. Recursive derived predicates are least-fixpoint computations; a simplified bound is:

```text
O(fixpoint_iterations * relation_join_cost)
```

The FSE incremental CodeQL paper matters because it shows the production challenge: precise relational analyses can be expensive to incrementally maintain, especially when interprocedural/context/field-sensitive derived facts depend on many inputs.

## Strengths

- Excellent language-neutral query abstraction.
- Strong provenance through extractor relations and library predicates.
- Recursion/fixpoints are natural.
- Very good for derived facts such as alias closure, flow closure, and reachability.
- Mature multi-language design.

## Weaknesses

- Database extraction lifecycle is heavy compared to IDE-like semantic indexes.
- Query performance is not naturally tied to low-latency per-edit feedback.
- Fact exactness is query/library-specific and must be documented.
- Externalizing every fact into relations can make small implementation experiments slower than typed in-memory Rust providers.

## Polint Implications

Copy:

- relation/fixpoint thinking for aliases, imports, reexports, module graph, call graph, and data-flow closures;
- explicit derived fact layers;
- query-facing stable abstractions rather than raw AST nodes.

Avoid:

- making CodeQL/QL a runtime dependency;
- storing all internal facts in a generic relation DB first;
- exposing public facts without exactness and provenance.

Recommended use in polint:

```text
typed Rust fact arenas
  + small internal relation/fixpoint helper
  + provenance side tables
  + typed SDK views
```

CodeQL is the conceptual reference for derived relations, not the internal architecture to clone wholesale.
