# Research Analysis: Algorithms, Accuracy, Complexity

## What Abstract Interpretation Gives polint

Abstract interpretation gives a disciplined way to compute facts that are true
for all executions represented by an abstraction. In polint, the goal is not
formal verification of arbitrary programs. The goal is to compute high-value
facts for repo-local rules and agent-authored analysis extensions:

- "this value is definitely non-nil here";
- "this string is one of these literals";
- "this builder can reach `build` only after required methods";
- "this branch refines a discriminated union";
- "this resource may escape without close";
- "this range proves the array index safe or unsafe";
- "this function summary preserves taint from arg 0 to return";
- "this fact is unknown because an extension/model is missing."

The key is to use abstract interpretation where it gives leverage, while
labeling precision honestly.

## Core Theory

The basic shape is:

```text
concrete collecting semantics
  -> abstraction function
  -> abstract transfer functions
  -> fixpoint over CFG/call graph
  -> conservative facts
```

For a domain `D`, the solver computes a post-fixpoint:

```text
F#(state) <= state
```

The domain must make loops converge through finite height, widening, budgets, or
domain-specific truncation. Without that, "most capable" becomes "unbounded."

## Main Algorithms

### Worklist Fixpoint

Useful for intraprocedural domains and local summaries.

Accuracy:

- precise enough for local nullness, constants, truthiness, initialization;
- loses path correlations at joins unless predicates/partitions preserve them;
- depends heavily on edge-specific branch transfer.

Complexity:

```text
O(iterations * (blocks + edges) * transfer_cost)
```

For bitset domains, `transfer_cost` is often `O(bits / word)`.

### Widening And Narrowing

Needed for infinite-height domains such as intervals, string sets with growing
templates, and relational numeric constraints.

Policy:

- widen only at loop/SCC headers;
- delay widening for a few iterations;
- use thresholds from literals and guards;
- log precision loss;
- optionally narrow after a stable post-fixpoint.

Bad widening can hide the facts users care about. For example, widening `i` to
`[-inf, +inf]` after one loop iteration destroys array-index checks. Thresholds
from conditions like `i < len(xs)` are strongly recommended for useful
precision; without them the result remains conservative but often too imprecise
for the rules polint wants to support.

### Reduced Products

Reduced products are the core design choice for polint.

Example:

```text
Constants: x = "admin"
Truthiness: x is truthy
StringValues: len(x) = 5
Shape: request.role is present
```

Each domain is cheap alone. Reductions exchange facts. This is more extensible
than a single mega-domain because agent extensions can add new reductions or
models without rewriting all domains.

Risk:

- reductions can become non-deterministic or non-terminating if cyclic;
- they must be fuel-bounded, ordered, and versioned.

### Disjunctive Domains And Trace Partitioning

Joins lose facts like:

```text
if role == "admin":
    allowed = true
else:
    allowed = false
```

After a naive join, `role` and `allowed` correlation disappears. Trace
partitioning keeps selected branch histories. It is powerful but explosive.

Use only selected partitions:

- nil/null checks;
- `err != nil` in Go;
- discriminant fields in TS/JS/Python;
- `typeof` and `isinstance`;
- selected framework guard calls;
- rule-requested predicates.

Evict partitions deterministically by priority and evidence value.

### Relational Numeric Domains

Intervals represent `x in [l, u]`. They cannot express `x < y`.

DBM/zones express constraints like `x - y <= c`.

Octagons express `+/-x +/-y <= c`.

Polyhedra express arbitrary linear inequalities.

Complexity ladder:

| Domain | Memory | Typical Operations | Recommendation |
|---|---:|---:|---|
| Intervals | `O(n)` | cheap | default |
| Congruence | `O(n)` | cheap/moderate | default or P1 |
| DBM/zones | `O(n^2)` | closure often `O(n^3)` | packed only |
| Octagons | `O(n^2)` | closure often `O(n^3)` | first relational domain |
| Polyhedra | high | exponential worst case | optional expert mode |

The mistake would be to build polyhedra first. The right path is intervals,
then congruence, then packed octagons selected by guards/rules.

### Abstract Garbage Collection / Scoped Forgetting

Stale abstract heap/location facts reduce precision. Before joins and widening,
forget facts for:

- dead locals;
- overwritten places;
- escaped objects;
- unmodeled dynamic writes;
- unknown calls;
- out-of-budget path partitions.

This is not only performance cleanup. It prevents stale facts from producing
misleading diagnostics.

### Summary-Based Interprocedural Analysis

Whole-program analysis is expensive and fragile. Summaries are the scaling
boundary:

```text
function body -> domain facts -> projected summary -> call transfer
```

Summaries should include:

- preconditions;
- postconditions;
- return facts;
- invalidated places;
- resource/typestate transitions;
- TITO/value-flow edges;
- external effects;
- unknowns and precision labels.

Recursive SCCs need summary fixpoints with widening. Unknown callees should
havoc relevant places or return domain top, with an explicit unresolved fact.

## Language-Specific Accuracy Constraints

### Go

Go is comparatively favorable:

- `nil` semantics are explicit;
- package/type info from official tooling is strong;
- `panic`, `defer`, goroutines, interface dispatch, build tags, and generated
  code still require explicit lifecycle facts.

Start with local nilness, constants, intervals, and resource/typestate. Use
official Go package metadata when semantic setup is available.

### TypeScript / JavaScript

TS/JS precision is mostly local narrowing:

- `typeof`;
- equality to `null`/`undefined`;
- truthiness;
- discriminant properties;
- `in` checks;
- literal unions;
- optional chaining;
- object shape exactness until dynamic writes/escape.

TypeScript annotations are not runtime facts. Treat them as declared type facts,
not as proof of JS runtime shape.

### Python

Python needs:

- `None` and singleton literal narrowing;
- `isinstance`, `callable`, `TypeGuard`, `TypeIs`;
- `TypedDict` and tag narrowing;
- decorators and dataclass transforms;
- static and string-literal imports;
- heuristic exception facts.

Do not execute imports. Use Python metadata for environment discovery only.

### JVM / Java

For Java, source-level nullness and typestate should use classpath, annotation,
and JDK metadata. Bytecode-only facts can use verifier-style frame domains.

Avoid reimplementing javac overload/generic inference as a first step. Use
official metadata where it is the compatibility source.

### Rust

Even before Rust language support, rustc's MIR dataflow is the strongest native
design reference inspected here:

- `JoinSemiLattice`;
- edge-specific effects;
- call-return effects distinct from unwind;
- move path trees;
- bitset domains;
- result cursors.

polint should copy the kernel shape, not rustc internals.

## Rejected Paths

| Path | Why Rejected For First Implementation |
|---|---|
| Full symbolic execution first | Too expensive; path explosion; hard to make multi-language and extension-safe. |
| Global Datalog engine as core | Good for dataflow/query relations, weaker fit for numeric/resource/path-sensitive domains. |
| Global octagon/polyhedra engine | High cost and too narrow as a first semantic layer. |
| External analyzer dependency | Conflicts with native implementation goal and makes extension/cache/provenance harder. |
| Raw AST-domain state | Parser-specific, brittle, hard to summarize or cache. |
| Public raw lattice SDK | Freezes internals and leaks complexity to rule authors. |

## Final Research Judgment

The right design is conservative in the kernel and ambitious in extension power:

- deterministic native solver;
- small domains first;
- summary projection early;
- explicit precision and unknowns;
- packed relational precision later;
- agent-authored Rust extensions through law-checked products.

That path gives polint maximum long-term capability without building into the
wrong corner early.
