# Soot

## What It Is

Soot is a classic Java/JVM program analysis framework. It is not primarily a source semantic index, but it is highly relevant for whole-program semantic identity: classes, methods, fields, resolving levels, type hierarchy, call graph, and points-to.

Primary inspected files:

- `src/main/java/soot/Scene.java`
- `src/main/java/soot/SootResolver.java`
- `src/main/java/soot/SootClass.java`
- `src/main/java/soot/FastHierarchy.java`
- `src/main/java/soot/jimple/toolkits/callgraph/CallGraph.java`
- `src/main/java/soot/jimple/toolkits/callgraph/ReachableMethods.java`
- `src/main/java/soot/jimple/spark/SparkTransformer.java`

## Index Shape

Core objects:

- **Scene:** global manager for classes, application/library/phantom classes, hierarchy, points-to, call graph.
- **SootClass:** class name/package/modifiers, fields, methods, resolving level.
- **SootMethod/SootField:** member identity.
- **SootResolver:** worklist resolver by desired class resolving level.
- **FastHierarchy:** accelerated subtype/interface queries.
- **CallGraph:** edge container indexed by source unit, source method, and target.
- **ReachableMethods:** incremental reachable set driven by call graph edge stream.

Resolving levels:

```text
DANGLING -> HIERARCHY -> SIGNATURES -> BODIES
```

This level structure is a major design lesson.

## Algorithm

```python
def resolve_class(name, desired_level):
    cls = scene.get_or_create_class(name)
    if cls.level >= desired_level:
        return cls

    worklist.add(cls, desired_level)
    while worklist:
        c, level = worklist.pop()
        if level >= HIERARCHY:
            resolve_superclass_and_interfaces(c)
        if level >= SIGNATURES:
            resolve_fields_and_methods(c)
        if level >= BODIES:
            resolve_method_bodies(c)
        c.level = max(c.level, level)
    return cls

def build_fast_hierarchy(scene):
    hierarchy = FastHierarchy()
    hierarchy.index_subclasses_interfaces_and_vtables(scene.classes)
    return hierarchy
```

## Accuracy

Soot can be strong in closed-world or well-modeled JVM analysis settings. It supports phantom classes for missing dependencies, which is honest but affects precision.

Hard cases:

- reflection;
- dynamic class loading;
- invokedynamic/lambdas depending on frontend/modeling;
- native methods;
- incomplete classpath;
- frameworks.

The 2026 Java call graph unsoundness research is important: mature frameworks such as Soot, SootUp, WALA, and Doop can disagree on call-graph semantics for modern Java features.

## Complexity

Class resolving cost depends on classpath size and desired level. Hierarchy construction is roughly:

```text
O(classes + subtype_edges)
```

Call graph and points-to costs vary dramatically by algorithm. Spark-style points-to can be high-polynomial in the number of variables/allocation sites/constraints, with optimizations such as SCC collapse.

## Strengths

- Clear resolving levels.
- Efficient hierarchy indexes.
- Explicit phantom/missing-class behavior.
- Call graph and reachable-method streams.
- Whole-program analysis orientation.

## Weaknesses

- Global mutable `Scene` is not a good polint architecture.
- JVM bytecode orientation differs from source semantic indexing.
- Precision depends on whole-program setup and modeling.

## Polint Implications

Copy:

- resolving levels for semantic facts;
- explicit missing/phantom external symbols;
- hierarchy indexes;
- call graph edge indexing and edge streams later.

Avoid:

- global singleton architecture;
- treating phantom/external targets as exact;
- building JVM call graphs before semantic index/module graph/type hierarchy research is stable.

Suggested polint levels:

```text
Parsed
  -> Declared
  -> SignatureResolved
  -> BodyBound
  -> TypeAssisted
  -> WholeProgram
```
