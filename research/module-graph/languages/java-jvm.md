# Java And JVM Package Graph Report

## Scope

Java/JVM support must model more than external dependencies. It must model:

- Maven projects and reactor modules;
- Gradle projects, source sets, configurations, variants, and capabilities;
- Bazel/Pants targets where present;
- classpath and module path;
- annotation processors and generated sources;
- test/source-set separation.

## State Of The Art

The JVM ecosystem's dependency graph is build-tool-shaped:

- Maven is mostly declarative: POMs, parent POMs, dependency management, BOM imports, scopes, exclusions, and nearest-wins mediation.
- Maven Resolver collects dependency descriptors, applies selectors and dependency management, expands version ranges, then transforms conflict sets.
- Gradle is a dynamic build platform: configurations, source sets, attributes, variants, capabilities, dependency substitution, metadata rules, plugins, and arbitrary code.
- Bazel and Pants operate on build targets and source ownership, not just packages.

## Tools To Support

| Tool | Must Support | Notes |
|---|---|---|
| Maven | `pom.xml`, multi-module reactors, dependency management, scopes, exclusions, BOM imports | Best first JVM target for native static support. |
| Gradle | `settings.gradle(.kts)`, `build.gradle(.kts)`, standard source sets, static dependency blocks where possible | Exact dynamic Gradle requires tool-reported or extension facts. |
| Bazel | `MODULE.bazel`, `WORKSPACE`, `BUILD`, target labels | Conservative target discovery first; exact configured graph is a separate tool/runtime problem. |
| Pants | `pants.toml`, BUILD files, target addresses, dependency inference | Useful for source ownership and generated targets. |

## OSS Implementation Evidence

Local repositories:

- `repos/maven` and `repos/maven-resolver`: Maven dependency collection and conflict resolution.
- `repos/gradle`: Gradle dependency graph builder, conflict/variant/capability resolution.
- `repos/bazel`: query, package, target, and Skyframe implementation.
- `repos/pants`: target graph and language dependency inference.

Key files:

- `maven-resolver/.../collect/DefaultDependencyCollector.java`
- `maven-resolver/.../transformer/ConflictResolver.java`
- `maven-resolver/.../transformer/NearestVersionSelector.java`
- `gradle/.../resolveengine/graph/builder/DependencyGraphBuilder.java`
- `gradle/.../ComponentResolutionState.java`
- `gradle/.../DefaultResolutionStrategy.java`
- `bazel/.../query2/query/GraphlessBlazeQueryEnvironment.java`
- `bazel/.../skyframe/TransitiveTargetFunction.java`
- `bazel/.../packages/Package.java`
- `pants/src/python/pants/engine/internals/graph.py`
- `pants/src/python/pants/backend/java/dependency_inference/`

## Maven Algorithm

```python
def collect_maven_dependencies(project):
    pom = parse_pom(project.pom)
    managed = collect_dependency_management(pom.parents, pom.boms, pom)
    graph = []
    queue = direct_dependencies(pom)
    while queue:
        dep = queue.pop()
        dep = apply_dependency_management(dep, managed)
        if excluded(dep):
            continue
        node = add_dependency_node(dep)
        queue.extend(read_pom_descriptor(dep).dependencies)
    return resolve_conflicts_nearest_wins(graph)
```

Need to preserve:

- scope;
- optionality;
- exclusions;
- dependency management origin;
- BOM imports;
- parent POM status;
- repository/profile status.

## Gradle Algorithm Tier

Static tier:

```python
def parse_gradle_static(project):
    settings = parse_included_projects(project.settings)
    for subproject in settings.projects:
        source_sets = standard_source_sets(subproject)
        deps = parse_obvious_dependency_blocks(subproject.build_file)
        emit_conservative_edges(subproject, deps)
        if dynamic_logic_seen(subproject.build_file):
            emit_unsupported_dynamic_build_script(subproject)
```

Exact tier later:

```python
def gradle_tool_reported_graph(project):
    report = run_or_read_gradle_model(project)
    return convert_report_to_source_sets_and_edges(report)
```

polint's native default should be honest: Gradle exactness from static parsing is not realistic for arbitrary repos.

## Bazel/Pants Target Tier

```python
def parse_build_targets(build_files):
    for file in build_files:
        for target in parse_static_targets(file):
            emit_build_target(target.label, target.kind, target.attrs)
            for label in static_label_attrs(target):
                emit_target_edge(target.label, label, precision="Conservative")
```

Exact configured targets require build-system evaluation. Bazel's Skyframe lesson is that every dependency read must be registered through the evaluation graph to preserve incrementality and correctness.

## Accuracy

High for:

- static Maven dependency declarations;
- Maven multi-module roots;
- Maven scopes/exclusions/dependency management when parents/BOMs are locally available;
- standard Gradle source-set directories;
- static Bazel/Pants target labels.

Lower or conditional for:

- Maven profiles and remote parents/BOMs;
- Maven plugin-generated sources;
- Gradle arbitrary build logic;
- Gradle variants/capabilities without executing Gradle;
- Bazel configured `select` and Starlark macro expansion;
- Pants plugin-generated targets and dependency inference without its engine.

## Complexity

- Maven POM parse: `O(pom bytes)`.
- Maven dependency collection: `O(nodes + edges + descriptor reads)`.
- Maven conflict resolution: current Maven Resolver recommends path-based conflict resolution with `O(N)` intent versus legacy `O(N^2)` worst case.
- Gradle exact resolution: build-tool-specific and can include metadata IO, dynamic script execution, conflict handling, and variant matching.
- Bazel configured graph: incremental Skyframe evaluation over keys and dependencies.

## Polint Implementation

Implement:

- native Maven static provider first;
- Gradle project/source-set static provider with explicit dynamic gaps;
- Bazel/Pants target discovery as conservative build-target facts;
- extension hooks for exact Gradle/Bazel/Pants graph facts.

Public SDK should avoid promising "the JVM dependency graph" until source-set and precision labels are stable.

## First Fixtures

- Maven single module with scopes and exclusions.
- Maven multi-module reactor with parent POM.
- Maven BOM import fixture.
- Gradle standard Java project with `main` and `test`.
- Gradle dynamic dependency fixture that emits unsupported dynamic fact.
- Bazel BUILD target with static deps.
- Pants target with dependency inference placeholder.
