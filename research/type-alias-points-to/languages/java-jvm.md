# Java / JVM Report

## State Of The Art

The JVM ecosystem has the most mature public work on call graph, pointer analysis, alias analysis, and framework modeling. The most relevant tools are:

- Doop: Datalog-based whole-program Java points-to analysis.
- WALA: SSA IR and multiple pointer-analysis/call-graph builders.
- Soot/Spark: Jimple IR and points-to analysis graph.
- SootUp/Qilin: modernized Soot ecosystem with advanced pointer-analysis options.
- OPAL: Scala JVM bytecode analysis framework with TAC/CFG/fixpoint infrastructure.
- Checker Framework: source-level Java data-flow/type-qualifier analysis.
- CodeQL Java: query-facing semantic/data-flow/call graph model.

## Algorithm Ladder

JVM tools converge on this precision ladder:

```text
CHA
  -> RTA
  -> VTA/type propagation
  -> field-sensitive Andersen/Spark/Doop/WALA
  -> selective context sensitivity
  -> framework/reflection/lifecycle models
```

This should become polint's general ladder too.

## Implementation Findings

### Doop

Doop encodes points-to/call graph facts in Datalog/Souffle-style relations. It has extensive reflection and framework modeling.

Important relations include:

- variable/object points-to;
- instance/static field points-to;
- call graph edges;
- method invocation and virtual dispatch;
- reflection modeling.

Polint lesson: relational facts are excellent for recursive analyses and extension merges, but polint should keep the public SDK typed rather than exposing raw Datalog.

### WALA

WALA builds SSA IR and pointer analysis with call graph construction. Its docs and implementation show how pointer keys, instance keys, call graph builders, and context selectors fit together.

Polint lesson: context sensitivity should be a pluggable policy, not one global mode.

### Soot / Spark / SootUp / Qilin

Soot's Spark uses a points-to analysis graph. SootUp and Qilin expose modern options such as field sensitivity, on-the-fly call graph, and context sensitivity.

Polint lesson: separate options/policies from facts. Field sensitivity and on-the-fly call graph construction are precision policies with cache implications.

### OPAL

OPAL is a strong reference for bytecode/TAC and fixpoint/property-computation architecture.

Polint lesson: source-level and bytecode-level facts should be distinct when Java support arrives.

### Checker Framework

Checker Framework focuses on source-level data-flow and type qualifiers, not whole-program heap points-to. It is still relevant for local flow and type refinement.

Polint lesson: source-level local facts can deliver high rule value without full whole-program points-to.

## Java Accuracy Model

| Feature | Default polint target | Extension target |
|---|---|---|
| Classpath/types | Classpath-aware type/class/member facts. | Build-system-specific/generated classes. |
| Calls | CHA then RTA/VTA tiers. | Framework lifecycle/callback/reflection models. |
| Allocations | `new`, arrays, lambdas, method refs, class objects. | DI containers and generated factories. |
| Points-to | Field-sensitive Andersen-like provider. | Framework object identity and scopes. |
| Reflection | Unknown/default heuristic. | Explicit validated reflection models. |
| Android/server frameworks | Unknown unless modeled. | Lifecycle/entrypoint/DI provider extensions. |

## Complexity And Risk

Hard JVM cases:

- incomplete classpaths;
- reflection;
- custom class loaders;
- native methods;
- invokedynamic/lambdas;
- Android lifecycle;
- framework dependency injection;
- bytecode/source mismatch;
- generated code.

These are exactly where polint's extension model matters. Agent-authored providers should add lifecycle and reflection facts instead of making the default analyzer guess.

## Recommended Java/JVM Implementation Path

Java is not a first adapter today, but the future design should be:

```text
1. source/classpath identity and class/member facts
2. source CFG or bytecode CFG, explicitly separated
3. CHA call graph
4. allocation/type facts
5. RTA
6. VTA/type propagation
7. bounded field-sensitive Andersen
8. selective context sensitivity
9. reflection/framework extension facts
10. Doop/WALA/Soot/OPAL differential validation suites
```

Do not implement Java as a separate analysis universe. It should use the same polint fact layers as Go/TS/Python.
