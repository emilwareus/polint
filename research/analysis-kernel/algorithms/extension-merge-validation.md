# Extension Merge And Validation Algorithm

## Extension Output Flow

```python
def run_extension_provider(extension, provider, db):
    raw = extension.run_provider(
        provider_id=provider.id,
        input_view=build_extension_input_view(db, provider.inputs),
    )

    schema_valid = schema_validate(raw, provider.declared_outputs)
    if schema_valid.failed:
        return reject(schema_valid.diagnostics)

    resolved = resolve_extension_facts(schema_valid.facts, db)
    checked = check_precision_and_scope(resolved, provider)
    normalized = normalize_facts(checked.accepted)

    merge = merge_extension_layer(db, normalized, provider)

    return ProviderResult(
        layers=[merge.layer],
        diagnostics=schema_valid.diagnostics + checked.diagnostics + merge.diagnostics,
        support=merge.support_delta,
    )
```

## Validation

```python
def validate_fact(fact, db, provider):
    if fact.family not in provider.outputs:
        return rejected("provider did not declare this output family")

    if fact.precision > provider.precision_ceiling:
        return rejected("precision exceeds provider ceiling")

    if not valid_stable_key(fact.stable_key):
        return rejected("missing or invalid stable key")

    if fact.span and not span_in_bounds(fact.span, db):
        return rejected("span out of bounds")

    for ref in fact.references:
        if not db.contains(ref):
            return rejected("referenced fact does not exist")

    if fact.selector:
        matches = resolve_selector(fact.selector, db)
        if not matches and not fact.allows_zero_matches:
            return rejected("selector matched no facts")
        fact.confidence = confidence_from_cardinality(matches)

    return accepted(fact)
```

## Merge

```python
def merge_extension_layer(db, facts, provider):
    existing = db.fact_index()
    accepted = []
    diagnostics = []

    for fact in sorted_by_stable_key(facts):
        current = existing.get((fact.family, fact.stable_key))

        if current is None:
            accepted.append(fact)
            continue

        if equivalent_payload(current, fact):
            merge_provenance(current, fact)
            continue

        if current.precision == "Exact" and fact.precision != "Exact":
            diagnostics.append(shadowed_by_exact(current, fact))
            record_shadow_fact(fact)
            continue

        if current.precision == "Exact" and fact.precision == "Exact":
            diagnostics.append(conflict_error(current, fact))
            continue

        if fact.is_suppression:
            if not suppression_allowed(fact.family):
                diagnostics.append(reject_suppression(fact))
            elif not fact.validation == "FixtureValidated":
                diagnostics.append(reject_unvalidated_suppression(fact))
            else:
                accepted.append(fact)
            continue

        accepted.append(fact)

    return MergeResult(
        layer=LayerOutput(facts=accepted),
        diagnostics=diagnostics,
    )
```

## Merge Policy By Fact Kind

| Fact kind | First policy |
|---|---|
| Entrypoints | Additive union. |
| Call edges | Additive union with provenance and precision. |
| Unresolved call explanations | Additive; can link to resolved alternatives but not delete unknowns. |
| Sources/sinks | Additive union. |
| Sanitizers/barriers | Additive only after stricter validation; suppressive semantics require fixtures. |
| Function summaries | Additive union by callable and access path; conflicts warn or reject. |
| Effects | Additive union; exact conflicts reject. |
| Symbols/references | Extension augmentation only at first; native replacement delayed. |
| Diagnostics | Rule-owned; extension diagnostics are separate. |

## Delta Report

Every extension run should be able to report:

```text
added facts
rejected facts
shadowed facts
conflicts
unknowns resolved or narrowed
downstream graph/path differences
new diagnostics
removed diagnostics
runtime/cache impact
```

This is how agents know whether an extension improved accuracy or hid uncertainty.

