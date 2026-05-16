# Domain Summary Algebra

Summaries are the boundary between local abstract interpretation and scalable
interprocedural facts. Domain summaries must plug into the shared summary kernel
from `research/effects-summaries/`; this file specifies the abstract-domain
payload contract.

## Summary Key

The shared summary key must include at least:

```rust
pub(crate) struct SummaryKey {
    callable_id: CallableId,
    domain_id: DomainId,
    domain_version: DomainVersion,
    language_id: LanguageId,
    package_or_module_id: PackageOrModuleId,
    signature_hash: Hash,
    semantic_digest: Hash,
    context_key: ContextKey,
    config_digest: Hash,
    setup_digest: Hash,
    extension_digest: Hash,
    dependency_summary_digest: Hash,
}
```

`ContextKey` must be explicit. Start with context-insensitive direct summaries,
then allow bounded variants:

- receiver/type context;
- call-string `k`;
- framework entrypoint context;
- type-argument context;
- selected trace-partition context;
- allocation-site context.

Context policy, context budget, and eviction policy are cache inputs.

## Summary Shape

Domain summary payloads use canonical roots:

```text
Param(i)
Receiver
Return
Global(symbol)
Allocation(alloc)
Unknown
```

Required fields:

```rust
pub(crate) struct DomainSummaryPayload {
    requires: Vec<DomainRequirement>,
    ensures: Vec<DomainEnsure>,
    returns: Vec<DomainReturnFact>,
    throws: Vec<DomainThrowFact>,
    modifies: Vec<Modification>,
    invalidates: Vec<Invalidation>,
    flows: Vec<TitoFlow>,
    guard_refinements: Vec<GuardRefinement>,
    typestate_transitions: Vec<TypestateTransition>,
    unknowns: Vec<UnknownFact>,
    diagnostics: Vec<LatentDiagnostic>,
    precision: Precision,
    status: SummaryStatus,
    provenance: ProvenanceId,
    dependencies: Vec<SummaryDependency>,
    digest: Hash,
}
```

## Algebra

Each domain summary defines:

- `bottom`: no reachable behavior or no facts, with polarity documented;
- `top`: conservative unknown summary, not failure;
- `leq`: precision/order relation;
- `join_into`: fieldwise least practical upper bound using each fact family's
  merge policy;
- `widen`: only at loop/SCC summary headers, with delay/fuel and
  precision-loss evidence;
- `narrow`: bounded post-widening refinement within the widened envelope.

Conflicts use declared merge policy:

| Policy | Behavior |
|---|---|
| `Join` | Add possible behavior or facts. |
| `MeetForPrecision` | Refine only when evidence proves the stronger fact remains conservative. |
| `ConservativeTopOnConflict` | Drop conflicting precision to unknown/top. |
| `RejectConflict` | Reject the extension/model payload and emit diagnostics. |

Use `join_into` / `JoinResult` semantics in solver code. Avoid open-coded
`if !candidate.leq(old)` checks.

## Summary Fixpoint

```python
def solve_summary_scc(scc, store, domain, policy):
    for iteration in range(policy.max_iters):
        changed = False

        for fn in stable_order(scc.functions):
            local = solve_body(fn, store.current_view())
            projected = project_summary(local, fn, domain)
            candidate = projected

            if scc.is_recursive and policy.should_widen(fn, iteration):
                candidate = store[fn].widen(candidate, site=fn, fuel=policy.fuel)

            if store[fn].join_into(candidate):
                changed = True

        if not changed:
            return SummaryFixpoint.Complete

    for fn in stable_order(scc.functions):
        store[fn].mark_budget_exceeded()
        store[fn].add_unknown("summary_scc_budget_exceeded")
    return SummaryFixpoint.BudgetExceeded
```

Widening delay and thresholds are part of the policy. They should be explicit in
pseudo-code and cache keys.

## Caller-Place Substitution

At call application:

| Callee Root | Caller Mapping |
|---|---|
| `Param(i)` | actual argument place |
| `Receiver` | receiver/base place |
| `Return` | destination place or expression result |
| callee allocation | call-site abstract allocation |
| `Global(symbol)` | same resolved global |
| unknown/dynamic projection | region invalidation |

Application rules:

- `requires` refine caller state or produce capability/setup diagnostics.
- `returns` and `ensures` add facts to substituted caller places.
- `modifies` and `invalidates` forget or havoc substituted caller places and
  reachable aliases.
- `throws` apply only to exceptional/unwind/reject edges.
- `guard_refinements` apply only when the call is used as the controlling guard
  or a language-specific guard expression.

## Unknown And Havoc Behavior

Missing or unresolved callee summary must emit an explicit unknown fact.

Default behavior:

- return facts become domain top for the call result;
- havoc mutable/reachable places only;
- do not top the whole state unless the domain has no smaller sound region.

Relevant havoc regions:

- by-reference arguments;
- mutable receiver;
- escaped heap reachable from arguments;
- globals when applicable;
- language-specific dynamic regions;
- async/callback regions when scheduling is unknown.

Purity/effect summaries or alias facts may shrink the havoc set.

## Cache Invalidation

Summary digests include:

- source and semantic digests;
- parser and adapter versions;
- semantic operation schema;
- domain id/version;
- lattice, reduction, merge, and widening policy versions;
- config and rule/options digests when behavior changes;
- language lifecycle setup;
- extension manifest/source/artifact/Cargo.lock/toolchain/feature digests;
- context policy and budget;
- call graph target set;
- substitution/place schema version;
- unknown/havoc policy version;
- dependent summary digests.

For recursive SCCs, cache the converged SCC summary identity, not unstable
intermediate summaries.
