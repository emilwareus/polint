# Python Report

## State Of The Art

Python static analysis is currently strongest when it combines:

- import/module resolution;
- binding/use-def graphs;
- declared and inferred type facts;
- flow-sensitive narrowing;
- literal/constant/nullness/truthiness facts;
- class/function/module object values;
- summaries for decorators, framework APIs, and standard-library behavior;
- explicit `Any`/unknown handling;
- optional taint/data-flow models.

Full heap-precise points-to for Python is not the default strategy in mainstream type checkers. The best tools get most of their precision from binding, type, narrowing, and summary infrastructure.

## Tools Studied

### Ty / Ruff

Ty is a Rust-native Python type checker from the creators of Ruff. The inspected source is currently inside the Ruff repository. It is the most relevant reference for polint because it shows how a modern Rust Python analyzer represents places, reachability, narrowing, and type relations without delegating to a Python runtime.

Important implementation ideas:

- place/use-def model for Python symbols and member-like places;
- reachability constraints represented as ternary formulas;
- narrowing constraints projected onto places;
- alias predicates for guard aliases;
- intersection/materialization-heavy type system;
- module resolver and semantic index separation.

Key inspected paths:

- `ruff/crates/ty_python_core/src/place.rs`
- `ruff/crates/ty_python_core/src/builder.rs`
- `ruff/crates/ty_python_core/src/predicate.rs`
- `ruff/crates/ty_python_core/src/reachability_constraints.rs`
- `ruff/crates/ty_python_semantic/src/reachability.rs`
- `ruff/crates/ty_python_semantic/src/types/infer.rs`
- `ruff/crates/ty_python_semantic/src/types/narrow.rs`
- `ruff/crates/ty_python_semantic/src/types/relation.rs`
- `ruff/crates/ty_module_resolver/src/resolve.rs`

Polint lesson: copy the shape of the fact layers, not the implementation. Python places and reachability/narrowing constraints should be first-class facts before points-to.

### Pyrefly

Pyrefly is also Rust-native. Its architecture document describes a module-centric checker:

```text
exports -> bindings -> solve bindings
```

It explicitly uses flow types, module-level incrementality, recursive `Var` placeholders, and binding keys that reference other keys.

Key inspected paths:

- `ARCHITECTURE.md` via `git show`
- `crates/pyrefly_graph/src/calculation.rs`
- `crates/pyrefly_types/src/types.rs`
- `crates/pyrefly_types/src/heap.rs`
- `crates/pyrefly_types/src/type_alias.rs`
- `crates/pyrefly_python/src/module.rs`
- `crates/pyrefly_build/src/source_db/*`

Polint lesson: a module-centric fixed point can be simpler than fine-grained query solving if module checks are fast. But polint's multi-language fact engine should still preserve provider cache keys and extension-aware invalidation.

### Pyright

Pyright is a mature TypeScript implementation of a Python type checker. Its narrowing engine and flow-node graph are highly relevant.

Key inspected paths:

- `packages/pyright-internal/src/analyzer/binder.ts`
- `packages/pyright-internal/src/analyzer/codeFlowTypes.ts`
- `packages/pyright-internal/src/analyzer/codeFlowEngine.ts`
- `packages/pyright-internal/src/analyzer/typeEvaluatorTypes.ts`
- `packages/pyright-internal/src/analyzer/checker.ts`

Polint lesson: Python type narrowing should be queryable at a reference/CFG location, not stored as one global type per symbol.

### Pyre / Pysa

Pyre and Pysa are important for interprocedural analysis and taint. Pysa's documentation explains model-based taint summaries and an interprocedural fixpoint. This is a model for polint's agent-authored summaries.

Key inspected areas:

- `source/analysis`
- `source/interprocedural`
- `source/taint`
- Pysa implementation docs.

Polint lesson: Python framework/security precision depends on summaries and models. polint should make those models native Rust extension facts.

### mypy

mypy remains a practical reference for Python narrowing and type binder behavior.

Key inspected paths:

- `mypy/binder.py`
- `mypy/checker.py`
- `mypy/reachability.py`
- `mypy/subtypes.py`

Polint lesson: even a mature checker keeps narrowing/binding as a distinct layer from general type inference.

### pytype

pytype uses a typegraph/VM-style analysis. Its docs explain CFG variables, bindings, origins, and solver visibility.

Key inspected paths:

- `docs/typegraph.md`
- `docs/main_loop.md`
- `docs/abstract_values.md`
- `pytype/typegraph`

Polint lesson: binding origins and visibility are critical for explaining why a value/type is possible.

## Python Accuracy Model

| Feature | Default polint target | Extension target |
|---|---|---|
| Imports/modules | Native module graph and source db. | Repo-specific import hooks/generated modules. |
| Locals/globals | Exact place facts. | Generated/builtins customization. |
| Type annotations | Parse and normalize common annotations. | Repo-specific type aliases/plugins. |
| Narrowing | `is None`, truthiness, `isinstance`, `issubclass`, `TypeGuard`, `TypeIs`, pattern match basics. | Project guard functions and framework validators. |
| Attributes | Known class/dataclass/typed dict/module attributes. | Dynamic attributes, decorators, ORM fields, Pydantic/Django models. |
| Function objects | Direct defs, lambdas, imports, simple assignments. | Registries, decorators, dependency injection. |
| Points-to | Allocation/function/class/module tokens, field-sensitive where known. | Framework object identity and injected singletons. |
| Dynamic behavior | Explicit unknowns. | Validated summaries/models. |

## Complexity And Risk

Python type analysis is expensive where:

- imports create large SCCs;
- decorators/metaclasses change APIs;
- protocols and structural types require shape checks;
- typed dicts and generics interact with unions;
- narrowing creates many branch-specific types;
- dynamic attributes defeat field precision.

Keep the first implementation bounded:

- local flow and narrowing first;
- summaries before global points-to;
- exact claims only for narrow modeled constructs;
- use `Unknown` for dynamic attribute/reflection behavior.

## Recommended Python Implementation Path

```text
1. Python module/package identity
2. scopes, symbols, imports, exports
3. places and access paths
4. declared/common annotation type facts
5. local CFG narrowing facts
6. function/class/module object value facts
7. class/typed-dict/dataclass-like field facts
8. summary facts for common standard-library/framework APIs
9. bounded points-to over function/object/module tokens
10. extension sinks for decorators, dynamic attributes, and framework registries
```

Do not attempt a heap-precise Python alias engine first. The maximum capability path is a strong type/value/model framework with optional points-to escalation.
