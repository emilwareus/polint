# Abstract Interpretation Algorithms

This file gives stripped-down pseudo-code for the algorithms polint should
implement or keep as explicit future precision tiers.

## Deterministic Worklist Solver

```python
def solve_cfg(cfg, domain, initial):
    in_state = {bb: domain.bottom() for bb in cfg.blocks}
    out_state = {bb: domain.bottom() for bb in cfg.blocks}
    in_state[cfg.entry] = initial

    queue = StableQueue(cfg.reverse_postorder())

    while queue:
        bb = queue.pop()
        state = in_state[bb].copy()

        for op in cfg.block_ops(bb):
            state = transfer_op(op, state)
            state = reduce_product(state)

        if out_state[bb].join_into(state):
            for edge in cfg.successors(bb):
                edge_state = transfer_edge(edge, out_state[bb])
                candidate = edge_state

                if edge.dst in cfg.widen_points and should_widen(edge.dst):
                    candidate = in_state[edge.dst].widen(candidate, site=edge.dst)

                if in_state[edge.dst].join_into(candidate):
                    queue.push(edge.dst)

    return Fixpoint(in_state, out_state)
```

Use stable block IDs and stable queue order. Do not let hash-map iteration affect
results.

## Branch Refinement

```python
def transfer_edge(edge, state):
    if edge.kind == "true":
        return assume(state, edge.predicate, True)
    if edge.kind == "false":
        return assume(state, edge.predicate, False)
    if edge.kind == "call_return":
        return apply_call_return_effect(state, edge.return_place)
    if edge.kind == "unwind":
        return apply_unwind_effect(state, edge.exception)
    return state
```

Edge-specific effects are required for precision. A call's success return should
not be applied to unwind/panic edges.

## Reduced Product

```python
def reduce_product(state):
    for _ in range(MAX_REDUCTION_ROUNDS):
        old = digest(state)

        if state.constants.is_singleton_none():
            state.nilness.refine_to_nil()

        for place, literal in state.constants.literals():
            state.truthiness.refine(place, truthiness_of(literal))
            state.strings.refine_from_literal(place, literal)
            state.ranges.refine_from_literal(place, literal)

        for pred in state.predicates.active():
            state.nilness.refine_from_predicate(pred)
            state.constants.refine_from_predicate(pred)
            state.shape.refine_from_predicate(pred)

        if digest(state) == old:
            break

    return state
```

Reductions are versioned cache inputs. New reductions can change results even
when individual domains do not change.

## Widening With Thresholds

```python
def interval_widen(prev, next, thresholds):
    lower = prev.lower
    upper = prev.upper

    if next.lower < prev.lower:
        lower = greatest_threshold_leq(next.lower, thresholds) or NEG_INF

    if next.upper > prev.upper:
        upper = smallest_threshold_geq(next.upper, thresholds) or POS_INF

    return Interval(lower, upper)
```

Thresholds should come from:

- numeric literals;
- array/string length comparisons;
- loop conditions;
- enum/discriminant values;
- config and extension-provided invariants.

## Narrowing

```python
def interval_narrow(prev, next):
    lower = next.lower if prev.lower == NEG_INF else prev.lower
    upper = next.upper if prev.upper == POS_INF else prev.upper
    return Interval(lower, upper)
```

Narrowing should run a bounded number of times after a widened fixpoint. It must
only refine within the post-fixpoint envelope.

## Trace Partitioning

```python
def partition_branch(state, predicate, sense, policy):
    if not policy.should_partition(predicate):
        return assume(state, predicate, sense)

    key = PartitionKey.from_predicate(predicate, sense)
    part = state.partition(key)
    part = assume(part, predicate, sense)
    state.update_partition(key, part)

    if state.partition_count > policy.max_partitions:
        state.merge_lowest_priority_partitions()

    return state
```

Initial partition predicates:

- Go `err != nil`, `x == nil`;
- TS/JS `typeof`, `x == null`, discriminant property equality;
- Python `x is None`, `isinstance`, `TypeGuard`, `TypedDict` tag checks;
- extension-provided guard functions.

## Packed Octagons

```python
def select_numeric_pack(cfg, rule_request):
    vars = set()
    vars |= vars_in_loop_guards(cfg)
    vars |= vars_in_array_indices(cfg)
    vars |= vars_in_numeric_assertions(cfg)
    vars |= rule_request.numeric_vars
    return partition_into_small_packs(vars, max_pack_size=8)
```

Do not run octagons globally. Keep packs small and explain when variables are
excluded by budget.

## Summary Fixpoint

```python
def solve_summaries(call_graph, domain):
    summaries = SummaryStore.bottom()

    for scc in call_graph.reverse_topological_sccs():
        changed = True
        iteration = 0
        while changed and iteration < MAX_SUMMARY_ITERS:
            changed = False
            iteration += 1

            for fn in stable_order(scc.functions):
                local = solve_cfg(fn.cfg, domain, entry_from_signature(fn))
                next_summary = project_summary(local, fn)
                old_summary = summaries[fn]

                if scc.is_recursive and should_widen_summary(fn, iteration):
                    next_summary = old_summary.widen(next_summary, site=fn)

                if summaries[fn].join_into(next_summary):
                    changed = True

        if changed:
            mark_budget_exceeded(scc)

    return summaries
```

Summary digests include domain versions, config, extension source hashes,
semantic input digests, and dependent summary digests.

## Extension Validation

```python
def validate_domain(domain, samples):
    assert reflexive(domain.leq, samples)
    assert antisymmetric(domain.leq, samples)
    assert transitive(domain.leq, samples)
    assert join_idempotent(domain.join, samples)
    assert join_commutative(domain.join, samples)
    assert join_associative(domain.join, samples)
    assert join_upper_bound(domain.leq, domain.join, samples)

    for transfer in domain.transfers:
        assert monotone(transfer, domain.leq, samples)

    assert stable_serialization(domain, samples)
    assert deterministic_hash(domain, samples)
```

If runtime sampling finds a non-monotone extension transfer, disable that
component, emit an extension diagnostic, and continue.
