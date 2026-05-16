# Research Analysis: Accuracy, Cost, And Design Pressure

## Central Tension

The classic static-analysis tradeoff is precision versus scalability:

```text
more context/path/flow sensitivity -> fewer false positives
more whole-program/dynamic coverage -> fewer false negatives
more sensitivity + coverage -> higher cost and harder invalidation
```

polint adds a different axis:

```text
agent-extensible repo knowledge -> higher precision without universal guessing
```

That should change the architecture. Instead of a maximal generic analyzer that tries to infer everything, polint should build a fact system where missing precision is visible and fixable by repo-local Rust providers.

## Why Type And Value Facts Come Before Points-To

For the languages polint cares about first, many high-value rules do not need full heap aliasing:

- "Was this call guarded by a project-specific authorization check?"
- "Is this value definitely nullable?"
- "Which route handler receives this request object?"
- "Which function object was passed into this callback registration?"
- "Is this module import a test utility or production dependency?"
- "Can this method call target a subtype implementation?"

These questions often need:

- symbol/reference resolution;
- declared/inferred/narrowed types;
- literal values and truthiness;
- function/class/module object values;
- local flow sensitivity;
- framework summaries.

They need points-to only when values pass through object fields, closures, interfaces, containers, dependency injection, or dynamic dispatch. A good type/value layer reduces the points-to search space and improves call graph construction.

## Accuracy By Algorithm Family

| Algorithm family | Accuracy profile | Cost profile | Best polint use |
|---|---|---|---|
| Declared type lookup | High for statically annotated code; weak for untyped dynamic code. | Near linear after semantic index/module graph. | Baseline type facts and call pruning. |
| Flow narrowing | High for local guards and discriminants; path-sensitive only where modeled. | `O(CFG edges * lattice height)`, with caps for unions/loops. | Nullness, `typeof`, `isinstance`, TypeGuard/TypeIs, discriminants. |
| Abstract value interpretation | High for constants/truthiness/literals; weak for arbitrary computation. | Usually linear-ish with small domains, can grow with strings/numbers/containers. | Cheap value facts before points-to. |
| CHA | Sound-ish for closed class hierarchies but very imprecise. | Cheap. | Initial Java/Go/OO dynamic dispatch over-approximation. |
| RTA | More precise than CHA by reachable allocation types; unsound when entrypoints/reflection are missing. | Cheap fixed point. | First dynamic-dispatch refinement where allocation tokens exist. |
| VTA/type propagation | Good middle ground; propagates types/function literals through value-flow graph. | Moderate; graph propagation over type nodes. | Go/JVM method/function target refinement; TS/Python callback heuristics. |
| Andersen | More precise inclusion-based points-to; field sensitivity possible. | Worst-case cubic; practical with bitsets/SCC/deltas. | Bounded internal points-to provider. |
| Steensgaard | Very scalable but merges too much. | Near linear. | Emergency coarse mode or pre-pass, not main precision mode. |
| IFDS/IDE | Precise for finite distributive data-flow problems. | Polynomial in exploded supergraph; practical with summaries. | Taint/data-flow families, not general heap aliasing. |
| Sparse MemorySSA/SVFG | Precise memory/value-flow queries without dense program-point state. | More engineering, better scaling for flow-sensitive refinements. | Future high-precision mode after local facts and summaries. |
| Demand-driven pointer analysis | Good for targeted queries; incomplete under strict budgets. | Query-dependent. | Rule-requested precision escalation. |

## Complexity Notes

### Type Systems

Modern type checkers are not "just linear":

- TypeScript conditional/distributive types and overloads can be expensive.
- Python union narrowing, protocols, typed dicts, decorators, imports, and recursive aliases require careful caching.
- Go method sets, interfaces, aliases, and generics are manageable but must match language semantics.
- Java generics, classpaths, reflection, and bytecode/source splits complicate exactness.

polint should therefore treat type facts as cached provider outputs with explicit budgets and precision labels.

### Points-To

Classic Andersen constraints have cubic worst-case behavior because loads/stores can create new copy edges as points-to sets grow. Practical implementations avoid naive cubic behavior with:

- dense IDs;
- bitset sets;
- delta propagation;
- SCC/cycle collapsing;
- offline variable substitution;
- field-based/field-sensitive switches;
- type filtering;
- summary boundaries;
- budgeted query scopes.

This is why polint should not start with a mandatory global solve.

### Alias Queries

