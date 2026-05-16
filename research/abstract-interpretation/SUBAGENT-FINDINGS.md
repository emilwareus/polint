# Subagent Findings

Eight initial research tracks and a second validation pass informed this folder.

## Theory

Key conclusion: use a deterministic fixpoint solver over a reduced product of
small domains. Widen only at loop/SCC headers. Add thresholds and optional
narrowing. Use trace partitioning and relational domains as selected precision
tiers, not default behavior.

Recommended kernel operations:

```text
bottom, top, leq, join, meet/filter, widen, narrow,
assign, assume, forget, rename, project
```

## Production Analyzer Tools

Key conclusion: production analyzers agree on layered states, explicit domains,
summaries, and model hooks.

- Infer/Pulse: summary-based, pre/post, resource/typestate attributes.
- Clang Static Analyzer: path-sensitive state snapshots and event hooks.
- Goblint: product analyses, typed query bus, solver-managed side effects.
- Eva/Astrée: domain cooperation, trace partitioning, carefully scoped soundness.
- IKOS/APRON/ELINA: common abstract-domain APIs and numeric precision ladder.
- CodeQL/Semgrep: declarative model UX and graph/dataflow query surface.

## Rust / rustc

Key conclusion: copy rustc's dataflow architecture, not its language semantics.

Important patterns:

- `JoinSemiLattice`;
- deterministic worklist;
- edge-specific effects;
- call-return effects separate from unwind;
- move-path style interned places;
- bitset domains;
- result cursor/visitor.

## Java / JVM

Key conclusion: build native domain facts but use JVM/JDK/javac metadata as the
semantic authority where needed.

Priority domains:

- nullness;
- called-methods;
- must-call/resource;
- initialization;
- optional bytecode frame facts.

Do not reimplement full javac inference or whole-program object-sensitive
pointer analysis first.

## TypeScript / JavaScript

Key conclusion: TS/JS should start with local narrowing and object/string facts,
not a full TAJS clone.

Priority domains:

- nullish;
- truthiness;
- literals;
- string templates;
- shape/property presence;
- effects/promises/throws;
- optional target sets for call graph precision.

## Python

Key conclusion: use Ty/Ruff-style Rust-native representation and Pyright-style
narrowing coverage.

Priority domains:

- `None`;
- literals;
- union/intersection/negation;
- `TypeGuard` / `TypeIs`;
- TypedDict shapes;
- known decorators;
- coarse exception facts;
- Pysa-style taint summaries later.

## Extension Architecture

Key conclusion: extensions must be registered, law-checked analysis products.
They should not mutate kernel state. They emit typed facts, summaries,
refinements, typestate machines, or diagnostics through sinks.

Required safeguards:

- manifest digests;
- provenance;
- merge policy;
- law tests;
- transfer monotonicity;
- suppressive model review;
- deterministic output;
- panic isolation.

## Evaluation

Key conclusion: use native fixtures for kernel correctness and external
benchmarks for pressure. Measure both default and agent-extended mode.

Important metrics:

- fact inclusion and precision;
- diagnostic precision/recall;
- top rate;
- unsupported rate;
- path precision;
- runtime/memory;
- cache behavior;
- deterministic output;
- extension delta.

## Second-Pass Validation

The validation agents found several issues that were incorporated into the
reports:

- The roadmap needed a tiered bootstrap path. Direct/syntactic call facts come
  before refined call graphs; local domains and direct summaries come before
  global data-flow.
- `ProductState` needed a hybrid design: fixed core slots plus
  registry-backed extension slots.
- Solver pseudo-code needed `join_into`/`JoinResult` semantics instead of
  open-coded `leq` polarity checks.
- Summary payloads needed an explicit algebra, caller-place substitution,
  unknown/havoc behavior, context keys, and cache invalidation.
- The semantic IR needed a MIR-like statement/terminator/edge contract with
  expression facts, allocation identity, destructuring, short-circuiting,
  async/callback/defer/finally/exception edges, and unsupported-semantics
  payloads.
- Extension execution needed isolation guidance; untrusted repo-local Rust
  should run out of process until a stronger safety model exists.
- Some comparative claims were softened or marked as secondary/contextual where
  the local evidence was source-inspection or known industry context rather
  than cloned primary implementation plus downloaded paper.
