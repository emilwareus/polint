# Final Report: Abstract Interpretation Domains

## Executive Synthesis

Abstract interpretation is the right foundation for polint's higher-value
semantic facts, but only if it is implemented as a **domain kernel**, not as a
single hardcoded analysis.

The inspected primary implementations and secondary/contextual references point
to the same pattern from different directions:

- Cousot-style abstract interpretation gives the fixpoint/lattice foundation.
- rustc MIR dataflow shows a small, deterministic, Rust-native domain interface
  with precise edge effects and per-location result cursors.
- TypeScript, Pyright, mypy, and Ty/Ruff show that most day-to-day precision in
  typed/dynamic frontend languages comes from fast local narrowing domains.
  Flow is a secondary reference for richer refinement invalidation and object
  exactness.
- Checker Framework, Clang Static Analyzer, Infer/Pulse, and Goblint show that
  resource, nullness, typestate, and path-sensitive facts are most useful when
  the analyzer has explicit stores, transfer hooks, and summaries. NullAway is a
  secondary reference for pragmatic Java nullness.
- Apron, ELINA, IKOS, and Eva show the power and cost of relational numeric
  domains, trace partitioning, and domain cooperation. Astrée is a contextual
  high-end reference, not a cloned implementation in this research folder.

The product-specific conclusion is important: polint should not optimize for a
sealed universal analyzer that tries to infer every project convention by
itself. It should provide strong native defaults and a high-capability extension
surface where AI agents can add repo-specific guards, typestate machines,
summaries, framework models, and domain validators in Rust.

## Recommendation

Implement a native reduced-product abstract-domain layer:

```text
CFG + semantic operations + places + types + call summaries
  -> deterministic worklist / SCC solver
  -> small abstract domains over a hybrid core/extension product
  -> bounded reductions between domains
  -> summary projection
  -> typed SDK fact views
  -> agent-authored extension products through validated sinks
```

Do not build:

- a whole-program symbolic executor as the first step;
- a global polyhedra/octagon engine for every variable;
- a Datalog-only analysis substrate;
- an extension API that mutates core facts directly;
- a public SDK that exposes raw internal lattice states.

## Domain Priority

The first domains should be chosen by value-to-cost ratio:

| Priority | Domain | Why It Comes First |
|---|---|---|
| P0 | Reachability and control outcomes | Needed by every other domain and diagnostic. |
| P0 | Nil/null/None/undefined/nullish | High rule value, cheap lattice, strong TS/Go/Python/Java precedent. |
| P0 | Truthiness | Required for TS/JS/Python narrowing and guard modeling. |
| P0 | Constants and literal sets | Feeds string/API/route/framework/security rules. |
| P1 | String values/templates/length | High policy value for routes, SQL fragments, env keys, feature flags. |
| P1 | Initializedness/definite assignment | Reusable across Go, TS/JS, Python, JVM, and future Rust. |
| P1 | Intervals | Cheap numeric range facts for bounds and loop reasoning. |
| P1 | Shape/property/TypedDict facts | Essential for JS/Python/framework-heavy code. |
| P1 | Typestate/resource | Enables project-specific lifecycle rules and leak checks. |
| P2 | Congruence and packed octagons | Useful but only for selected variable packs and rule-requested facts. |
| P2 | Trace partitions/path predicates | Powerful but must be budgeted and selected. |
| P3 | Polyhedra/path focusing/SMT | Precision mode, not default CI behavior. |

## Accuracy And Complexity

Abstract interpretation has a predictable precision/cost ladder:

- Flat constants, nilness, truthiness, initializedness, and finite typestate are
  near-linear over CFG size when implemented as bitsets or capped maps.
- Intervals are cheap enough for default use but lose relational facts.
- Congruence helps parity/modulo and combines well with intervals.
- DBM/zones and octagons need `O(n^2)` memory and often `O(n^3)` closure, so
  they should be packed over selected variables.
- Polyhedra can be exponentially expensive and should remain an optional expert
  backend.
