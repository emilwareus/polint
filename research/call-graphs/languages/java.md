# Java And JVM Call Graphs

## Best OSS References

- `repos/sootup`: clean modern API for CHA/RTA.
- `repos/soot`: classic Soot and Spark points-to.
- `repos/wala`: mature SSA, CFA, and propagation builders.
- `repos/doop`: Datalog/Souffle points-to and context sensitivity.
- `repos/opal`: modular call graph architecture and TypeIterator design.
- `repos/tai-e`: modern Java framework with CHA and PTA-based call graphs.
- `repos/codeql/java/ql/lib`: production query-facing Java dispatch.

## Java Setup Matters Most

Before algorithm choice, a Java call graph needs:

- application classes or jars;
- dependency jars;
- JDK classes or modules;
- entrypoints;
- application/library/JDK scope split;
- generated code and annotation processor handling;
- treatment of reflection, native code, lambdas, invokedynamic, and framework callbacks.

Missing setup should become capability diagnostics, not an empty graph.

## CHA

SootUp source: `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/ClassHierarchyAnalysisAlgorithm.java`

Tai-e source: `repos/tai-e/src/main/java/pascal/taie/analysis/graph/callgraph/CHABuilder.java`

```python
def java_cha(entrypoint, hierarchy):
    graph = Graph()
    work = [entrypoint]

    while work:
        method = work.pop()
        if graph.mark_reachable(method):
            for call in method.call_sites:
                for target in java_cha_targets(call, hierarchy):
                    if graph.add_edge(method, call, target, algorithm="cha"):
                        work.append(target)

    return graph

def java_cha_targets(call, hierarchy):
    if call.kind in ["static", "special"]:
        return {resolve_declared(call.method_ref)}

    receiver_type = declared_or_base_type(call)
    targets = set()
    for cls in hierarchy.all_subclasses(receiver_type):
        if cls.is_abstract:
            continue
        target = hierarchy.dispatch(cls, call.method_ref)
        if target:
            targets.add(target)
    return targets
```

CHA is deterministic and easy to explain. It over-approximates every subclass.

## RTA

SootUp source: `repos/sootup/sootup.callgraph/src/main/java/sootup/callgraph/RapidTypeAnalysisAlgorithm.java`

```python
def java_rta(entrypoints, hierarchy):
    graph = Graph()
    instantiated = set()
    ignored_calls_by_class = defaultdict(list)
    work = list(entrypoints)

    while work:
        method = work.pop()
        if not graph.mark_reachable(method):
            continue

        for cls in allocations_in(method):
            if cls not in instantiated:
                instantiated.add(cls)
                for call in ignored_calls_by_class[cls]:
                    work.extend(add_targets_for(call, graph, hierarchy, instantiated))

        for call in method.call_sites:
            if call.kind in ["static", "special"]:
                targets = {resolve_declared(call.method_ref)}
            else:
                targets = {
                    target for target in java_cha_targets(call, hierarchy)
                    if target.owner_class in instantiated
                }
                remember_uninstantiated_targets(call, ignored_calls_by_class, instantiated)

            for target in targets:
                if graph.add_edge(method, call, target, algorithm="rta"):
                    work.append(target)

    return graph
```

RTA is often a strong second tier, but assumes the entry/lifecycle model is good.

## Points-To With On-The-Fly Call Graph

Soot source:

- `repos/soot/src/main/java/soot/jimple/spark/SparkTransformer.java`
- `repos/soot/src/main/java/soot/jimple/spark/solver/OnFlyCallGraph.java`
- `repos/soot/src/main/java/soot/jimple/toolkits/callgraph/CallGraphBuilder.java`

Tai-e source:

- `repos/tai-e/src/main/java/pascal/taie/analysis/pta/core/solver/DefaultSolver.java`

```python
def pta_call_graph(entrypoints):
    pts = PointsToSets()
    pfg = PointerFlowGraph()
    call_graph = ContextSensitiveCallGraph()
    work = initialize(entrypoints)

    while work:
        event = work.pop()

        if event.new_reachable_method:
            constraints = extract_pointer_constraints(event.method)
            pfg.add(constraints)

        if event.points_to_changed:
            propagate_pointer_flow(event.pointer, event.diff, pfg, pts, work)

            for call in invokes_on_receiver(event.pointer.var):
                for obj in event.diff:
                    target = dispatch(obj.type, call.method_ref)
                    if target:
                        edge = make_edge(call, target, context_for(obj, call))
                        if call_graph.add_edge(edge):
                            work.append(new_reachable_method(target))
                            bind_this_and_args(edge, pts, pfg, work)

    return call_graph, pts
```

