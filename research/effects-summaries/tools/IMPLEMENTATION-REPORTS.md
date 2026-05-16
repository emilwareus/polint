# Implementation Reports

These reports use the schema from `STANDARD.md`.

## CodeQL

Subject summarized:

- library callables and framework APIs;
- query-defined `SummarizedCallable` ranges;
- data-extension model rows.

Domain:

- data-flow and taint summaries;
- source/sink/barrier/guard models;
- access-path based flow from input to output.

Mechanism:

- `summaryModel` data extensions add tuples for package/member/path input,
  output, and kind;
- QL classes can implement `propagatesFlow`;
- `provenance` and `isExact` are visible in the summary predicate shape.

Strengths:

- best declarative access-path model;
- model packs are versionable and shareable;
- separates value-preserving and taint-preserving flow;
- supports barriers and guards in modern models-as-data.

Weaknesses:

- string access paths are ergonomic but should not be polint's internal core;
- dynamic JS often needs custom QL;
- model completeness is library/framework dependent.

polint action:

- copy access-path summary declarations and model provenance;
- implement typed Rust data structures internally;
- allow direct Rust model providers for cases data declarations cannot express.

## Pysa/Pyre

Subject summarized:

- Python callables.

Domain:

- forward source generations;
- backward sinks;
- taint-in-taint-out;
- parameter sources;
- sanitizers;
- breadcrumbs/features;
- modes and generated models.

Mechanism:

- model file declarations and inferred model states;
- forward/backward analysis over CFG and call graph;
- global fixpoint until models stabilize;
- access-path trees with widening/broadening.

Strengths:

- closest product fit for a summary-first taint engine;
- excellent debugging vocabulary;
- explicit model verification.

Weaknesses:

- Python-specific;
- depends heavily on type/call graph precision;
- broadening and obscure calls reduce precision.

polint action:

- copy model shape: sources, sinks, TITO, sanitizers, features;
- preserve broadening and obscure-call markers as precision metadata;
- make model validation mandatory for agent-authored summaries.

## Infer/Pulse

Subject summarized:

- procedures.

Domain:

- heap/resource/invalid-access summaries;
- pre/post execution states;
- non-disjunctive transitive data;
- latent issues;
- skipped/unknown calls.

Mechanism:

- summary database keyed by procedure and analysis request;
- Pulse pre/post states over stack, heap, and address attributes;
- compositional analysis stores callee summaries for callers.

Strengths:

- strongest heap/effect vocabulary;
- good model for resource leaks and invalidation;
- explicit summary metadata/dependencies.

Weaknesses:

- path-sensitive symbolic analysis is a large implementation;
- manifest-bug objective differs from polint's policy-rule objective.

polint action:

- copy address/resource invalidation vocabulary;
- do not implement full Pulse first;
- build simpler resource/effect summaries and leave room for later symbolic
  domains.

## LLVM MemoryEffects And MLIR SideEffects

Subject summarized:

- functions/calls/instructions/operations.

Domain:

- memory mod/ref over location kinds;
- resource-scoped effects such as read/write/allocate/free.

Mechanism:

- LLVM `MemoryEffects` encodes mod/ref bits per memory-location class;
- MLIR `EffectInstance` combines effect, resource, optional value/symbol, and
  stage/full-region metadata.

Strengths:

- compact, composable lattice;
- clear unknown/no-effect/read/write distinctions;
- resource hierarchy and disjointness are explicit.

Weaknesses:

- low-level IR memory locations are not source-policy resources;
- alias analysis remains separate.

polint action:

- use access kind x resource kind as the first memory/effect summary lattice;
- keep alias refinement separate.

## Go analysis/x/tools

Subject summarized:

- packages, objects, and analyzer facts.

Domain:

- modular facts;
- SSA local analysis;
- imported/exported facts;
- analyzer dependency results.

Mechanism:

- analyzers declare `Requires` and `FactTypes`;
- facts are object/package-scoped and imported/exported;
- `buildssa` provides SSA for local summaries.

Strengths:

- official Go tooling;
- good modular fact concept;
- stable compatibility path through `go/packages`.

Weaknesses:

- Go facts are analyzer-private and gob-serialized;
- not a cross-language summary substrate by itself.

