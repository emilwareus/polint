# Algorithm: Import To Package Resolution

## Goal

Map source imports from the semantic index to package/module facts from the module graph.

This answers:

```text
Which package owns this imported module?
Is the dependency declared?
Is it runtime, dev, test, optional, peer, or external?
Is the edge exact under current lifecycle inputs?
```

## Language-Neutral Shape

```python
def resolve_imports_to_packages(import_facts, package_index, language_contexts):
    for imp in import_facts:
        context = language_contexts.for_file(imp.file)
        owner = package_index.package_owning_file(imp.file)
        result = context.resolver.resolve(imp.specifier, imp.file, owner, package_index)
        emit(ImportToPackageFact(
            import_fact=imp.id,
            from_file=imp.file,
            from_package=owner,
            specifier=imp.specifier,
            resolved_module=result.module,
            resolved_package=result.package,
            external_name=result.external_name,
            status=result.status,
            precision=result.precision,
        ))
```

## Go

```python
def resolve_go_import(specifier, from_file, go_context):
    package = go_context.import_path_index.get(specifier)
    if package:
        return exact(package)
    if is_stdlib(specifier):
        return external("go-stdlib", specifier)
    return unresolved_or_external(specifier)
```

## TS/JS

```python
def resolve_js_import(specifier, from_file, ctx):
    if is_relative(specifier):
        module = resolve_file_or_directory(from_file, specifier, ctx.extensions)
        return owner_package(module)

    if ctx.tsconfig.paths.match(specifier):
        candidates = resolve_ts_paths(specifier, ctx.tsconfig)
        return package_from_candidates(candidates)

    if ctx.pnp_map:
        return ctx.pnp_map.resolve(owner_package(from_file), specifier)

    package_name, subpath = split_package_specifier(specifier)
    package = resolve_declared_package(package_name, owner_package(from_file), ctx)
    module = apply_exports(package, subpath, ctx.conditions)
    return package, module
```

## Python

```python
def resolve_python_import(specifier, from_file, ctx):
    owner = owner_package(from_file)
    for root in ctx.search_path(owner):
        module = find_module_or_package(root, specifier)
        if module:
            return owner_package(module)
    if ctx.stub_path.has_stub(specifier):
        return external_stub_package(specifier)
    return dynamic_or_unresolved(specifier)
```

Python needs explicit precision labels because runtime `sys.path`, namespace packages, `.pth` files, editable installs, and generated modules are common.

## Java/JVM

```python
def resolve_java_import(import_name, from_file, ctx):
    source_set = ctx.source_set_for_file(from_file)
    candidates = ctx.classpath.lookup_type_or_package(import_name, source_set)
    if len(candidates) == 1:
        return exact(candidates[0].package)
    if len(candidates) > 1:
        return ambiguous(candidates)
    return unresolved(import_name)
```

Exactness depends on classpath/module path/source-set facts.

## Declared-Vs-Used Check

```python
def check_declared_for_import(import_edge, dependency_edges):
    if import_edge.status not in EXACTISH:
        return
    owner = import_edge.from_package
    target = import_edge.resolved_package
    if not dependency_edges.has_allowed_edge(owner, target, import_edge.context):
        emit_undeclared_dependency(import_edge)
```

This supports rules like "no undeclared dependency", "test cannot import production-only package", and "package cannot cross layer."

## Complexity

With caches:

- Go: near `O(I)`.
- TS/JS: `O(I * (path depth + extension candidates + condition branches))`.
- Python: `O(I * search path roots * module candidate checks)`.
- Java: `O(I * lookup cost)` where lookup is indexed by package/type/classpath.

## Cache Inputs

Import resolution keys include:

- semantic import layer digest;
- package graph digest;
- tsconfig/dynamic resolver config;
- Python environment/stub/search path config;
- Go module/build tag/test config;
- JVM source-set/classpath config;
- extension provider digest.
