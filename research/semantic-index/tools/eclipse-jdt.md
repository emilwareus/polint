# Eclipse JDT Core

## What It Is

Eclipse JDT Core includes the Eclipse Java compiler and Java model. It is the best source-level Java semantic-index reference in this research set: bindings, scopes, lookup environment, imports, packages, modules, and incremental compiler behavior.

Primary inspected files:

- `org.eclipse.jdt.core.compiler.batch/src/org/eclipse/jdt/internal/compiler/lookup/Binding.java`
- `org.eclipse.jdt.core.compiler.batch/src/org/eclipse/jdt/internal/compiler/lookup/Scope.java`
- `org.eclipse.jdt.core.compiler.batch/src/org/eclipse/jdt/internal/compiler/lookup/LookupEnvironment.java`
- `org.eclipse.jdt.core.compiler.batch/src/org/eclipse/jdt/internal/compiler/lookup/CompilationUnitScope.java`
- `org.eclipse.jdt.core/dom/org/eclipse/jdt/core/dom/DefaultBindingResolver.java`

## Index Shape

Core objects:

- **Binding:** semantic identity for fields, locals, types, methods, packages, imports, modules, arrays, parameterized/raw/generic types, type parameters.
- **Scope:** block, method, class, compilation unit, module.
- **LookupEnvironment:** packages, modules, known types, missing types, default imports, classpath/module state.
- **CompilationUnitScope:** top-level unit scope, imports, reference recording.
- **DOM binding resolver:** maps compiler bindings to DOM bindings and binding keys.

JDT is built around staged type/binding completion. Type completion levels include imports, hierarchy connection, hierarchy sealing, fields/methods, annotations, and parameterized type checks.

## Algorithm

```python
def jdt_compile_unit(unit, environment):
    cu_scope = CompilationUnitScope(unit, environment)
    cu_scope.build_type_bindings()
    cu_scope.resolve_imports(non_static=True)
    cu_scope.resolve_imports(static=True)
    cu_scope.record_simple_and_qualified_references()
    return cu_scope

def resolve_name(scope, name):
    # Location-aware lookup starts in the innermost scope.
    for s in scope.walk_outward():
        result = s.lookup_local_field_method_type_or_package(name)
        if result:
            return result

    result = lookup_imports_and_on_demand_packages(scope.compilation_unit, name)
    if result:
        return result

    return problem_binding("not found or ambiguous")
```

## Accuracy

JDT is strong because it models:

- source and binary types;
- packages and modules;
- imports and static imports;
- nested/member types;
- fields, methods, locals;
- generics/type parameters;
- unresolved-error tolerant compilation;
- reference dependency recording for incremental builds.

Hard cases:

- incomplete classpaths;
- annotation processors/generated code;
- reflection;
- module path differences;
- source/binary mismatch;
- unresolved code that IDE still permits.

## Complexity

Lookup is location-sensitive and mostly bounded by scope depth plus imports/package lookup, but Java type resolution can be expensive due to generics, overloads, and hierarchy traversal.

The key complexity strategy is **staged completion**:

```text
imports -> hierarchy -> fields/methods -> annotations -> parameterized checks
```

This avoids doing every expensive operation immediately and lets the compiler work with partially completed bindings.

## Strengths

- Mature Java source semantic model.
- Explicit binding kinds and scope kinds.
- Strong package/module/classpath environment.
- Incremental dependency recording.
- Tolerates unresolved code while still indexing useful facts.

## Weaknesses

- Java-specific and large.
- Mutable compiler state is not the right direct shape for polint.
- Exact behavior depends on classpath/module setup.

## Polint Implications

Copy:

- binding taxonomy;
- staged semantic completion;
- `ProblemBinding` equivalent for unresolved/ambiguous references;
- package/module/classpath as first-class lifecycle inputs;
- dependency/reference recording during lookup.

Avoid:

- file-only Java analysis;
- hiding missing classpath facts;
- making Java reference facts exact without module/classpath provenance.

Recommended Java stable key shape:

```text
java:
  class_loader_or_scope
  module_name
  package_name
  binary_or_source_type_name
  member_signature
  type_parameter_context
```
