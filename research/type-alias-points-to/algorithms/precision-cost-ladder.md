# Precision And Cost Ladder

This is the recommended escalation ladder for polint. Each tier should be separately requestable, cacheable, and measurable.

## Tier 0: Syntax, Symbols, References, Imports

**Answers:** direct names, syntactic calls, imports, exports, definitions.

**Cost:** near linear in source size.

**Precision:** exact for syntax; not enough for dynamic dispatch.

**Use:** baseline for every other layer.

## Tier 1: Declared And Resolved Type Facts

**Answers:** declared parameter/variable/field types, resolved nominal symbols, method sets, structural shape candidates.

**Cost:** near linear for many codebases, higher for generics/overloads/unions/structural typing.

**Precision:** high where annotations and language semantics are explicit.

**Use:** call pruning, diagnostics, local rule facts.

## Tier 2: Local Flow And Narrowing

**Answers:** narrowed types at CFG nodes, nullness/truthiness, literal/discriminant facts, definite assignment/boundness.

**Cost:** `O(CFG edges * lattice height)` with caps/widening.

**Precision:** high for local guards, weak across function boundaries without summaries.

**Use:** most high-value policy rules and direct call-target improvements.

## Tier 3: Value Object Facts

**Answers:** function/class/module object values, allocation tokens, object literal tokens, closure captures, simple container tokens.

**Cost:** local pass plus summary propagation.

**Precision:** good for callback/framework registration and factory patterns.

**Use:** call graph expansion and framework models.

## Tier 4: Summary Propagation

**Answers:** parameter-return flow, receiver effects, framework API behavior, builtins, source/sink/barrier semantics.

**Cost:** depends on summary graph; manageable with caching and SCC fixed points.

**Precision:** high if summaries are accurate; extension-provided summaries can be excellent.

**Use:** interprocedural data flow and call graph precision.

## Tier 5: Flow-Insensitive Points-To

**Answers:** may-points-to sets for places/values and function objects.

**Cost:** worst-case high; practical with bitsets/SCC/deltas/budgets.

**Precision:** conservative; field sensitivity improves object-property precision but increases size.

**Use:** alias queries, dynamic dispatch, callbacks, container/property flow.

## Tier 6: Selective Context Sensitivity

**Answers:** points-to/call/data-flow separated by receiver object/type or call-string.

**Cost:** can multiply graph size quickly.

**Precision:** useful in OO/framework-heavy code.

**Use:** opt-in for high-value queries or agent-extended models.

## Tier 7: Sparse Flow-Sensitive Refinement

**Answers:** order-sensitive alias/value-flow for selected memory locations.

**Cost:** high engineering complexity; query cost budgeted.

**Precision:** strong for mutation-heavy code where flow-insensitive points-to is too coarse.

**Use:** security/data-flow rules, mutation protocols, resource lifecycle.

## Tier 8: Path-Sensitive Refinement

**Answers:** feasible-path constrained facts.

**Cost:** potentially exponential without strong pruning.

**Precision:** strongest for narrow questions.

**Use:** last-mile evidence and false-positive reduction for selected diagnostics.

## Recommendation

Ship tiers 0-5 first. Design IDs/cache/provenance so tiers 6-8 can be added later without rewriting the fact model.
