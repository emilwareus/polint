# Java / JVM Abstract Domains

## State Of The Art Pattern

Java/JVM tools split cleanly:

- Checker Framework, NullAway, and Error Prone use javac/source facts with
  flow-sensitive stores and access paths.
- WALA, Soot, OPAL, and SpotBugs use bytecode/IR/SSA/frame domains.
- Resource and typestate checks are often annotation/model-driven.

For polint, use a native Rust domain kernel, but use official JVM/JDK/javac
metadata where it is the semantic authority.

## Recommended Domains

| Domain | Use |
|---|---|
| Nullness | source-level and bytecode-level facts. |
| CalledMethods | builder/typestate-style definite method calls. |
| MustCall/resource | close/free/unlock obligations. |
| Initialization | constructor and field initialization facts. |
| Bytecode frames | optional classfile-only facts. |
| Constants/ranges | from literals, annotations, and bytecode constants. |

## Nullness

Use an access-path store:

```text
this.field
local.field
local.getX()
map.get(key) under containsKey(key)
```

Unknown calls should invalidate heap access paths unless purity/effect metadata
or summary facts say otherwise.

## Typestate And Must-Call

Checker Framework's split is useful:

- `MustCall`: methods/resources that must be called;
- `CalledMethods`: methods definitely called;
- report when obligations are not satisfied at ownership/scope boundary.

Generalize this to repo-local typestate machines:

```text
new Resource -> state Open, must_call Close
close() -> state Closed
scope exit -> require state Closed or ownership transferred
```

## JVM Metadata

Use official metadata for:

- class names, packages, modules, records, sealed classes;
- descriptors and generic signatures;
- annotations and type annotations;
- method parameters and local variable tables;
- line numbers;
- stack map tables;
- bootstrap methods and `invokedynamic`;
- synthetic/bridge methods.

## Avoid In V1

- full javac overload/generic inference clone;
- annotation processor semantics;
- Lombok/delombok behavior;
- full object-sensitive whole-program pointer analysis.

## First JVM Slice

When JVM support begins:

1. source/classfile identity and metadata facts;
2. nullness access-path store;
3. called-methods typestate;
4. must-call resource domain;
5. method summaries from source, annotations, and models.
