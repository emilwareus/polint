# Core Algorithms

The pseudocode here is intentionally Python-ish. It is meant to make the
algorithms inspectable without committing to exact Rust APIs.

## Input Snapshot

```python
def build_input_snapshot(repo, config, rule_registry, extension_registry):
    files = {}
    for path in discover_files(repo, config.includes, config.excludes):
        bytes = read_file(path)
        files[path] = FileSnapshot(
            path=normalize(path),
            language=detect_language(path),
            text_digest=hash_bytes(bytes),
            size=len(bytes),
            overlay_kind="disk",
        )

    return InputSnapshot(
        repo_root=repo.root,
        files=files,
        config=hash_canonical(config),
        lifecycle=build_language_lifecycle_snapshot(repo, files, config),
        toolchains=detect_toolchains(config),
        rules=snapshot_rules(rule_registry),
        extensions=snapshot_extensions(extension_registry),
        models=snapshot_model_files(repo),
    )
```

Rule: all later providers read through `InputSnapshot` or declare additional
inputs. Direct filesystem reads by providers are cache hazards.

## Layer Key Construction

```python
def make_layer_key(provider, layer, inputs):
    return LayerKey(
        layer=layer.kind,
        provider=provider.id,
        provider_version=provider.version,
        schema_version=layer.schema_version,
        params_digest=hash_canonical(provider.params),
        lifecycle_digest=inputs.lifecycle.digest_for(provider.language),
        config_digest=inputs.config.digest_for(provider),
        toolchain_digest=inputs.toolchains.digest_for(provider),
        input_digests=sorted(inputs.required_shape_digests),
        dependency_layer_digests=sorted(inputs.layer_output_digests),
        extension_digests=sorted(inputs.extension_digests),
    )
```

Rule: lists are sorted or otherwise canonicalized. Nondeterministic cache keys
are correctness and benchmarking bugs.

## Layer Cache Read

```python
def compute_layer(provider, layer_kind, cx):
    inputs = provider.collect_inputs(cx.snapshot, cx.layers)
    key = make_layer_key(provider, layer_kind, inputs)

    manifest = cx.layer_cache.get_manifest(key)
    if manifest and validate_manifest(manifest, cx.snapshot):
        cx.stats.hit(layer_kind)
        return cx.layer_cache.load_output(manifest.output_digest)

    trace = Trace()
    output = provider.compute(cx.with_trace(trace))
    output_digest = stable_digest(output)

    manifest = LayerCacheManifest(
        key=key,
        output_digest=output_digest,
        dependencies=trace.edges,
        precision=output.precision,
        validation=output.validation,
        stats=provider.stats,
    )
    cx.layer_cache.store_atomic(manifest, output)
    cx.dependency_index.record(manifest.key, trace.edges)
    cx.stats.miss(layer_kind)
    return output
```

## Change Classification

```python
def classify_file_change(old_file, new_file, old_shapes, new_shapes):
    changes = {ContentOnly}

    if old_shapes.syntax != new_shapes.syntax:
        changes.add(SyntaxShape)
    if old_shapes.imports != new_shapes.imports:
        changes.add(ImportShape)
    if old_shapes.public_api != new_shapes.public_api:
        changes.add(PublicApiShape)
    if old_shapes.framework_boundary != new_shapes.framework_boundary:
        changes.add(FrameworkBoundaryShape)
    if old_shapes.summary_effect != new_shapes.summary_effect:
        changes.add(SummaryShape)

    return changes
```

If shape extraction fails, return `Unknown`, which forces broader invalidation.

## Invalidation Planning

```python
def plan_invalidation(changes, dependency_index):
    plan = InvalidationPlan()
    queue = deque()

    for changed_input in changes.inputs:
        queue.append(CacheNode.Input(changed_input))

    seen = set()
    while queue:
        node = queue.popleft()
        if node in seen:
            continue
        seen.add(node)

        for edge in dependency_index.reverse_deps(node):
            dependent = edge.from_node
            action = classify_dependency_change(edge, changes)

            plan.add(action, dependent)

            if action in [Verify, Recompute, Drop, Quarantine]:
                queue.append(dependent)

    return plan.canonicalize()
```

Dependency classification:

```python
def classify_dependency_change(edge, changes):
    if changes.includes_provider_or_schema_change(edge):
        return Drop
    if changes.includes_untrusted_extension_input(edge):
        return Quarantine
    if changes.satisfies_shape_requirement(edge.required_shape):
        return Verify
    if changes.definitely_does_not_affect(edge.required_shape):
        return Reuse
    return Recompute
```

