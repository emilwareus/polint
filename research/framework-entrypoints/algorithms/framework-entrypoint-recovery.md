# Algorithm: Framework Entrypoint Recovery

This file gives language-neutral pseudo-code for the framework boundary layer.

## Pipeline

```python
def recover_framework_boundaries(project, requested_families):
    manifests = load_manifests(project)
    syntax = require_layer("syntax")
    imports = require_layer("imports")
    symbols = require_layer("symbols")
    references = require_layer("references")

    frameworks = detect_frameworks(manifests, imports, symbols)
    components = discover_components(project, frameworks, syntax, symbols, references)
    registrations = discover_registrations(project, frameworks, components, references)
    lifecycle = discover_lifecycle(project, frameworks, components, registrations)

    graph = compose_framework_graph(components, registrations, lifecycle)
    graph = resolve_targets(graph, symbols, references)
    graph = attach_trust_boundaries(graph, frameworks, symbols, references)

    facts = emit_facts(graph)
    return validate_merge_cache(facts)
```

## Framework Detection

```python
def detect_frameworks(manifests, imports, symbols):
    detected = []

    for dep in manifests.dependencies:
        if dep.name in FRAMEWORK_PACKAGE_TABLE:
            detected.append(framework(dep.name, evidence=dep.span))

    for imp in imports:
        if imp.resolved_path in FRAMEWORK_IMPORT_TABLE:
            detected.append(framework(imp.resolved_path, evidence=imp.span))

    for symbol in symbols:
        if symbol.qualified_name in FRAMEWORK_TYPE_TABLE:
            detected.append(framework_for_type(symbol.qualified_name, evidence=symbol.span))

    return dedupe_by_framework_and_scope(detected)
```

Do not rely on bare local names. `router.get` only means Express if `router` is tied to `express.Router()` or a provider says so.

## Component Discovery

Components are framework-owned objects such as app, router, controller, server, blueprint, APIRouter, Spring controller class, or MCP server.

```python
def discover_components(project, frameworks, syntax, symbols, references):
    components = []

    for call in references.calls:
        callee = resolve_callee_identity(call)

        if callee in COMPONENT_FACTORIES:
            value = assigned_value(call)
            components.append(Component(
                id=stable_component_key(value, call),
                framework=COMPONENT_FACTORIES[callee].framework,
                kind=COMPONENT_FACTORIES[callee].kind,
                symbol=value.symbol,
                evidence=call.span,
                precision=precision_for(call),
            ))

    for declaration in syntax.declarations:
        decorators = resolved_decorators(declaration)
        annotations = resolved_annotations(declaration)

        if matches_controller(decorators, annotations):
            components.append(Component(
                id=stable_decl_key(declaration),
                kind="controller",
                symbol=declaration.symbol,
                evidence=decorator_or_annotation_span(declaration),
            ))

    return components
```

## Registration Discovery

```python
def discover_registrations(project, frameworks, components, references):
    registrations = []

    for call in references.calls:
        recv = resolve_receiver_component(call.receiver, components)
        method = call.method_name

        if recv and method in ROUTE_METHODS[recv.framework]:
            registrations.append(parse_route_registration(recv, call))

        if recv and method in MIDDLEWARE_METHODS[recv.framework]:
            registrations.append(parse_middleware_registration(recv, call))

        if recv and method in TOOL_METHODS[recv.framework]:
            registrations.append(parse_protocol_tool_registration(recv, call))

        if call.callee in GLOBAL_REGISTRATION_FUNCTIONS:
            registrations.append(parse_global_registration(call))

    for decl in syntax.declarations:
        if has_route_decorator_or_annotation(decl):
            registrations.append(parse_decorator_registration(decl))

    return registrations
```

Route registration parsers should emit unknowns for dynamic arguments:

```python
def parse_route_registration(component, call):
    path = literal_or_unknown(call.arg(PATH_ARG))
    method = method_from_call_or_arg(call)
    handler = resolve_handler_arg(call)

    if handler is None:
        return UnresolvedRegistration(
            reason="handler argument is dynamic",
            evidence=call.span,
            component=component.id,
        )

    return Registration(
        kind="http_route",
        component=component.id,
        method=method,
        path=path,
        handler=handler,
        evidence=call.span,
        precision=precision_from(path, handler),
    )
```

## Graph Composition

Composition handles prefixes, mounted routers, controller class prefixes, middleware order, dependencies, and lifecycle hooks.

