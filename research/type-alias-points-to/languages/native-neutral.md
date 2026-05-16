# Native / Language-Neutral Report

## State Of The Art References

This track inspected language-neutral and compiler-oriented systems:

- LLVM AliasAnalysis and MemorySSA;
- SVF pointer/value-flow analysis;
- Rust borrow checker and Polonius;
- rust-analyzer incremental semantic database;
- Souffle/Datalog;
- Joern/Code Property Graph.

These systems do not map directly to polint's product, but they define important architecture lessons.

## LLVM AliasAnalysis

LLVM's alias analysis is a provider interface, not a single result. Consumers ask alias queries and get answers such as `NoAlias`, `MayAlias`, `PartialAlias`, or `MustAlias`.

Polint lesson:

- alias should be query-driven;
- multiple providers can answer;
- conservative fallback is necessary;
- rule APIs should get evidence/precision, not raw internals.

## LLVM MemorySSA

MemorySSA represents memory definitions, uses, and phis in a sparse intraprocedural graph. It supports clobber queries and relies on alias analysis.

Polint lesson:

- sparse flow-sensitive refinement should come after local CFG/place/value facts;
- memory-like facts should be represented in a way that future MemorySSA/SVFG can use;
- dense flow-sensitive points-to is not the first scalable implementation.

## SVF

SVF builds pointer analysis and sparse value-flow graphs over LLVM IR, using memory SSA and points-to information to build SVFGs.

Key inspected paths:

- `SVF/svf/lib/MSSA/SVFGBuilder.cpp`
- `SVF/svf/lib/MSSA/MemSSA.cpp`
- `SVF/svf/lib/MSSA/MemRegion.cpp`
- `SVF/svf/lib/Graphs/ConsG.cpp`
- `SVF/svf/lib/CFL/CFLVF.cpp`

Polint lesson:

- points-to and sparse value-flow should be separate but connected;
- memory regions/field sensitivity must be explicit;
- flow-sensitive precision is feasible when sparse.

## Rust Borrow Checker / Polonius

Rust borrow checking is not a general alias analysis for arbitrary languages. It is an ownership/loan/lifetime analysis for Rust MIR. Polonius represents borrow-checking facts relationally.

Polint lesson:

- ownership facts can prove strong no-alias/no-mutation results in languages with ownership semantics;
- relational sub-engines are useful for recursive fact computation;
- do not mistake borrow checking for a general cross-language points-to solver.

## rust-analyzer

rust-analyzer is relevant for query architecture, incremental computation, stable IDs, and semantic database design.

Polint lesson:

- cache and invalidation design matters as much as solver choice;
- facts should have stable identity and fine enough invalidation;
- keep public APIs separate from internal compiler/query structures.

## Souffle / Datalog

Souffle is important because many static analyses are naturally relational. Doop demonstrates this for Java.

Polint lesson:

- a relational/fixpoint sub-engine may be useful internally;
- extension facts can be validated as relation inserts;
- do not expose Datalog as the main user/rule API unless there is a deliberate advanced mode.

## Joern / Code Property Graph

Joern's CPG combines AST, CFG, call graph, data-flow, and semantic layers into one queryable graph.

Polint lesson:

- unified graph identity is valuable;
- public query ergonomics matter;
- raw graph APIs can become too broad and unstable for polint's SDK discipline.

## Neutral Architecture Recommendation

```text
typed facts as stable product surface
  + internal relation/fixpoint engine
  + provider-stack alias queries
  + sparse memory/value-flow future
  + query/cache/provenance discipline
```

Do not choose between compiler-style and query-style architecture. Use typed facts as the product API, relation/fixpoint internals where they help, and provider stacks for precision escalation.
