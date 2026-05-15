# Extension Lifecycle Algorithms

The pseudocode is Python-ish and deliberately stripped down. It describes the engine shape, not Rust syntax.

## Planning

```python
def build_analysis_plan(rules, extensions, config, target_files):
    rule_needs = union(rule.requested_fact_views for rule in rules)
    extension_outputs = union(ext.declared_outputs for ext in extensions)
    extension_inputs = union(ext.declared_inputs for ext in extensions)

    required_facts = closure(rule_needs + extension_inputs, dependencies={
        "references": ["symbols"],
        "call_graph": ["symbols", "references", "module_graph"],
        "dataflow": ["cfg", "symbols", "references", "call_graph"],
        "effects": ["symbols", "references", "call_graph"],
    })

    schedule = topological_sort([
        native_base_facts,
        extensions_that_produce("entrypoints"),
        native_call_graph,
        extensions_that_produce("call_graph"),
        native_cfg,
        native_dataflow,
        extensions_that_produce("dataflow"),
        native_effects,
        extensions_that_produce("effects"),
        rules,
    ])

    return Plan(required_facts, schedule, digest_inputs(rules, extensions, config, target_files))
```

## Extension Discovery

```python
def discover_extensions(repo):
    manifests = find(repo / ".polint/extensions/*/Cargo.toml")
    result = []

    for manifest in manifests:
        meta = read_extension_manifest(manifest)
        validate_id(meta.id)
        validate_sdk_version(meta.sdk_version)
        result.append(ExtensionCrate(manifest, meta))

    return sort_by_id(result)
```

## Build And Handshake

```python
def prepare_extension(ext, host_protocol_version):
    binary = build_or_reuse_cached_binary(
        manifest=ext.manifest,
        digest=hash_files(ext.source_files, ext.cargo_lock, ext.sdk_version),
    )

    response = run(binary, input={
        "command": "handshake",
        "host_protocol_version": host_protocol_version,
    }, timeout="short")

    if not response.supports(host_protocol_version):
        raise CapabilitySetupError(ext.id, "protocol mismatch")

    return PreparedExtension(binary, response.capabilities)
```

## Running A Provider

```python
def run_provider(prepared, provider, input_views):
    request = {
        "command": "run_provider",
        "extension_id": prepared.id,
        "provider_id": provider.id,
        "input_fact_views": serialize_views(input_views),
        "output_fact_families": provider.outputs,
        "budget": provider.budget,
    }

    output = run(prepared.binary, input=request, timeout=provider.budget.timeout)

    if output.status != "ok":
        return ProviderFailure(output.error)

    validated = []
    for fact in output.facts:
        if validate_fact(fact, input_views):
            validated.append(stamp_provenance(fact, prepared, provider))
        else:
            emit_validation_diagnostic(fact)

    return validated
```

## Fact Validation

```python
def validate_fact(fact, db):
    if fact.family == "call_edge":
        return (
            db.has_callsite(fact.callsite_id)
            and db.has_function_or_synthetic(fact.target_id)
            and fact.precision in allowed_precisions
        )

    if fact.family == "dataflow_summary":
        return (
            db.has_symbol(fact.callable)
            and valid_access_path(fact.input)
            and valid_access_path(fact.output)
            and fact.kind in ["taint", "value", "control"]
        )

    if fact.family == "entrypoint":
        return db.has_symbol(fact.target) and fact.kind in known_entrypoint_kinds

    return schema_validate(fact)
```

## Merge And Provenance

```python
def merge_extension_facts(native_facts, extension_facts):
    all_facts = []

    for fact in native_facts:
        all_facts.append(fact.with_provenance(origin="native"))

    for fact in extension_facts:
        if fact.conflicts_with(native_facts):
            if fact.override_policy == "augment":
                all_facts.append(fact)
            elif fact.override_policy == "replace" and fact.trust_level == "trusted_repo":
                all_facts = replace_matching(all_facts, fact)
            else:
                emit_conflict_diagnostic(fact)
        else:
            all_facts.append(fact)

    return stable_dedupe_sort(all_facts)
```

## Default Versus Agent-Extended Delta

```python
def measure_extension_delta(repo, extension):
    before = run_polint(repo, extensions=[])
    after = run_polint(repo, extensions=[extension])

    return {
        "resolved_calls_delta": after.resolved_calls - before.resolved_calls,
        "unresolved_calls_delta": after.unresolved_calls - before.unresolved_calls,
        "new_entrypoints": after.entrypoints - before.entrypoints,
        "new_sources": after.sources - before.sources,
        "new_sinks": after.sinks - before.sinks,
        "new_paths": after.dataflow_paths - before.dataflow_paths,
        "removed_unknowns": before.unknowns - after.unknowns,
        "runtime_delta_ms": after.runtime_ms - before.runtime_ms,
        "validation_failures": after.extension_validation_failures,
    }
```

## Rule Execution

```python
def run_rules_after_extensions(db, rules):
    diagnostics = []

    for rule in rules:
        if any_missing_required_capability(rule):
            diagnostics.append(capability_diagnostic(rule))
            continue

        views = build_typed_views(db, rule.requested_views)
        diagnostics.extend(run_rule(rule, views))

    return stable_dedupe_sort(diagnostics)
```
