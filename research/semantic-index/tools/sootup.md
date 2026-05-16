# SootUp

## What It Is

SootUp is the modernized successor-style project around Soot concepts. It provides a cleaner view-oriented API, lazy class resolution, type hierarchy abstractions, and call graph algorithms such as CHA and RTA.

Primary inspected files:

- `sootup.core/src/main/java/sootup/core/views/View.java`
- `sootup.java.core/src/main/java/sootup/java/core/views/JavaView.java`
- `sootup.core/src/main/java/sootup/core/typehierarchy/TypeHierarchy.java`
- `sootup.callgraph/src/main/java/sootup/callgraph/AbstractCallGraphAlgorithm.java`
- `sootup.callgraph/src/main/java/sootup/callgraph/ClassHierarchyAnalysisAlgorithm.java`
- `sootup.callgraph/src/main/java/sootup/callgraph/RapidTypeAnalysisAlgorithm.java`

## Index Shape

Core objects:

- **View:** collection of code, class/method/field lookup, type hierarchy, identifier factory.
- **JavaView:** lazy class resolution from input locations with deterministic first-location behavior.
- **TypeHierarchy:** implementers, subclasses, subinterfaces, interfaces, direct subtypes, superclasses, subtype queries.
- **Call graph algorithms:** worklist over method signatures and call sites.

The `View` abstraction is close to what polint should expose internally as an analysis snapshot.

## Algorithm

```python
def java_view_lookup(view, class_type):
    if class_type in view.cache:
        return view.cache[class_type]
    for location in view.input_locations_in_order:
        maybe = location.resolve(class_type)
        if maybe:
            view.cache[class_type] = maybe
            return maybe
    return missing_class(class_type)

def cha_call_graph(view, entrypoints):
    worklist = list(entrypoints)
    graph = CallGraph()
    while worklist:
        method = worklist.pop()
        for call in method.calls:
            targets = resolve_with_type_hierarchy(view.type_hierarchy, call)
            for target in targets:
                if graph.add_edge(method, call, target):
                    worklist.append(target)
    return graph
```

## Accuracy

SootUp clarifies algorithm tiers:

- CHA over-approximates virtual dispatch by type hierarchy.
- RTA refines by instantiated classes.
- unresolved or not-yet-instantiated calls can be kept pending.

The `RapidTypeAnalysisAlgorithm` pattern of storing ignored/pending calls for classes not yet known as instantiated is particularly useful for precision accounting.

Hard cases:

- reflection;
- dynamic class loading;
- incomplete input locations;
- framework lifecycle;
- native methods.

## Complexity

View lookup is cache-dependent. Type hierarchy construction is roughly:

```text
O(classes + hierarchy_edges)
```

CHA dispatch can be:

```text
O(call_sites * candidate_subtypes)
```

RTA adds a fixed point over reachable methods and instantiated classes:

```text
O(iterations * (reachable_methods + call_sites + instantiated_classes))
```

## Strengths

- Cleaner than classic Soot.
- `View` is a good analysis snapshot concept.
- Lazy deterministic resolution.
- Explicit type hierarchy API.
- Algorithm tiers are easy to compare.

## Weaknesses

- JVM-specific.
- Still does not solve framework/dynamic modeling by itself.
- Call graph focus is downstream of semantic indexing.

## Polint Implications

Copy:

- internal `AnalysisView` concept;
- deterministic lazy resolution;
- hierarchy API shape;
- pending/ignored unresolved edge facts;
- algorithm tier labels.

Avoid:

- exposing the view as public mutable API;
- building only one exact JVM algorithm.

Semantic-index lesson:

```text
Keep unresolved/pending resolution facts rather than dropping them.
Those facts become agent extension opportunities.
```