- Disjunctive domains and trace partitioning recover path precision but can
  explode without explicit budgets and merge policies.

The practical design is therefore a **precision ladder**:

```text
default local domains
  -> summary-based interprocedural facts
  -> selected partitions
  -> packed relational domains
  -> diagnostic-focused refinement
```

## Product Implication: Agent-Extensible Domains

The agent-era product thesis changes the implementation:

- Unknown facts should become integration tasks, not hidden false negatives.
- Extensions should add repo-specific guards, summaries, typestate machines,
  framework refinements, and domain validators.
- Extension facts must carry provenance, precision, cache digests, and
  validation status.
- Suppressive facts such as sanitizers/barriers need stricter review than
  additive facts such as sources/sinks or extra summaries.

The extension surface should be more capable than typical linter configuration,
but the kernel must stay in control of scheduling, merging, invalidation, and
diagnostic reporting.

## Key Design Choices

### 1. Semantic Operation IR Before Domains

Do not run domains directly on parser ASTs. Lower language adapters into a small
semantic operation layer:

```text
assign, load, store, call, return, throw/panic/reject,
branch, switch, await, yield, acquire, release, defer/finally,
field/property read, field/property write, index read/write
```

AST nodes remain evidence and source-span anchors. Domains should use stable
operation, place, symbol, and summary IDs.

### 2. Reduced Product, Not One Value Lattice

Combine small domains:

```text
State =
  Reachability
  x Nilness
  x Truthiness
  x Constants
  x StringValues
  x NumericRanges
  x Shape
  x Typestate
  x PathPredicates
```

Use bounded reductions. Example: `Constant(None)` refines `Nilness`, string
literal length refines `NumericRanges`, shape discriminant refines union
alternatives, and typestate transition summaries refine resource obligations.

### 3. Summary Projection Is Required Early

Without summaries, domains either stay local or become whole-program guesses.
Each domain needs a summary projection:

```text
requires, ensures, modifies, invalidates, returns, throws,
guard refinements, typestate transitions, TITO/value flows,
unknowns, precision, provenance
```

Summaries are the bridge between local abstract interpretation, call graphs,
data flow, alias facts, and extension models.

The summary key/store/SCC scheduler belongs to the effects-summary kernel. The
abstract-domain layer contributes payload algebra, projection, caller-place
substitution, widening policy, and call-application transfer.

### 4. Explicit Unknowns

Unsupported semantics must not disappear. Emit facts such as:

- dynamic property write invalidated shape precision;
- call target unresolved, return domain top;
- external summary missing;
- loop widened at block X;
- extension model rejected due non-monotone transfer;
- setup missing for official language metadata.

Rules and agents can decide whether to accept the uncertainty or write a model.

## Implementation Path

1. Add the internal `analysis_domain` kernel.
2. Add the shared MIR-like semantic operation layer and interned `PlaceId`.
3. Implement P0 domains: reachability, nilness/nullish, truthiness, constants.
4. Add `ResultsCursor`-style query APIs for CFG locations.
5. Add direct/syntactic call facts and context-insensitive direct summaries for
   P0 domains.
6. Add minimal dependency digests/cache invalidation before summary SCCs.
7. Add model-extension sinks for guard and summary facts.
8. Add string, initializedness, interval, and shape domains.
9. Add typestate/resource domain and extension-driven state machines.
10. Add law/property tests and inline fact fixtures before exposing SDK views.
11. Expose typed SDK views only when facts are useful and documented.

## Non-Goals For The First Vertical Slice

- No global relational numeric engine by default.
- No full Java/Python/TypeScript type checker clone.
- No proof-grade soundness claims across dynamic languages.
- No extension mutation of core stores.
- No public raw lattice API.

## Final Position

This research supports building polint's abstract interpretation layer now, but
with strict boundaries: native, typed, deterministic, summary-aware,
extension-safe, and honest about precision. This is the right shape for a
max-capability static analysis engine whose most powerful integrations can be
authored by AI agents as repo-local Rust code.