This is the research-grade tier. It is powerful, but it needs substantial engineering.

## WALA

Sources:

- `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/cha/CHACallGraph.java`
- `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/propagation/SSAPropagationCallGraphBuilder.java`
- `repos/wala/core/src/main/java/com/ibm/wala/ipa/callgraph/propagation/cfa/ZeroXCFABuilder.java`

WALA's important design choice is `CGNode = method + context`.

```python
def wala_style(program, context_policy):
    for entry in entrypoints:
        add_node(method=entry, context=root_context())

    while solver_changes:
        for cg_node in new_or_changed_nodes:
            ir = ssa_ir(cg_node.method)
            add_constraints_from_ir(ir, cg_node.context)

        solve_points_to_constraints()

        for call in changed_calls:
            for target in target_selector(call, points_to, class_hierarchy):
                ctx = context_selector(cg_node.context, call, target)
                add_cg_node(target, ctx)
                add_call_edge(cg_node, call, target, ctx)
```

Good for understanding context-sensitive architecture.

## Doop

Sources:

- `repos/doop/souffle-logic/main/full-call-graph.dl`
- `repos/doop/souffle-logic/main/export.dl`
- `repos/doop/souffle-logic/main/reflection/rules.dl`
- `repos/doop/souffle-logic/main/method-handles.dl`

Doop expresses call graph construction as recursive relations:

```python
def datalog_style():
    # CallGraphEdge(ctx_from, invocation, ctx_to, callee)
    # VarPointsTo(ctx, var, heap_ctx, object)
    # Reachable(ctx, method)

    CallGraphEdge += static_edges()

    for invocation in virtual_invocations:
        for obj in VarPointsTo[caller_ctx, invocation.base]:
            callee = dispatch(obj.type, invocation.signature)
            callee_ctx = context(invocation, caller_ctx, obj)
            CallGraphEdge.add(caller_ctx, invocation, callee_ctx, callee)
            Reachable.add(callee_ctx, callee)

    add_reflection_edges()
    add_method_handle_edges()
    add_lambda_edges()
```

Doop is a great specification model and experiment platform, but too heavy to embed in polint first.

## OPAL

Sources:

- `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/TypeIterator.scala`
- `repos/opal/OPAL/tac/src/main/scala/org/opalj/tac/fpcf/analyses/cg/CallGraphAnalysis.scala`

OPAL's key design: separate "which receiver types are possible?" from "how does this call dispatch for a receiver type?"

```python
class TypeIterator:
    def possible_types(self, receiver_use, context):
        ...

class CHATypeIterator(TypeIterator): ...
class RTATypeIterator(TypeIterator): ...
class PointsToTypeIterator(TypeIterator): ...

def resolve_virtual_call(call, type_iterator):
    for typ in type_iterator.possible_types(call.receiver, call.context):
        target = project.instance_call(typ, call.name, call.descriptor)
        emit_edge(call, target, algorithm=type_iterator.name)
```

This maps extremely well to polint's desired pluggable analysis design.

## CodeQL Java

CodeQL exposes:

- `Callable`
- `Call`
- `Callable.calls()`
- `Callable.polyCalls()`
- virtual dispatch predicates under `dispatch/VirtualDispatch.qll`

The model is query-oriented, not a reusable algorithm package. The useful lesson is the API shape and the distinction between static calls and virtual dispatch.

## Polint Recommendation For Java

Java is future work for polint, but the staged design should be:

1. Use bytecode/classpath setup, not source-only Java parsing, for real call graph resolution.
2. Start with call site extraction and `STATIC` / `SPECIAL` edges.
3. Add CHA.
4. Add RTA once entrypoints and allocations are modeled.
5. Add optional points-to provider after the base Java lifecycle is robust.
6. Keep reflection, invokedynamic, native calls, missing classes, and framework callbacks explicit as unresolved/unsupported/setup diagnostics.

