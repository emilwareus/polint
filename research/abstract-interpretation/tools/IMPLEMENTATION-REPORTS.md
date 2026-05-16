# Implementation Reports

## Infer / Pulse

Infer's `absint` framework exposes abstract domains with `leq`, `join`, and
`widen`, and separates transfer functions from summaries. Pulse uses an
abductive state with pre/post heap state, stack, address attributes, path
conditions, TOPL typestate, skipped calls, and transitive call information.

Lessons for polint:

- summaries are first-class analysis outputs;
- latent diagnostics can represent precondition-dependent issues;
- resource/typestate fits naturally as finite state attached to abstract
  addresses;
- model unknown calls explicitly.

Do not copy Pulse wholesale. Its separation-logic machinery is too specialized
for polint's first multi-language domain layer.

## Clang Static Analyzer

Clang Static Analyzer uses path-sensitive symbolic execution over an
ExplodedGraph. `ProgramState` is an immutable-style combination of environment,
store, generic data map, and constraints. Checkers subscribe to events and store
checker-specific state in the generic data map.

Lessons for polint:

- immutable snapshots make branching and explanation easier;
- checker/domain event hooks are useful;
- range constraints and resource checkers are good examples of small state
  domains;
- path sensitivity must be budgeted.

Risk:

- full symbolic execution as the default would be too expensive and language
  specific.

## rustc MIR Dataflow

rustc's framework is the strongest inspected shape for polint's native solver:

- domain is a `JoinSemiLattice`;
- analysis supplies bottom, start state, transfer functions, edge effects;
- solver computes a fixpoint;
- result cursors query facts at locations.

Initializedness over move paths and const propagation over flat values are
directly relevant. The important abstraction is interned places plus bitset
domains, not Rust-specific ownership.

## Checker Framework / NullAway

Checker Framework represents stores mapping expressions/access paths to
abstract values. Transfer functions produce regular and conditional stores.
NullAway is a secondary reference for a fast, pragmatic access-path nullness
model; it was not cloned in this research folder.

Lessons for polint:

- source-level nullness and typestate should be store-based;
- called-methods and must-call domains are strong typestate/resource templates;
- annotations/models are a valid source of declared external facts;
- exactness needs honest labels.

## TypeScript / Flow / Pyright

TypeScript and Pyright use flow nodes and on-demand traversal to compute
narrowed types. Flow is a secondary/contextual reference for richer refinement
invalidation and object exactness.

Lessons for polint:

- local narrowing is the high-value default;
- nullish/truthiness/literal/discriminant/property facts matter more than global
  symbolic execution for day-to-day rules;
- dynamic writes and calls must invalidate refinements;
- model exact and inexact object shapes.

## Python Tools: Pyright, Pyre, mypy, Ty

Pyright is a strong reference for Python narrowing breadth based on source and
tests. Ty/Ruff is the closest Rust-native source architecture. mypy's binder is
mature and widely used. Pyre/Pysa provides a strong taint/access-path summary
reference.

Lessons for polint:

- use `None` as a singleton type and intersection/negation for narrowing;
- support `TypeGuard` and `TypeIs` distinctly;
- TypedDict tag narrowing is high-value;
- do not execute imports;
- taint summaries are powerful but should be rule-requested.

## Goblint

Goblint has a product of analyses with typed local/global domains and a query
bus. Analyses implement transfer hooks and the solver manages constraints,
side effects, widening, and dependencies.

Lessons for polint:

- analysis-to-analysis queries should be typed;
- side effects must be scheduled and dependency-tracked;
- dynamic product domains are useful but need deterministic merge order.

## Frama-C Eva / Astrée

Eva documentation and contextual Astrée material show high-end abstract
interpretation patterns: cooperative domains, alarms, trace partitioning,
reduced products, numeric domains, and careful scope claims. Eva documentation
was downloaded; Astrée is contextual here, not a cloned implementation.

Lessons for polint:

- domain-specific abstractions are the precision engine;
- alarms/diagnostics should be first-class analysis outputs;
- trace partitioning is valuable but scoped;
- strong soundness claims require narrow language/setup assumptions.

## Apron / ELINA / IKOS

Apron provides a common API over intervals, octagons, polyhedra, and related
domains. ELINA optimizes octagons/polyhedra. IKOS provides clean abstract-domain
interfaces and fixpoint iterators.

Lessons for polint:

- common domain APIs are possible;
- intervals and congruence are cheap enough early;
- octagons are the recommended first relational numeric candidate for polint
  because they offer useful pairwise constraints with known costs;
- polyhedra are too expensive for default mode;
- variable packing is required for polint's recommended default use of
  relational domains.

## CodeQL / Semgrep

CodeQL's model packs and dataflow configuration are excellent references for
declarative extension models. Semgrep's taint vocabulary is excellent for rule
UX.

Lessons for polint:

- declarative sources/sinks/sanitizers/barriers should exist;
- suppressive facts need stricter review;
- graph/dataflow queries can be declarative while numeric/resource domains stay
  native Rust;
- precision controls must be visible to users.
