# Summary Algorithms In Pseudo-code

This file strips the main algorithms down to Python-ish pseudo-code.

## Local Summary Builder

```python
def local_summary(fn, facts, domain):
    state = domain.initial(fn)

    for block in facts.cfg(fn).reverse_postorder():
        for op in block.ops:
            state = domain.transfer_op(op, state, facts)

    return domain.project(fn, state).with_precision("ExactLocal")
```

Use this for the first implementation. It is cheap, deterministic, cacheable,
and gives useful facts even before full interprocedural closure.

## Summary Application At A Call

```python
def transfer_call(call, state, domain, summaries):
    targets = resolve_targets(call)

    if not targets:
        return state.join(domain.unknown_call(call, reason="unresolved"))

    out = domain.bottom_state()

    for callee in targets:
        summary = summaries.get(callee, domain.id, context=domain.context(call))
        if summary is None:
            out = out.join(domain.unknown_call(call, reason="missing-summary"))
        elif summary.status.is_usable():
            out = out.join(domain.apply_summary(call, state, summary.payload))
        else:
            out = out.join(domain.apply_incomplete_summary(call, state, summary))

    return out
```

Unknown is joined as top/unknown for the relevant domain. It is never treated as
no effect.

## Bottom-up SCC Closure

```python
def close_summaries(program, domain, summary_store):
    graph = program.call_graph()

    for scc in reverse_topological_sccs(graph):
        if is_trivial_non_recursive(scc):
            fn = scc[0]
            summary = analyze_with_store(fn, domain, summary_store)
            summary_store.put(summary)
            continue

        state = {fn: domain.bottom_summary(fn) for fn in scc}
        budget_exceeded = False

        for iteration in range(domain.max_iterations):
            changed = False

            for fn in deterministic_order(scc):
                next_summary = analyze_with_scc_state(fn, domain, summary_store, state)

                if iteration >= domain.widen_after:
                    next_summary = domain.widen(iteration, state[fn], next_summary)

                joined = domain.join(state[fn], next_summary)
                if not domain.less_equal(joined, state[fn]):
                    state[fn] = joined
                    changed = True

            if not changed:
                break
        else:
            budget_exceeded = True

        for fn, summary in state.items():
            if budget_exceeded:
                summary = summary.with_status("BudgetExceeded")
            else:
                summary = summary.with_status("Complete")
            summary_store.put(summary.with_precision("SummaryBased"))
```

## Demand Summary

```python
def answer_query(query, store, domain):
    cached = store.get_demand(query.key())
    if cached and cached.inputs_still_valid():
        return cached.answer

    frontier = Worklist([query.seed])
    answer = domain.bottom_answer()

    while frontier and not query.budget.exceeded():
        item = frontier.pop()
        if domain.can_answer_locally(item):
            answer = answer.join(domain.local_answer(item))
            continue

        for callee_query in domain.expand(item):
            if store.has_demand(callee_query.key()):
                answer = answer.join(store.get_demand(callee_query.key()))
            else:
                frontier.push(callee_query)

    if query.budget.exceeded():
        answer = answer.with_status("BudgetExceeded")

    store.put_demand(query.key(), answer)
    return answer
```

Use demand summaries for expensive alias/path/evidence queries after eager
local and SCC summaries exist.

## Extension Summary Validation

```python
def validate_extension_summary(candidate, facts, policy):
    errors = []

    subject = facts.resolve_callable(candidate.selector)
    if subject is None and not candidate.declares_synthetic_subject:
        errors.append("selector did not resolve")

    if subject and candidate.signature_hash not in [subject.signature_hash, "*"]:
        errors.append("signature hash mismatch")

    for endpoint in candidate.flow_endpoints:
        if not facts.is_valid_access_path(subject, endpoint):
            errors.append(f"invalid access path: {endpoint}")

    if candidate.precision not in policy.allowed_precision(candidate.provider):
        errors.append("precision claim too strong")

    if candidate.merge_mode == "replace" and not candidate.has_required_fixtures:
        errors.append("replace requires fixtures")

    if candidate.has_no_evidence():
        errors.append("summary requires source/model evidence")

    if errors:
        return Rejected(errors)
    return Accepted(candidate.with_precision_ceiling(policy.precision_ceiling))
```

## Summary Cache Key

```python
def summary_key(subject, domain, context, inputs):
    return hash_tuple(
        subject.stable_id,
        subject.signature_hash,
        subject.language,
        subject.package,
        domain.id,
        domain.version,
        context.key,
        inputs.source_digest,
        inputs.semantic_digest,
        inputs.config_digest,
        inputs.setup_digest,
        inputs.dependency_summary_digest,
        inputs.extension_digest,
        inputs.budget_digest,
    )
```

Function names are not enough. Keys must include source, setup, model, extension,
and dependency-summary inputs.

## Typed View Query

```python
def effects_may_write(callable_id, resource_selector):
    summaries = summary_store.get_bundle(callable_id, "MemoryEffects")

    if summaries.missing_required_inputs():
        return Unknown("missing summary inputs")

    answer = Maybe()
    for summary in summaries:
        if summary.payload.accesses.matches(resource_selector, kind="write"):
            answer = Yes(summary.evidence)
        elif summary.payload.has_unknown_external_write():
            answer = answer.join(Maybe(summary.evidence))

    return answer.with_precision(summaries.combined_precision())
```

Public views return `Yes`, `No`, `Maybe`, or `Unknown`, not raw payloads.