Alias answers are cheap only after the relevant providers have facts. `NoAlias` can often be answered early from scope, ownership, type disjointness, or disjoint allocation tokens. `MustAlias` is much harder and should be rare unless identity, singleton points-to, or language ownership proves it.

Default answer should be `MayAlias` or `Unknown`, not false `NoAlias`.

## Language-Specific Pressure

### Python

Python pushes the design toward:

- places and access paths over raw variable names;
- flow narrowing from guards;
- import/module resolution;
- class/function/module object values;
- decorator/metaclass/plugin summaries;
- TypeGuard/TypeIs and `isinstance`/`issubclass` support;
- explicit unknowns for monkeypatching, dynamic attributes, `getattr`, `setattr`, import hooks, and reflection.

Ty and Pyrefly show that a Rust-native Python checker is viable, but they also show how much of the precision comes from semantic/index/binding infrastructure before heap aliasing.

### TypeScript / JavaScript

TS/JS pushes toward:

- property-key modeling;
- structural type facts;
- discriminated unions and control-flow narrowing;
- function objects and closures;
- module namespace/default/named export values;
- object literal allocation tokens;
- framework callback registration summaries;
- dynamic property and `eval` unknowns.

TypeScript's flow graph is excellent for reference-specific narrowing, but it is not a points-to engine. TAJS and Jelly are better references for abstract interpretation and value/points-to style reasoning.

### Go

Go pushes toward:

- method-set and interface implementation facts;
- pointer receiver versus value receiver distinctions;
- address-taken and escape-related allocation tokens;
- function values and closures;
- package/module lifecycle correctness;
- generics and aliases;
- reflection/unsafe unknowns.

Go has a clean official semantic oracle. polint should validate against it while implementing native facts.

### Java/JVM

Java/JVM pushes toward:

- classpath-aware type facts;
- source and bytecode views;
- CHA/RTA/VTA/points-to tiers;
- reflection/class-loader/framework models;
- context sensitivity as an optional precision tier;
- Android/JVM lifecycle summaries.

The JVM ecosystem demonstrates why extension models and precision tiers are not optional.

## How Agent Extensions Change The Solver

Traditional analyzers must infer framework facts automatically. polint can ask agents to write code that says:

```rust
sink.add_call_target(call_site, target, Evidence::from_fixture("routes.rs"));
sink.add_summary(handler, Summary::request_response_boundary());
sink.add_points_to(req_user_place, auth_user_token);
sink.add_no_alias(cache_client, db_transaction, "distinct framework singletons");
```

This changes algorithm design:

- unknowns become extension opportunities;
- facts need stable IDs and provenance;
- merges need conflict diagnostics;
- cache keys need extension code digests;
- evaluation needs default-vs-extended deltas;
- public APIs can be more complex if they are typed and testable.

The result should be a higher maximum capability than a generic closed analyzer.

## Assumptions Challenged

### Assumption: "We need one full-program points-to result."

Rejected. Most rules need typed answers for specific places/calls/flows. Build a provider stack and budgeted query system. Materialize only facts that are requested, cached, or useful for downstream providers.

### Assumption: "External analyzers can be the implementation."

Mostly rejected for arbitrary OSS analyzers. Ty, Pyright, CodeQL, WALA, Soot, SVF, Doop, and similar tools are research references, comparison oracles, and benchmark sources, not the core runtime implementation.

Official language tooling is different. If the Go toolchain, JDK/JVM metadata, `javac`, TypeScript compiler behavior, or official Python metadata is the compatibility authority for a language, polint may use it behind a provider boundary. The invariant is that downstream consumers see polint-owned facts with stable IDs, provenance, precision labels, cache keys, and validation status.

### Assumption: "Alias analysis is language-neutral."

Partially false. The solver substrate can be neutral, but the facts are language-owned. Java reflection, Go interfaces, Python dynamic attributes, and JS property keys require language-specific lowering and unknown handling.

### Assumption: "More precision is always better."

False without budgets and evidence. A precise but expensive default analysis can make the product unusable. Precision should be requested by fact consumers, escalated by agents, and measured by the harness.

### Assumption: "Heuristics are bad."

False if labeled. Heuristics are useful when they emit precision labels and unknowns. They become dangerous only when reported as exact.

## Strategic Recommendation

Make type/value/place facts the next implementation target after CFG, and design points-to/alias as an internal query engine that can evolve.

This avoids building into a corner:

- place IDs make future points-to possible;
- type/value facts improve call graph and data-flow immediately;
- summaries scale interprocedural analysis;
- extension sinks allow agents to add repo-specific precision;
- alias results stay honest because they are derived from evidence.
