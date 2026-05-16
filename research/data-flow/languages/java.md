# Java / JVM Data-Flow Notes

## Summary

Java has the strongest ecosystem for precise static data-flow analysis. The state of the art combines bytecode/IR normalization, class hierarchy, call graph, points-to analysis, IFDS/IDE, access paths, and framework models.

If polint must stay fully native Rust, Java should come after the shared data-flow engine is proven. A serious native Java provider needs classpath handling, class hierarchy, bytecode/source normalization, exception edges, method resolution, virtual dispatch, lambdas, reflection markers, and framework entrypoints.

## OSS References

- FlowDroid: `repos/FlowDroid/soot-infoflow/src/soot/jimple/infoflow/*`.
- Heros: `repos/heros/src/heros/*`.
- WALA: `repos/WALA/core/src/main/java/com/ibm/wala/dataflow/IFDS/*`.
- Checker Framework: `repos/checker-framework/dataflow/src/main/java/org/checkerframework/dataflow/*`.
- Doop: `repos/doop/souffle-logic/*`.
- OpenTaint: `repos/opentaint/core/*`, `repos/opentaint/rules/*`.
- CodeQL Java: `repos/codeql/java/ql/lib/*/dataflow/*`.

## Required Facts

```python
JavaFacts(
    packages,
    classes,
    interfaces,
    records,
    enums,
    methods,
    fields,
    annotations,
    classpath,
    hierarchy,
    overrides,
    overloads,
    bytecode_or_source_cfg,
    exception_edges,
    allocations,
    loads,
    stores,
    arrays,
    lambdas,
    method_refs,
    invokedynamic,
    reflection_markers,
    native_methods,
    framework_entrypoints,
)
```

## Local Flow

Java local flow should use a normalized IR, not raw source AST.

```python
for instr in method.ir:
    match instr:
        case LoadLocal(x):
            use(place(x))

        case StoreLocal(x, value):
            edge(value, place(x), "assignment")

        case GetField(base, field):
            edge(place(base, field), result(instr), "field_read")

        case PutField(base, field, value):
            edge(value, place(base, field), "field_write")

        case Invoke(target, args):
            bind_call_args(instr, target, args)

        case Throw(value):
            edge(value, exceptional_exit(method), "throw")
```

## IFDS Shape

Java is the natural fit for IFDS/IDE.

```python
class JavaTaintProblem:
    def normal_flow(stmt, fact):
        return transfer_stmt(stmt, fact)

    def call_flow(call, callee_entry, fact):
        return map_actual_to_formal(call, callee_entry, fact)

    def return_flow(call, callee_exit, return_site, fact):
        return map_return_to_call_result(call, fact)

    def call_to_return_flow(call, return_site, fact):
        return model_unanalyzed_call_or_identity(call, fact)
```

## Access Paths

FlowDroid-style access paths are essential:

```python
AccessPath(
    base = local_or_param,
    fields = [field1, field2, ...],
    taint_sub_fields = True,
    max_depth = 5,
)
```

Use reductions:

```python
if len(path.fields) > max_depth:
    path = path.prefix(max_depth - 1) + [WILDCARD]
```

## Call Graph And Points-To

Build in stages:

```python
direct_edges = invokestatic + invokespecial + constructors
cha_edges = virtual_dispatch(class_hierarchy)
rta_edges = virtual_dispatch(allocated_types)
points_to_edges = virtual_dispatch(points_to(receiver))
```

Data-flow precision depends on the selected call graph:

- CHA: broad but easy.
- RTA: better if allocation roots are known.
- points-to: strongest but more costly.

## Dynamic And Framework Features

Emit explicit facts for:

```python
reflection
MethodHandle
invokedynamic
lambdas
ServiceLoader
Spring bean wiring
dependency injection
JPA persistence
native methods
serialization
```

Framework models should be versioned, documented, and optional.

## Recommended Java Milestones

1. Classpath and declaration index.
2. Class hierarchy and method resolution.
3. Source/bytecode normalized CFG.
4. Local data flow.
5. Direct and CHA call graph.
6. Function summaries.
7. Access paths and field sensitivity.
8. RTA or allocation-site points-to.
9. IFDS/IDE engine.
10. Spring/Jakarta/JPA framework models.

## Polint Decision

Do not implement Java first unless Java is the immediate product priority. Java is the best validation target for advanced algorithms, but a fully native Java provider is a large project.