polint action:

- use Go toolchain/x/tools as provider input;
- normalize facts into polint-owned summary payloads.

## WALA

Subject summarized:

- JVM methods, including synthetic/library methods.

Domain:

- SSA-like method summaries;
- bypass methods;
- mod/ref and call target policies.

Mechanism:

- `MethodSummary` stores SSA instructions and constants;
- `XMLMethodSummaryReader` loads model summaries for calls, returns, puts,
  gets, throws, allocations, constants;
- target selectors can redirect calls to summaries.

Strengths:

- library summaries are executable synthetic bodies;
- classpath/library boundaries are explicit.

Weaknesses:

- JVM-specific;
- XML models are powerful but verbose;
- precision still depends on points-to/call graph/reflection.

polint action:

- copy the idea of synthetic callable summaries;
- keep model syntax Rust-typed or generated, not XML-like.

## Soot

Subject summarized:

- JVM methods and statements.

Domain:

- read/write sets over fields/globals/array elements;
- transitive targets through call graph.

Mechanism:

- `SideEffectAnalysis` computes non-transitive read/write sets from points-to;
- call-site read/write sets union callee non-transitive sets over transitive
  targets;
- native calls are flagged separately.

Strengths:

- simple and concrete read/write summary model;
- shows dependency on points-to and call graph precision.

Weaknesses:

- coarse transitive union can be imprecise;
- native and reflection require models.

polint action:

- start with simple read/write summaries;
- expose native/unresolved as explicit summary status.

## OPAL

Subject summarized:

- JVM methods/classes/fields as entities in a property store.

Domain:

- purity;
- allocation freeness;
- thrown exceptions;
- static data usage;
- field access;
- many other properties.

Mechanism:

- FPCF property store schedules eager/lazy/collaborative computations;
- analyses derive properties with upper/lower bounds and dependencies;
- fixed-point updates propagate through the property store.

Strengths:

- best reference for separate effect properties;
- scheduling model maps well to polint's typed provider DAG.

Weaknesses:

- JVM-specific implementation;
- property ecosystem is large.

polint action:

- implement summary domains as separate properties, not a monolithic effect
  field.

## Heros And PhASAR

Subject summarized:

- IFDS/IDE supergraph edges and library functions.

Domain:

- finite data-flow facts;
- IDE edge functions;
- parameter-to-return library summaries.

Mechanism:

- normal/call/return/call-to-return flow functions;
- PhASAR provides `getSummaryFlowFunction` and `getSummaryEdgeFunction`;
- `FunctionDataFlowFacts` maps function keys and parameter indexes to output
  facts.

Strengths:

- clear IFDS/IDE abstraction boundary;
- summary hooks show how to bypass expensive callee traversal.

Weaknesses:

- IFDS/IDE does not fit every effect domain;
- library summary shape is intentionally simple.

polint action:

- use IFDS/IDE for domains that fit;
- keep other domains under abstract-interpretation-style summaries.

## Joern

Subject summarized:

- CPG methods with custom data-flow semantics.

Domain:

- argument/receiver/return flow pairs.

Mechanism:

- method full-name semantics map positions such as receiver/argument/return;
- regex matching can attach summaries to many methods;
- custom semantics can kill default passthrough behavior.

Strengths:

- compact language-neutral summary declarations;
- useful for security flow queries.

Weaknesses:

- positional model is less rich than CodeQL/Pysa access paths;
- precision depends on CPG quality.

polint action:

- support compact positional shorthand as a generated layer over typed access
  paths.

## Semgrep

Subject summarized:

- rule-local source/sink/sanitizer/propagator patterns.

Domain:

- taint rules over matched ranges and IL dataflow.

Mechanism:

- pattern matching finds source/sink/sanitizer ranges;
- taint analysis maps IL origins to those ranges;
- rule options such as exactness, by-side-effect, labels, and propagators shape
  flow behavior.

Strengths:

- very good user ergonomics;
- simple model concepts map well to user intent.

Weaknesses:

- OSS engine is not a full summary-first global analysis;
- pattern ranges can be ambiguous and duplicate-prone.

polint action:

- copy taint model ergonomics at the SDK/model layer;
- ground implementation in semantic ids and access paths, not only ranges.
