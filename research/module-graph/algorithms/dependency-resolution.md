# Algorithm: Declared And Resolved Dependency Edges

## Goal

Separate what a manifest declares from what a package manager selected.

## Declared Requirements

```python
def collect_declared_requirements(packages):
    for pkg in packages:
        manifest = pkg.manifest
        for section in dependency_sections(manifest):
            for raw_name, raw_spec in section.entries:
                req = parse_requirement(pkg.ecosystem, raw_name, raw_spec)
                emit(DependencyRequirementFact(
                    from_package=pkg.id,
                    raw_name=raw_name,
                    normalized_name=normalize_name(pkg.ecosystem, raw_name),
                    requested=req.spec,
                    kind=section.kind,
                    marker=req.marker,
                    source_span=req.span,
                    status=req.status,
                ))
```

## Resolved Edges From Lockfiles

```python
def resolved_edges_from_lockfiles(roots, requirements):
    for root in roots:
        manager = root.manager
        lockfile = find_lockfile(root, manager)
        if not lockfile:
            emit_missing_lockfile_if_expected(root)
            continue

        parser = lockfile_parser(manager, lockfile.schema_version)
        entries = parser.parse(lockfile)
        package_instances = intern_lockfile_packages(entries)
        for entry in entries:
            for dep in entry.dependencies:
                target = resolve_lockfile_target(dep, package_instances)
                emit_resolved_edge(entry.package, target, source="Lockfile")
```

## Native Resolver Tier

Only run native solvers where implemented and requested.

```python
def run_native_resolvers(roots, requirements, lock_edges):
    for root in roots:
        if root.ecosystem == "go":
            emit_edges(go_mvs(root.main_modules))
        elif root.ecosystem == "maven" and root.static_complete():
            emit_edges(maven_static_resolve(root))
        elif root.has_lockfile_edges():
            continue
        else:
            emit_unresolved_selected_versions(root)
```

## Merge Rules

```python
def merge_declared_and_resolved(requirements, resolved_edges):
    by_requirement = index_resolved_edges(resolved_edges)
    for req in requirements:
        matches = by_requirement.get(req.id)
        if not matches:
            emit_unresolved_requirement(req)
            continue
        for edge in matches:
            validate_edge_target(edge)
            emit(edge)
```

## Conflict Policy

| Conflict | Policy |
|---|---|
| Manifest declares dep missing from lockfile | Emit `DeclaredButUnresolved` unless manager allows dev-only omitted lock entries. |
| Lockfile entry not declared | Emit `TransitiveResolved` or `ExtraneousLockEntry` depending on graph reachability. |
| Multiple package managers claim root | Emit ambiguous root and keep facts manager-scoped. |
| Extension contradicts lockfile exact edge | Keep lockfile edge, emit conflict diagnostic, require validation to override. |

## Complexity

- Declared requirements: `O(P + R)`.
- Lockfile parse: `O(L + P + E)`.
- Merge: `O(R + E)` with maps.
- Native solving: ecosystem-specific; general solving can be exponential.

## Implementation Notes

Do not store resolved edges as only `from -> name`.

Required dimensions:

- selected version;
- package instance context;
- peer context if JS;
- marker/environment if Python;
- source-set/configuration if JVM;
- dependency kind/scope;
- precision;
- source file and span;
- manager/provider version.