## Demand Query Execution

```python
def query(cx, query_kind, params):
    key = make_query_key(cx, query_kind, params)
    memo = cx.memo_table.get(key)

    if memo and is_green(cx, memo):
        cx.trace.read_query(key, memo.changed_at)
        cx.stats.query_hit(query_kind)
        return memo.value

    trace = Trace()
    value = execute_query(cx.with_trace(trace), query_kind, params)
    digest = stable_digest_with_status(value)

    old = memo
    if old and old.digest == digest:
        changed_at = old.changed_at          # backdate
        cx.stats.query_backdated(query_kind)
    else:
        changed_at = cx.current_revision
        cx.stats.query_recomputed(query_kind)

    cx.memo_table[key] = Memo(
        value=value,
        digest=digest,
        dependencies=trace.edges,
        changed_at=changed_at,
        verified_at=cx.current_revision,
        durability=derive_durability(trace.edges),
    )
    cx.trace.read_query(key, changed_at)
    return value
```

## Red-Green Verification

```python
def is_green(cx, memo):
    if memo.verified_at == cx.current_revision:
        return True

    if cx.no_changes_at_or_above(memo.durability, since=memo.verified_at):
        memo.verified_at = cx.current_revision
        return True

    for dep in memo.dependencies:
        if dependency_changed(cx, dep, since=memo.verified_at):
            if not verify_dependency(cx, dep):
                return False

    memo.verified_at = cx.current_revision
    return True
```

This only works when dependencies are complete. If a provider has undeclared
inputs, do not allow green verification.

## Equality Backdating

```python
def recompute_with_backdating(cx, key, compute):
    old = cx.cache.get(key)
    new_value = compute()
    new_digest = stable_digest_with_status(new_value)

    if old and old.digest == new_digest:
        changed_at = old.changed_at
        changed = False
    else:
        changed_at = cx.current_revision
        changed = True

    cx.cache.put(key, value=new_value, digest=new_digest, changed_at=changed_at)
    return new_value, changed
```

For polint, `stable_digest_with_status` must include semantic payload, precision,
validation status, unknown/truncated status, and relevant provenance when
consumers care about it.

## Summary SCC Update

```python
def update_summaries(cx, changed_functions):
    call_sccs = cx.call_graph.sccs()
    affected = closure_over_callers(call_sccs, changed_functions)

    for scc in topological_order(affected):
        old_digest = digest_scc_summaries(cx.summary_cache, scc)

        new_summaries = solve_scc_summaries(cx, scc)
        new_digest = digest_summaries(new_summaries)

        if new_digest == old_digest:
            backdate_scc(cx.summary_cache, scc)
            continue

        store_scc_summaries(cx.summary_cache, scc, new_summaries)
        mark_callers_dirty(cx.dependency_index, scc)
```

First implementation should recompute affected SCCs. Later monotone IDFA
domains can use an IncIDFA-like internal update.

## Extension Cache Validation

```python
def validate_extension_cache(cx, extension_id, cached_output):
    snapshot = cx.snapshot.extensions[extension_id]

    if cached_output.extension_code_digest != snapshot.crate_digest:
        return Quarantine("extension code changed")

    if cached_output.api_version != snapshot.api_version:
        return Drop("extension api changed")

    if cached_output.validation_digest != snapshot.validation_digest:
        return Quarantine("validation fixture state changed")

    for declared_input in cached_output.declared_inputs:
        if digest(declared_input) != cached_output.input_digests[declared_input]:
            return Recompute("declared extension input changed")

    if cached_output.observed_undeclared_reads:
        return Quarantine("extension had undeclared reads")

    return Reuse
```

## Diagnostic Reuse

```python
def reuse_diagnostic(cx, old_diag):
    if old_diag.rule_digest != cx.snapshot.rules[old_diag.rule_id].digest:
        return Drop

    if old_diag.options_digest != cx.rule_options_digest(old_diag.rule_id):
        return Drop

    for view_digest in old_diag.required_view_digests:
        if not cx.view_digest_still_valid(view_digest):
            return Verify

    if old_diag.evidence_digest:
        if not cx.evidence_digest_still_valid(old_diag.evidence_digest):
            return Verify

    if not source_anchor_exists(cx.snapshot, old_diag.primary_anchor):
        return Drop

    return Reuse
```

Diagnostics are downstream; cache them only after fact/query caches are stable.
