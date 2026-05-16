# Recommended Implementation: Module Graph

## Goal

Build a native Rust package/module/topology subsystem that can support high-precision static analysis across Go, TS/JS, Python, Java/JVM, and future languages without making external package managers mandatory runtime dependencies.

The subsystem should:

- parse popular package-manager manifests and lockfiles natively;
- represent declared and resolved dependency facts separately;
- connect source imports to package/module facts;
- model source sets, generated code, and build targets where available;
- expose topology gaps as facts;
- let repo-local Rust extensions improve accuracy.

## Internal Module Layout

```text
crates/polint/src/module_graph/
  mod.rs
  ids.rs
  facts.rs
  discovery.rs
  manager.rs
  manifest.rs
  lockfile.rs
  requirements.rs
  resolved.rs
  import_resolution.rs
  source_set.rs
  topology.rs
  validation.rs
  cache_key.rs
  providers/
    go.rs
    js.rs
    python.rs
    jvm.rs
    rust_cargo.rs
    monorepo.rs
  formats/
    package_json.rs
    package_lock.rs
    pnpm_lock.rs
    yarn_lock.rs
    bun_lock.rs
    go_mod.rs
    go_work.rs
    pyproject.rs
    requirements_txt.rs
    uv_lock.rs
    poetry_lock.rs
    pdm_lock.rs
    pom_xml.rs
    gradle_static.rs
    cargo_toml.rs
    cargo_lock.rs
```

Keep this module internal first. Public SDK views should be curated after the facts stabilize.

## Fact Families To Implement First

| Fact | Timing | Notes |
|---|---|---|
| `WorkspaceRootFact` | phase 1 | Required for every other fact. |
| `PackageFact` | phase 1 | Workspace members, root packages, external packages, virtual roots. |
| `DependencyRequirementFact` | phase 1 | Declared dependency edges. |
| `ResolvedDependencyFact` | phase 2 | Lockfile/native-resolved/tool-reported selected edges. |
| `SourceSetFact` | phase 3 | Needed for Java/JVM, tests, generated roots, and build target selection. |
| `ImportToPackageFact` | phase 3 | Bridge from semantic index imports to package graph. |
| `RepoTopologyFact` | phase 4 | Ownership/layers/deploy units/generated zones. |

## Provider Pipeline

```python
def module_graph_pipeline(repo, semantic_imports, extension_providers):
    roots = discover_roots(repo)
    managers = detect_package_managers(roots)
    manifests = parse_manifests(roots, managers)
    requirements = collect_requirements(manifests)
    lock_edges = parse_lockfiles(roots, managers, requirements)
    native_edges = run_native_resolvers_where_ready(requirements, lock_edges)
    source_sets = infer_source_sets(manifests, roots, managers)
    imports = resolve_imports_to_packages(semantic_imports, manifests, lock_edges)
    topology = infer_topology(manifests, source_sets, imports)
    extensions = run_extension_providers(roots, manifests, requirements, imports)
    return validate_and_merge(roots, manifests, requirements, lock_edges,
                              native_edges, source_sets, imports, topology,
                              extensions)
```

## Implementation Phases

### Phase 1: Cross-Language Topology Skeleton

Deliver:

- typed IDs and fact arenas;
- root discovery walker using existing ignore-aware file discovery;
- package-manager detection;
- generic manifest registry;
- declared dependency requirements;
- diagnostics for missing/ambiguous roots;
- cache-key sidecars.

Support:

- `package.json`;
- `go.mod` and `go.work`;
- `pyproject.toml`;
- `requirements*.txt`;
- `pom.xml`;
- `Cargo.toml`;
- workspace config files such as `pnpm-workspace.yaml`, `.yarnrc.yml`, `bun.lock`, `nx.json`, `turbo.json`.

Acceptance:

- fixtures can assert roots, packages, and declared dependencies;
- every fact has provider/provenance/precision;
- root detection is deterministic;
- unsupported dynamic inputs emit explicit facts.

### Phase 2: Exact Lockfile Readers

Start with lockfiles that create high value without running solvers:

1. `package-lock.json`.
2. `pnpm-lock.yaml`.
3. Yarn v1 `yarn.lock`.
4. Yarn Berry lockfile plus `.pnp.cjs` metadata recognition.
5. `bun.lock` / `bun.lockb` recognition, parse text lockfile first.
6. `go.sum` as checksum/evidence, not full graph.
7. `uv.lock`.
8. `poetry.lock`.
9. `pdm.lock`.
10. `Cargo.lock`.

Do not try to fully solve npm/pip from registry metadata first. Lockfiles give exact selected edges in real repositories.

Acceptance:

- lockfile schema version recorded;
- selected versions and edge contexts emitted where available;
- peer dependency context represented for JS managers;
- Python markers/groups/extras preserved;
- missing or stale lockfile produces diagnostics instead of fake exactness.

### Phase 3: Go Native Resolver

Implement Go as the first full native resolver because semantics are tractable and directly useful:

```python
def go_mvs(main_modules):
    selected = {}
    queue = list(main_modules)
    while queue:
        module = queue.pop()
        for req in read_go_mod(module).requires:
            if selected[req.path] < req.version:
                selected[req.path] = req.version
                queue.append(load_module(req.path, req.version))
    return selected
```

Need to model:

- `module`;
- `require`;
- `replace`;
- `exclude`;
- `retract` as metadata;
- `go.work use`;
- `go.work replace`;
- local `replace` roots;
- vendor mode;
- module graph pruning/lazy loading status as precision notes.

Acceptance:

- known `go list -m all` fixtures match for simple and multi-module workspaces;
- local replace modules become workspace packages;
- lifecycle inputs include build tags and test inclusion;
- unresolved module loads are explicit facts.

