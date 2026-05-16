# Research Analysis: Module Graphs

## Main Thesis

Package and module topology should be a **fact system**, not a single graph class.

The state of the art splits the problem into:

1. workspace/root discovery;
2. manifest and lockfile parsing;
3. ecosystem-specific dependency resolution;
4. source import resolution;
5. build target/source-set modeling;
6. monorepo project and task graphs;
7. architecture/ownership overlays.

polint should copy this split and keep each fact's precision visible.

## Architecture Patterns

### Manifest Reader Pattern

Used by `x/mod`, package manager frontends, Turborepo, Nx, uv workspace discovery, Maven POM readers, and Cargo.

```python
def parse_manifests(roots):
    packages = []
    requirements = []
    for root in roots:
        manifest = read_manifest(root.manifest)
        package = package_from_manifest(manifest)
        packages.append(package)
        requirements.extend(dependencies_from_manifest(package, manifest))
    return packages, requirements
```

Accuracy is high when the manifest is declarative and complete. It is lower when build scripts or generated manifests participate.

### Lockfile Reader Pattern

Used by npm, pnpm, Yarn, Bun, uv, Poetry, PDM, Cargo, and Gradle lock modes.

```python
def resolved_edges_from_lockfile(lockfile, packages):
    entries = parse_lock(lockfile)
    for entry in entries:
        pkg = intern_package(entry.name, entry.version, entry.context)
        for dep in entry.dependencies:
            emit_resolved_edge(pkg, dep.target, source="Lockfile")
```

Lockfiles give selected versions, but their semantics vary. Some encode dependency edges directly; others encode package entries and require reconstruction.

### Native Resolver Pattern

Used by Go MVS, Cargo, uv/PubGrub, pip/resolvelib, Poetry/Mixology, Maven Resolver, Gradle, and Conda.

```python
def resolve(requirements, repository, policy):
    state = ResolutionState(requirements)
    while not state.done():
        requirement = policy.pick_requirement(state)
        candidates = repository.candidates(requirement)
        candidate = policy.pick_candidate(candidates)
        if candidate.conflicts(state):
            state.backtrack_or_learn(candidate)
        else:
            state.add(candidate)
            state.add_requirements(candidate.dependencies)
    return state.solution
```

The algorithm is simple only in restricted ecosystems. General dependency solving is hard, and real package managers add policies for peers, variants, markers, sources, yanks, platforms, and lockfile reuse.

### Import-To-Package Pattern

Used by TypeScript, Pyright, gopls, JDT, Oxc resolver, enhanced-resolve, Pants dependency inference, and build tools.

```python
def resolve_import_to_package(import_fact, package_index, language_options):
    module = language_resolve(import_fact.specifier, import_fact.from_file, language_options)
    package = package_index.owner_of(module.file_or_external_name)
    return ImportToPackageFact(import_fact, module, package)
```

This is what converts "package A declares dependency B" into "file X actually imports module Y from package B."

### Build Target Pattern

Used by Bazel, Pants, Gradle, Maven, Nx, and Turborepo task graphs.

```python
def build_target_graph(build_files):
    targets = parse_targets(build_files)
    for target in targets:
        deps = collect_declared_or_inferred_deps(target)
        emit_target_edges(target, deps)
    return targets
```

Build targets are not equivalent to packages. A single package can have many source sets, generated targets, test targets, binary targets, and deploy artifacts.

### Extension Overlay Pattern

Required by polint's agent-extensible product model.

```python
def merge_topology(native_facts, extension_facts):
    for fact in extension_facts:
        validate_schema(fact)
        validate_references(fact, native_facts)
        if conflicts_with_exact_native_fact(fact):
            emit_conflict(fact)
        else:
            add_with_extension_precision(fact)
    return merged
```

Extensions are the path to max capability without forcing universal black-box inference.

## Accuracy Comparison

| Ecosystem/Tool | Accuracy Strength | Accuracy Weakness |
|---|---|---|
| Go modules | MVS is deterministic; `go.mod`/`go.work` are declarative; local replaces are explicit. | Build tags, generated code, vendor mode, lazy loading, and missing module cache affect full package facts. |
| npm Arborist | Models actual npm install tree, hoisting, lockfiles, peers, optional/platform checks. | Complex install tree is not the same as source import graph; peer placement can explode contexts. |
| pnpm | Strong workspace graph, strict dependency access, `workspace:` protocol, content-addressed virtual store. | Symlink layout and peer contexts require manager-specific modeling. |
| Yarn Berry/PnP | `.pnp.cjs` is an explicit dependency map and protects ghost dependencies. | Tools must understand PnP; lockfile alone is not the full runtime resolver. |
| Bun | Fast npm-compatible workspace/lockfile ecosystem with Bun-specific lock/install options. | Lockfile and isolated-install semantics are still evolving relative to older tools. |
| TypeScript/Oxc resolver | Strong import resolution including tsconfig paths/references and package exports. | Dynamic imports, CommonJS patterns, bundler plugins, and generated aliases require extensions. |
| pip/resolvelib | Generic backtracking resolver with provider API and useful result graph. | Environment markers, installed state, constraints, extras, and index IO affect accuracy and cost. |
| uv | Modern Rust implementation, PubGrub, universal marker-aware resolution, workspace cache keys. | Exact registry solving is broader than polint needs initially; lockfile semantics still require careful parsing. |
| Poetry/PDM | Mature Python project lock/solve semantics and groups/markers support. | Tool-specific lock formats and dynamic package metadata. |
| Conda/libmamba | SAT-style environment solving over rich package records and platform/build metadata. | Solving depends on channel metadata and installed prefix state; not a simple source repo graph. |
| Maven Resolver | Declarative POMs, scopes, dependency management, conflict transformers. | Parent POMs, profiles, plugins, repositories, and generated sources complicate static exactness. |
| Gradle | Most expressive JVM dependency model: variants, attributes, capabilities, substitutions. | Dynamic build scripts make exact native static parsing unrealistic early. |
| Bazel/Skyframe | Explicit targets, configured graph, incremental dependency tracking, query APIs. | Starlark macros, repository rules, configuration, and generated targets require build-system semantics. |
| Pants | Source ownership and dependency inference across languages. | Plugin/rule execution is the graph; static parsing only approximates. |
| Nx/Turborepo | Practical monorepo project graph, cache keys, affected-file analysis, task graph. | Mostly project/task layer, not full language semantic resolution. |
| Cargo | Excellent workspace/lockfile/features model and native Rust implementation reference. | Rust not current polint target language, but useful for internal design. |

