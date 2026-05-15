# WALA

## What It Is

WALA is a long-running static analysis framework for Java bytecode, Android, and JavaScript. It is important for semantic indexing because it makes class loader context, type/method references, class hierarchy, context-sensitive call graph nodes, and pointer-analysis interfaces explicit.

Primary inspected files:

- `core/src/main/java/com/ibm/wala/ipa/callgraph/AnalysisScope.java`
- `core/src/main/java/com/ibm/wala/ipa/cha/ClassHierarchy.java`
- `core/src/main/java/com/ibm/wala/types/TypeReference.java`
- `core/src/main/java/com/ibm/wala/types/MethodReference.java`
- `core/src/main/java/com/ibm/wala/types/ClassLoaderReference.java`
- `core/src/main/java/com/ibm/wala/ipa/callgraph/CGNode.java`
- `core/src/main/java/com/ibm/wala/ipa/callgraph/propagation/PointerAnalysis.java`
- `core/src/main/java/com/ibm/wala/ipa/callgraph/AnalysisCacheImpl.java`

## Index Shape

Core objects:

- **AnalysisScope:** partitions code by class loader: primordial, extension, application, synthetic.
- **ClassHierarchy:** maps type references to class hierarchy nodes and handles missing superclass policies.
- **TypeReference:** canonicalized by classloader and type name.
- **MethodReference:** canonicalized by declaring type and selector/descriptor.
- **CGNode:** method plus context.
- **PointerAnalysis:** pointer keys, instance keys, heap graph, class hierarchy.
- **AnalysisCache:** caches IR, def-use, and analysis products.

## Algorithm

```python
def build_wala_identity(scope):
    for loader in scope.class_loaders:
        for module in scope.modules(loader):
            for cls in read_classes(module):
                type_ref = canonical_type(loader, cls.name)
                class_hierarchy.add(type_ref, cls)

def make_call_graph_node(method_ref, context):
    return CGNode(method=method_ref, context=context)

def lookup_method(type_ref, selector):
    cls = class_hierarchy.lookup(type_ref)
    return cls.lookup_method(selector)
```

## Accuracy

WALA is strong in whole-program JVM analysis when class loader/module scope is correct. The core identity lesson is that `ClassName.method` is not a complete semantic key. It must include:

- class loader;
- declaring type;
- selector/descriptor;
- context for context-sensitive analyses.

Hard cases:

- incomplete classpath;
- reflection;
- native methods;
- Android lifecycle/callbacks;
- JavaScript dynamic features;
- context explosion in precise pointer analyses.

## Complexity

Class hierarchy construction:

```text
O(classes + hierarchy_edges)
```

Call graph construction integrated with pointer analysis ranges from relatively cheap RTA/0-CFA to much more expensive context-sensitive algorithms:

```text
O(pointer_constraints * propagation_iterations)
```

Context sensitivity multiplies semantic identities:

```text
node identity = method * context
```

This is essential for precision but can increase memory and runtime substantially.

## Strengths

- Excellent identity model for JVM whole-program analysis.
- Explicit classloader scope.
- Mature context-sensitive call graph and pointer-analysis APIs.
- Strong cache separation for IR/def-use.

## Weaknesses

- JVM/bytecode-centered.
- Not a direct source semantic-index implementation.
- Context-sensitive analysis can be expensive.

## Polint Implications

Copy:

- identity must include lifecycle context, not just names;
- future call graph nodes should be `callable + context`, not just `callable`;
- class loader/module/package context should be part of stable keys;
- cache IR/def-use/type facts separately.

Avoid:

- flattening all external symbols into one namespace;
- adding context sensitivity without budget/precision labels.

Recommended future fact key:

```text
CallableId = SymbolId + Optional<AnalysisContextId>
```

Semantic index should be context-free first, but it must not make future context-sensitive call/data-flow impossible.
