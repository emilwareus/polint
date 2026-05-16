# Go Module Graph Report

## Scope

Go should be the first language where polint attempts a full native module/package graph.

Important package-manager/build inputs:

- `go.mod`;
- `go.work`;
- `go.sum`;
- local `replace` directives;
- `exclude` and `retract` directives;
- `vendor/modules.txt`;
- package import paths;
- build tags and test inclusion.

## State Of The Art

The official Go module reference defines Minimal Version Selection over a directed module graph: vertices are module versions, edges are minimum required dependency versions, and MVS returns the build list by selecting the highest required version per module path. It also defines workspaces: `go.work` lists multiple main modules, and workspace `replace` directives can modify the module graph.

`x/mod/modfile` is the canonical implementation reference for parsing `go.mod` and `go.work`. `gopls` adds the practical workspace layer: it finds `go.work`, parses `use` directives into local `go.mod` roots, includes local `replace` modules from `go.mod`, and treats workspace files as lifecycle inputs.

## OSS Implementation Evidence

Local repositories:

- `repos/go`: Go command implementation, especially `cmd/go/internal/mvs` and `cmd/go/internal/modload`.
- `repos/golang-mod`: exact parser/formatter for `go.mod` and `go.work`.
- `repos/golang-tools`: `gopls` workspace and package metadata lifecycle.

Key files:

- `golang-mod/modfile/rule.go`: parsed `File` contains `Module`, `Require`, `Exclude`, `Replace`, `Retract`, `Tool`, and `Ignore`.
- `golang-tools/gopls/internal/cache/workspace.go`: `goWorkModules`, `goModModules`, local replace discovery, workspace-file matching, and a bounded module-file search.
- `golang-tools/gopls/internal/cache/snapshot.go`: module roots, package loading, workspace packages, diagnostics, and metadata reload lifecycle.
- `golang-tools/go/packages/packages.go`: package loading API and module metadata.

## Algorithm

```python
def discover_go_roots(repo):
    go_work = nearest_or_configured_go_work(repo)
    if go_work:
        modules = parse_go_work(go_work).use_paths
        roots = [path / "go.mod" for path in modules]
        roots += local_replaces_from_each_go_mod(roots)
        return unique_existing_roots(roots, source="go.work")

    roots = find_go_mod_roots(repo)
    roots += local_replaces_from_each_go_mod(roots)
    return unique_existing_roots(roots, source="go.mod")

def go_mvs(main_modules):
    selected = {}
    queue = list(main_modules)
    while queue:
        mod = queue.pop()
        for req in parse_go_mod(mod).requires:
            req = apply_replace_and_exclude(req)
            if selected.get(req.path, LOW) < req.version:
                selected[req.path] = req.version
                queue.append(load_module(req.path, req.version))
    return selected
```

## Accuracy

High for:

- module roots;
- local workspace modules;
- direct declared requirements;
- local replacements;
- selected module versions when all needed `go.mod` metadata is available.

Lower or conditional for:

- packages selected by build tags;
- tests and external test packages;
- generated Go files;
- vendor mode;
- module cache/network-unavailable dependencies;
- lazy loading/pruned module graph effects when not loading packages.

## Complexity

- Root discovery: `O(F)` with ignore pruning, plus bounded module root search.
- Manifest parse: `O(total go.mod/go.work bytes)`.
- MVS: `O(V + E)` over loaded module requirement graph, with map updates per module path.
- Package import ownership: near `O(imports)` once packages are indexed by import path.

## Polint Implementation

Implement native Go provider with:

- `GoWorkspaceRootFact`;
- `PackageFact` for each module and package;
- `DependencyRequirementFact` for `require`;
- `ResolvedDependencyFact` for MVS-selected modules;
- `ImportToPackageFact` for Go import paths;
- lifecycle inputs for `build_tags`, `include_tests`, module roots, and workspace mode.

## Recommended Precision Labels

- `ManifestExact` for parsed module roots and require/replace/exclude.
- `NativeResolved` for MVS-selected module edges when all module metadata is available.
- `Conservative` when package loading is skipped.
- `Unsupported` or `Dynamic` for generated imports or missing module cache.

## First Fixtures

- Single module with direct and indirect deps.
- `go.work` with two modules.
- Local `replace` to sibling module.
- Version conflict where MVS selects highest minimum.
- Vendor mode fixture.
- Build tags fixture with two alternative imports.
- Test import fixture.