```python
def compose_framework_graph(components, registrations, lifecycle):
    graph = FrameworkGraph()

    for component in components:
        graph.add_component(component)

    for reg in registrations:
        graph.add_registration(reg)

    changed = True
    iterations = 0
    while changed and iterations < MAX_COMPOSITION_ITERATIONS:
        changed = False
        iterations += 1

        for mount in graph.mounts():
            changed |= propagate_prefix_and_middleware(graph, mount)

        for group in graph.groups():
            changed |= propagate_group_context(graph, group)

        for controller in graph.controllers():
            changed |= compose_class_and_method_routes(graph, controller)

        for plugin in graph.plugins():
            changed |= propagate_plugin_prefixes(graph, plugin)

    if iterations == MAX_COMPOSITION_ITERATIONS and changed:
        graph.add_unknown(reason="composition budget exceeded")

    return graph
```

## Target Resolution

```python
def resolve_targets(graph, symbols, references):
    for reg in graph.registrations:
        if reg.handler.is_resolved():
            continue

        candidates = resolve_expression_to_symbols(reg.handler_expr, symbols, references)

        if len(candidates) == 1:
            reg.handler = candidates[0]
            reg.precision = min(reg.precision, "ResolvedStatic")
        elif len(candidates) > 1:
            reg.handler = SyntheticTarget("ambiguous")
            reg.precision = "Conservative"
            graph.add_unknown(reg, reason="multiple handler candidates")
        else:
            graph.add_unknown(reg, reason="handler unresolved")

    return graph
```

## Trust Boundary Attachment

```python
def attach_trust_boundaries(graph, frameworks, symbols, references):
    for entrypoint in graph.entrypoints():
        model = SOURCE_MODEL_TABLE[entrypoint.framework]

        for param in entrypoint.handler.parameters:
            if model.parameter_is_request(param):
                graph.add_source(entrypoint, param, kind=model.request_kind(param))

        for expr in references.inside(entrypoint.handler):
            access = model.match_request_access(expr)
            if access:
                graph.add_source(entrypoint, expr, kind=access.kind, access_path=access.path)

            output = model.match_protocol_output(expr)
            if output:
                graph.add_boundary_sink(entrypoint, expr, kind=output.kind)

    return graph
```

## Fact Emission

```python
def emit_facts(graph):
    facts = []

    for route in graph.routes():
        facts.append(EntrypointFact(
            stable_key=route_key(route),
            kind="http_route",
            target=route.handler,
            registration=route.evidence,
            trigger=route.method_path_metadata(),
            precision=route.precision,
            provenance=route.provenance,
        ))

        for source in route.sources:
            facts.append(TrustBoundaryFact(
                stable_key=source_key(route, source),
                entrypoint=route.id,
                source_kind=source.kind,
                expression=source.expression,
                precision=source.precision,
            ))

        for edge in route.dispatch_edges:
            facts.append(FrameworkDispatchEdgeFact(
                stable_key=edge_key(edge),
                from=edge.from_,
                to=edge.to,
                edge_kind=edge.kind,
                guard=edge.guard,
                precision=edge.precision,
            ))

    for unknown in graph.unknowns:
        facts.append(UnresolvedFrameworkFact(...))

    return facts
```

## Validation And Merge

```python
def validate_merge_cache(facts):
    accepted = []
    rejected = []

    for fact in facts:
        if not schema_valid(fact):
            rejected.append(reject(fact, "schema"))
            continue
        if not spans_exist(fact):
            rejected.append(reject(fact, "span"))
            continue
        if not targets_resolve_or_are_synthetic(fact):
            rejected.append(reject(fact, "target"))
            continue
        if fact.precision > provider_precision_ceiling(fact.provider):
            rejected.append(reject(fact, "precision ceiling"))
            continue
        accepted.append(normalize(fact))

    merged = deterministic_merge(accepted)
    emit_model_diagnostics(rejected, conflicts(merged))
    write_layer_cache(merged)
    return merged
```

## Integration With Call Graph

```python
def build_call_graph_with_framework_overlay(call_sites, direct_edges, framework_edges, entrypoints):
    graph = CallGraph()
    graph.add_edges(direct_edges)

    for ep in entrypoints:
        graph.add_edge(SYNTHETIC_ROOT, ep.target, kind="entrypoint", provenance=ep.id)

    for edge in framework_edges:
        if edge.precision.allowed_for_call_graph():
            graph.add_edge(edge.from_, edge.to, kind=edge.edge_kind, provenance=edge.id)
        else:
            graph.add_unknown(edge, reason="framework edge below precision threshold")

    return graph
```

## Integration With Data Flow

```python
def seed_dataflow(entrypoints, trust_boundaries, boundary_sinks):
    sources = []
    sinks = []

    for boundary in trust_boundaries:
        if boundary.precision.allowed_for_dataflow():
            sources.append(DataFlowSource(boundary.expression, boundary.source_kind))

    for sink in boundary_sinks:
        sinks.append(DataFlowSink(sink.expression, sink.kind))

    return sources, sinks
```

Data flow should not treat every entrypoint as attacker-controlled. It should use explicit trust-boundary facts.
