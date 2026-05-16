# Cache And Invalidation Algorithm

## Layer Cache Key

```python
def layer_cache_key(provider, inputs, config, lifecycle, options, versions):
    return hash_json({
        "provider_id": provider.id,
        "provider_version": provider.version,
        "provider_schema": provider.schema_version,
        "output_schemas": provider.output_schema_versions,
        "input_layer_digests": sorted(inputs.layer_digests),
        "config_digest": config.digest_for(provider),
        "lifecycle_digest": lifecycle.digest_for(provider.language_scope),
        "options_digest": options.digest_for(provider),
        "polint_version": versions.polint,
        "kernel_version": versions.kernel,
    })
```

Rule code should not be part of parser/provider cache keys unless it changes provider behavior.

## Source And Shape Digests

```python
def source_digest(file):
    return hash(file.relative_path, file.content_hash)

def import_shape_digest(file_imports):
    normalized = [
        (imp.language, imp.path, imp.kind, imp.span.start, imp.span.end)
        for imp in file_imports
    ]
    return hash(sorted(normalized))

def symbol_shape_digest(symbols):
    normalized = [
        (sym.language, sym.stable_key, sym.kind, sym.namespace, sym.visibility, sym.signature)
        for sym in symbols
        if sym.affects_downstream_shape
    ]
    return hash(sorted(normalized))
```

Downstream providers should depend on the narrowest stable shape digest they need, not always raw source content.

## Cache Lookup

```python
def run_provider_with_cache(provider, db, cache):
    key = layer_cache_key(provider, collect_inputs(provider, db), ...)

    cached = cache.read(key)
    if cached and validate_cached_layer(cached, provider, db):
        return ProviderResult.from_cache(cached)

    result = provider.run(db.snapshot())
    validated = validate_provider_result(result, db)

    if validated.cacheable and not validated.failed:
        cache.write(key, normalize_for_cache(validated))

    return validated
```

## Invalidation Cases

| Change | Should invalidate |
|---|---|
| File content changed, imports unchanged | syntax for that file, local facts; not necessarily module graph |
| File import changed | syntax for file, module graph, symbol graph, dependent call/data-flow summaries |
| Exported symbol signature changed | symbol graph dependents, call/data-flow summaries that reference that symbol |
| Rule code changed | rule diagnostics only, unless rule options influence provider plan |
| `.polint.toml` language lifecycle changed | affected language providers and downstream layers |
| Extension source changed | extension layer and downstream layers |
| Extension fixture changed | validation status and dependent layer promotion |
| Provider schema changed | that provider's cached layers |
| polint version/kernel version changed | all affected layers according to schema compatibility |

## Presence Dependencies

Some results depend on absence:

```python
Dependency:
    kind: "value" | "presence" | "absence" | "membership"
    key: StableKey
```

Examples:

- "no symbol matched selector X"
- "module foo did not resolve"
- "no sanitizer model exists for function Y"
- "entrypoint extension was not installed"

These must invalidate when the missing thing appears.

## Cold-vs-Incremental Equivalence

Every future incremental optimization should pass:

```python
def assert_incremental_equals_cold(repo, edit):
    cold_before = run_cold(repo)
    incremental_state = start_incremental(repo)

    apply_edit(repo, edit)

    cold_after = run_cold(repo)
    incremental_after = incremental_state.update(edit)

    assert normalized_facts(cold_after) == normalized_facts(incremental_after)
    assert normalized_diagnostics(cold_after) == normalized_diagnostics(incremental_after)
```

Do this per provider and for end-to-end rule output.

