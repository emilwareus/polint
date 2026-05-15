# Provider Scheduling Algorithm

This is the target mental model for the first kernel scheduler.

## Provider Manifest

```python
class Provider:
    id: str
    inputs: set[FactFamily]
    outputs: set[FactFamily]
    language_scope: set[Language]
    cache_policy: CachePolicy
    provider_kind: "native" | "derived" | "extension" | "relation"
    precision_ceiling: Precision
```

## Build Plan

```python
def build_kernel_plan(rules, extensions, providers):
    requested = set()

    for rule in rules:
        requested |= rule.required_fact_families

    for ext in extensions:
        requested |= ext.required_inputs
        if ext.outputs_satisfy_any(requested):
            requested |= ext.declared_outputs

    required = dependency_closure(requested, providers)
    selected = select_providers(required, providers)

    graph = ProviderGraph()
    for provider in selected:
        graph.add_node(provider)
        for input_family in provider.inputs:
            producer = selected_producer_for(input_family, selected)
            if producer:
                graph.add_edge(producer, provider)

    return KernelPlan(
        requested_families=requested,
        required_families=required,
        provider_graph=graph,
        plan_digest=digest_plan(graph, requested),
    )
```

## Execute Plan

```python
def execute_kernel_plan(plan, db, cache):
    support = CapabilitySupportView.initial(plan.requested_families)
    diagnostics = []

    for batch in topological_batches(plan.provider_graph):
        runnable = []

        for provider in batch:
            if dependencies_failed(provider, support):
                support.block_outputs(provider, "BlockedByDependency")
                diagnostics.append(capability_diag(provider, "blocked"))
                continue
            runnable.append(provider)

        results = run_batch_deterministically(runnable, db.snapshot(), cache)

        for provider, result in sorted_by_provider_id(results):
            diagnostics.extend(result.diagnostics)

            if result.failed:
                support.mark_outputs(provider, result.failure_status)
                continue

            valid_layers = []
            for layer in result.layers:
                validated = validate_layer(layer, db, provider)
                diagnostics.extend(validated.diagnostics)
                if validated.accepted:
                    valid_layers.append(validated.layer)
                else:
                    support.mark_outputs(provider, "ValidationFailed")

            merge_result = merge_layers(db, valid_layers)
            diagnostics.extend(merge_result.diagnostics)
            support.apply(merge_result.capability_support_delta)

    return KernelOutput(db=db, diagnostics=diagnostics, support=support)
```

## Recursive Families

Some providers are not simple DAG nodes. They are fixpoint groups.

```python
def run_fixpoint_group(group, db, budget):
    relations = initialize_relations(group.inputs, db)
    deltas = initial_deltas(relations)
    iteration = 0

    while any_nonempty(deltas):
        if iteration >= budget.max_iterations:
            return ProviderResult.failed("BudgetExceeded")

        new_rows = {}
        for rule in group.rules:
            rows = evaluate_rule_with_delta(rule, relations, deltas)
            new_rows[rule.output] |= rows

        next_deltas = {}
        for relation, rows in new_rows.items():
            fresh = rows - relations[relation]
            relations[relation] |= fresh
            next_deltas[relation] = fresh

        deltas = next_deltas
        iteration += 1

    return materialize_relation_layers(relations)
```

## Determinism Rules

- Providers are sorted by provider ID before merge.
- Files are merged by stable file order.
- Facts are normalized before hashing.
- Diagnostics are deduped and sorted.
- Parallel execution cannot affect output order.

## Failure Rules

- Provider panic becomes `ProviderFailed`.
- Timeout becomes `TimedOut`.
- Budget exhaustion becomes `BudgetExceeded`.
- Validation failure becomes `ValidationFailed`.
- Downstream providers become `BlockedByDependency`.
- Rules with hard required capabilities do not run unless all required families are supported.

