# Standard: How To Talk About Abstract Domains

This standard keeps the abstract-interpretation research, implementation notes,
and future rule-facing docs aligned.

## Abstract Domain Object

An abstract domain is not just a value type. In polint it should be a typed,
versioned analysis component:

```text
AbstractDomain =
  domain id and version
  + semantic subject
  + lattice operations
  + transfer/refinement operations
  + widening/narrowing policy
  + serialization/cache identity
  + provenance and precision labels
  + validation record
```

The subject can be a function body, CFG location, place, expression, call site,
abstract allocation, package, module, or summary.

## Required Operations

| Operation | Meaning |
|---|---|
| `bottom` | Impossible state or no information, depending on domain polarity. Must be documented. |
| `top` | Conservative unknown state. Unknown is not empty. |
| `leq(a, b)` | Precision/order relation: `a` is at least as precise as or implies `b`. |
| `join(a, b)` | Least practical upper bound for merging paths. |
| `join_into(dst, src)` | Mutating merge used by solvers. Returns `Changed` so solver code does not rely on handwritten `leq` polarity checks. |
| `meet(a, b)` | Optional filter/intersection for guards and refinement. |
| `widen(prev, next, site)` | Termination operator at loops/SCCs. Must over-approximate. |
| `narrow(prev, next, site)` | Optional post-widening refinement. Must not invent unsound precision. |
| `assign(place, expr)` | Transfer for assignments and binding updates. |
| `assume(predicate, sense)` | Branch/path refinement. |
| `forget(place_or_region)` | Drop facts invalidated by mutation, escape, call, scope exit, or budget. |
| `project(summary_key)` | Extract summary or public fact view. |

Domains that cannot implement one of these operations must expose the missing
operation as unsupported rather than silently approximating with an unrelated
behavior.

Solver code should prefer `join_into` or an equivalent `JoinResult` API over
open-coded comparisons such as `if !joined.leq(old)`. Different domains will
still document their order, but the mutation API is the guardrail that keeps
fixpoint scheduling correct when a domain uses a dual order, a bitset
representation, or a compact canonical form.

## Precision And Status

Every exported domain fact must carry both a status and a precision label.

| Status | Meaning |
|---|---|
| `Complete` | Provider completed within the selected domain and budget. |
| `Incomplete` | Provider produced useful facts but not complete domain coverage. |
| `SetupMissing` | Required toolchain, dependency metadata, type info, module root, or classpath was missing. |
| `Unsupported` | Construct is outside the current domain semantics. |
| `Ambiguous` | Multiple plausible interpretations remain. |
| `BudgetExceeded` | Path/domain/context budget forced widening, truncation, or top. |
| `Rejected` | Extension/model failed validation. |

| Precision | Meaning |
|---|---|
| `ExactSemantic` | Derived from official language semantics for the modeled fragment. |
| `ExactLocal` | Exact for one body under declared assumptions. |
| `SummaryBased` | Depends on callee/package/framework summaries. |
| `FrameworkModeled` | Depends on lifecycle/entrypoint/model overlays. |
| `Conservative` | Safe over-approximation but not precise. |
| `Heuristic` | Useful approximation that cannot claim exactness. |
| `DeclaredExternal` | Provided by extension/model metadata. |
| `UnknownTop` | Conservative top value. |

## Domain Families

| Domain | Payload Shape |
|---|---|
| `Reachability` | reachable, unreachable, ambiguous, path condition evidence. |
| `Nilness` | definitely nil/null/None/undefined, definitely non-nullish, maybe. |
| `Truthiness` | definitely truthy, definitely falsy, maybe. |
| `Constants` | bottom, capped literal set, singleton, top. |
| `StringValues` | capped literals, template parts, prefix/suffix, length interval, top. |
| `NumericRanges` | intervals, optional congruence, optional packed DBM/octagon relations. |
| `Initializedness` | maybe initialized, maybe uninitialized, definitely initialized queries. |
| `Shape` | object/record/TypedDict/class/property presence and exactness. |
| `Typestate` | finite state machine over abstract objects/resources. |
| `PathPredicates` | guarded predicates, type tests, branch correlations, trace partitions. |
| `Permission` | borrow/loan/ownership/alias permission facts where language semantics support it. |

## Report Template For Implementations

Every tool report should answer:

1. What semantic subject is analyzed?
2. What abstract domains are represented?
3. What is the lattice order?
4. How are transfer functions expressed?
5. How are guards and branch-sensitive facts represented?
6. How are loops, recursion, and fixpoints handled?
7. Where does widening/narrowing happen?
8. How are summaries or interprocedural facts represented?
9. How are unknown calls, dynamic language features, framework behavior, and missing setup handled?
10. What are the accuracy/cost tradeoffs?
11. What should polint copy, adapt, or reject?

## Pseudo-Code Conventions

Pseudo-code is Python-ish unless Rust types are important:

```python
def analyze_function(cfg, domain):
    state = domain.bottom()
    for block in cfg.worklist_order():
        state = transfer_block(block, state)
    return state.project_summary()
```

Use `top` for conservative unknown, not for failure. Use `unsupported` for
constructs that the domain does not model.