## Complexity Analysis

### Root Discovery

Expected:

```text
O(F)
```

with ignore-aware pruning and early recognition of root files. The hard part is not big-O; it is choosing the correct root when a repository contains multiple managers, nested workspaces, generated packages, examples, vendored directories, or checked-in fixtures.

### Manifest Parsing

Expected:

```text
O(total manifest bytes)
```

Memory is proportional to parsed packages and declared requirements. XML/TOML/JSON/YAML parsers should be structured, not ad hoc string scanning.

### Workspace Glob Expansion

Expected:

```text
O(candidate directories + matching patterns)
```

Do not glob through `node_modules`, build outputs, `.git`, virtualenvs, `target`, `dist`, or cache directories unless explicitly configured.

### Lockfile Parsing

Expected:

```text
O(lockfile bytes + lockfile nodes + lockfile edges)
```

Some lockfiles encode peer contexts or package instances as composite keys. The implementation must preserve those keys because collapsing them creates false dependency edges.

### Dependency Solving

Theoretical worst case:

```text
NP-hard / exponential for general dependency solving
```

Research confirms that generic package solving remains hard, even though practical tools use strong heuristics, lockfiles, learned incompatibilities, SAT/PubGrub/backtracking, and metadata reduction.

Practical tiers:

- Go MVS: graph traversal with highest-version selection over requirements.
- Maven conflict mediation: graph collection plus conflict resolution, path-based implementation targets `O(N)` conflict handling in current Maven Resolver.
- Cargo/PubGrub/pip/Poetry/uv: backtracking or conflict learning; usually fast on real projects but exponential in adversarial cases.
- Conda/libmamba: SAT solving over large channel metadata; cost dominated by metadata reduction and solver search.
- Gradle: resolution plus variant/capability matching; cost depends on metadata downloads, configurations, conflicts, and attributes.

### Import Resolution

Typical cached shape:

```text
O(I * (path_depth + candidate_extensions + condition_branches))
```

TS/JS has extra cost for `exports`/`imports`, tsconfig path patterns, project references, PnP maps, and package boundary checks. Python has `sys.path`, namespace packages, stubs, editable installs, and environment markers. Java has classpath/module path/source sets. Go import paths are easier once packages are loaded.

### Build Target Graphs

Static parse:

```text
O(build files + target declarations + declared edges)
```

Exact configured graph:

```text
build-tool specific incremental evaluation
```

Bazel/Skyframe is the state-of-the-art reference for registering dependency reads through evaluation keys. polint should copy the dependency-tracking lesson, not try to reimplement Bazel early.

## Product-Specific Insight

Traditional tools try to infer generic topology automatically. polint can aim higher because AI agents can write repo-local Rust extensions.

This changes the implementation:

- Unknown package roots are not just errors; they are extension opportunities.
- Unsupported Gradle/Bazel/Pants dynamic logic should be visible facts.
- Repo-specific aliases should not be forced into global config strings; an agent can write a provider.
- Architecture layers should be code-provided facts that rules can query.
- Extension facts must be benchmarked against default facts so improvements are measurable.

## Recommended Internal Invariants

1. Every package/root/source-set/dependency fact has provenance and precision.
2. Declared requirements and resolved edges are separate.
3. Import-to-package edges are separate from dependency declarations.
4. Lockfile exact facts record lockfile path and schema version.
5. Environment/condition-specific edges keep their conditions.
6. Dynamic or unsupported build logic emits explicit facts.
7. Extension-provided topology never silently becomes native topology.
8. Exact public SDK claims require exact or validated facts.
9. Cache keys include every file/config/lifecycle input read by a provider.
10. Multiple package managers in one repo are supported as multiple roots, not normalized away.

## What To Research Next

The next deep research topic should be **CFG and control dependence**, because:

- semantic indexes now define symbol/import identity;
- module graph defines which files/packages/source sets are in scope;
- framework entrypoint research defines callable boundaries;
- call graph/data-flow both need CFG as their local execution substrate.

CFG research should explicitly include exception control flow, async/await, generators, short-circuit logic, panic/throw/defer/finally, Java try-with-resources, Python context managers, JS promises, Go defer/panic/recover, and source-set-aware test/runtime variants.