### Phase 4: TS/JS Package And Import Resolution

Implement:

- package manager detection:
  `packageManager` field, lockfiles, config files, workspace files;
- workspace discovery:
  npm/Yarn/Bun `package.json#workspaces`, pnpm `pnpm-workspace.yaml`;
- package name normalization, scopes, and aliases;
- dependency sections:
  `dependencies`, `devDependencies`, `peerDependencies`, `optionalDependencies`, `bundleDependencies`, overrides/resolutions/catalog references;
- Node package `exports`/`imports` conditions;
- TypeScript `baseUrl`, `paths`, `rootDirs`, project references;
- CommonJS requires as conservative import facts.

Resolution ladder:

```python
def resolve_js_import(import_spec, from_file):
    if is_relative_or_absolute(import_spec):
        return resolve_file_or_directory(from_file, import_spec)
    if tsconfig_paths_match(import_spec):
        return resolve_ts_path_mapping(import_spec)
    if yarn_pnp_map_available():
        return resolve_pnp(import_spec, from_package)
    package = find_declared_or_installed_package(import_spec, from_package)
    return apply_package_exports(package, import_spec, conditions)
```

Acceptance:

- workspaces resolve internal package imports;
- tsconfig paths can map to package/source roots;
- undeclared dependencies are reported separately from unresolved imports;
- PnP and pnpm layouts are represented without depending on physical `node_modules`.

### Phase 5: Python Manifests, Locks, And Environments

Implement:

- PEP 508 dependency parser;
- `pyproject.toml` `[project]`, `[project.optional-dependencies]`, `[dependency-groups]`;
- tool sections for uv, Poetry, PDM where needed;
- `requirements.txt` subset: requirement lines, `-r`, constraints, editable/path/URL lines;
- uv/Poetry/PDM lockfile readers;
- conda `environment.yml` later as a separate provider;
- virtualenv/site-packages/stub discovery later as validation/support layer.

Preserve:

- markers;
- extras;
- groups;
- Python version requirements;
- URL/path/editable source;
- direct references;
- dynamic metadata status.

Acceptance:

- marker-conditioned edges are not flattened;
- dependency groups can be queried separately;
- path/editable deps become local package candidates;
- lockfile exactness is scoped to its environment/marker assumptions.

### Phase 6: JVM Static Tier

Implement Maven first:

- POM XML parse;
- parent POM declaration as unresolved/external unless local parent found;
- dependency scopes;
- dependency management;
- exclusions;
- BOM import recognition;
- multi-module reactor modules;
- Maven Resolver style conflict metadata as research-informed native logic later.

Implement Gradle as conservative static tier:

- detect settings files and included projects;
- parse obvious `dependencies` blocks only when static enough;
- infer source sets from standard layout and Gradle metadata;
- emit `UnsupportedDynamicBuildScript` for dynamic logic;
- allow extension providers to add exact Gradle project/source-set/dependency facts.

Acceptance:

- Maven static fixtures reach useful accuracy;
- Gradle fixtures are honest about dynamic uncertainty;
- source-set facts distinguish main/test/generated/plugin classpaths.

### Phase 7: Monorepo And Build Overlays

Implement static readers for:

- Nx project configs and `nx.json`;
- Turborepo package graph inputs and `turbo.json`;
- Pants BUILD address discovery and dependency inference hooks later;
- Bazel BUILD/MODULE/WORKSPACE target discovery later with conservative labels;
- generated source roots and ownership maps.

Acceptance:

- package/project graph and task/build graph are separate layers;
- affected-file or ownership facts can be derived for rules;
- repo-local Rust extensions can add or override topology with validation.

## Extension Surface

Extensions should be able to emit:

- workspace roots;
- package facts;
- source-set facts;
- dependency requirements;
- resolved dependency edges;
- import-to-package resolution hints;
- generated package/source facts;
- topology tags, layers, owners, deploy units;
- suppressions/replacements of low-confidence native facts.

Merge policy:

| Operation | Allowed? | Rule |
|---|---|---|
| Add package/source-set/root | Yes | Validate path exists or is generated by known provider. |
| Add declared dependency | Yes | Mark `ExtensionAsserted` until validated. |
| Add resolved edge | Yes | Must reference valid package ids or external target. |
| Mark native unresolved fact exact | Yes, gated | Requires validation fixture or explicit trust setting. |
| Override exact native lockfile edge | Rare | Must produce conflict diagnostic and retain old fact. |
| Suppress heuristic fact | Yes | Keep suppressed fact in evidence side table. |

## Cache Keys

Module graph provider keys must include:

```text
repo root digest
file discovery inputs
manifest file digests
lockfile file digests
package-manager config digests
language lifecycle config
provider version
fact schema version
parser/format version
extension crate digest
input semantic import layer digest
selected environment or marker profile
```

## Public SDK Timing

Do not expose this all at once. Recommended sequence:

1. Internal `WorkspaceRootFact`, `PackageFact`, `DependencyRequirementFact`.
2. Internal lockfile exact `ResolvedDependencyFact`.
3. Public `Packages<'_>` and `Dependencies<'_>` once Go and TS/JS are stable.
4. Public `Imports<'_>` after semantic-index import facts are stable.
5. Public `RepoTopology<'_>` after extension validation and docs.

## Max-Capability Principle

The engine should provide the best native facts it can, but it should never hide uncertainty. The highest ceiling comes from combining:

```text
native ecosystem knowledge
  + exact lockfile readers
  + semantic import resolution
  + source-set/build-target facts
  + repo-local Rust topology extensions
  + validation and benchmarks
```

That architecture is harder than a generic dependency parser, but it avoids the early corner: false exactness from a too-small graph model.
