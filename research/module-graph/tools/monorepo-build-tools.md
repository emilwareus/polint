# Monorepo And Build Tool Graphs

## Why This Is Separate From Package Managers

Package managers answer:

```text
Which packages are declared or selected as dependencies?
```

Build/monorepo tools answer:

```text
Which projects, source sets, targets, generated files, tasks, owners, and changed-file impacts exist?
```

For polint, both matter. Rules about dependency boundaries, generated code, entrypoints, call graphs, and data flow need package edges and build/source ownership.

## Tools Studied

| Tool | Graph Model | Key Lesson |
|---|---|---|
| Bazel | Packages, targets, configured targets, Skyframe keys/values, query/cquery/aquery | Exact graph requires evaluation and registered dependencies. Static BUILD parsing is only a lower tier. |
| Pants | Addresses, targets, source ownership, dependency inference, rule engine | Source ownership and inferred dependencies are first-class. |
| Nx | Project graph, external nodes, plugins, file map cache, affected graph | Project graph needs plugin-created edges and cache invalidation. |
| Turborepo | Package graph from package manager plus task graph from `turbo.json` | Package graph and task graph are distinct layers. |
| Maven | Reactor modules and dependency graph | Static declarative multi-module model. |
| Gradle | Projects, configurations, source sets, variant-aware dependency graph | Exactness depends on executing build logic and variant selection. |

## Bazel

Bazel's query docs distinguish:

- query: post-loading target graph, not configured;
- cquery: configured target graph;
- Sky Query: introspects Skyframe graph.

Skyframe's core lesson is essential for polint's analysis kernel: a computed value is keyed by a `SkyKey`; functions request dependencies through the environment; reading hidden inputs directly creates invalid incremental builds.

Polint implication:

- static Bazel BUILD parsing can emit conservative `BuildTargetFact`;
- exact configured target facts require tool-reported adapter or future native Starlark/config evaluator;
- every build-system input read by a provider must be in the cache key.

## Pants

Pants models:

- target addresses;
- generated targets;
- source ownership;
- language-specific dependency inference;
- rule-engine requests and results.

Polint implication:

- copy the source ownership model;
- represent inferred dependencies separately from explicitly declared dependencies;
- treat language-specific dependency inference as extension/provider layer.

## Nx

Nx project graph construction uses:

- project configurations;
- file maps;
- external nodes;
- plugins;
- cache invalidation over package deps, projects, nx config, root tsconfig, and external node hashes;
- partial graph errors.

Polint implication:

- project graph building needs a plugin/extension model;
- partial graph errors should not discard useful facts;
- affected-file analysis is a useful future rule accelerator.

## Turborepo

Turborepo separates:

- package graph: internal package dependencies from the package manager;
- task graph: task dependencies from `turbo.json`;
- lockfile/package-manager abstractions.

Polint implication:

- keep `PackageFact` and `Task/BuildTargetFact` separate;
- do not encode task graph semantics inside dependency edges.

## Recommended Polint Facts

```text
BuildTargetFact {
  id,
  label_or_address,
  package,
  kind,
  source_files,
  generated_outputs,
  declared_deps,
  inferred_deps,
  configuration,
  precision,
  provenance,
}

ProjectGraphFact {
  project_package,
  project_root,
  files,
  external_nodes,
  plugin_edges,
  cache_inputs,
  precision,
}

TaskGraphFact {
  project,
  task_name,
  depends_on_tasks,
  inputs,
  outputs,
  precision,
}
```

## Algorithm

```python
def build_monorepo_overlays(repo, packages):
    overlays = []
    if exists("nx.json"):
        overlays += parse_nx_project_graph_inputs(repo, packages)
    if exists("turbo.json"):
        overlays += parse_turbo_package_and_task_inputs(repo, packages)
    if exists("pants.toml"):
        overlays += parse_pants_targets_conservatively(repo)
    if exists("MODULE.bazel") or exists("WORKSPACE"):
        overlays += parse_bazel_targets_conservatively(repo)
    return overlays
```

## Precision

| Fact Source | Precision |
|---|---|
| Static project config | `ManifestExact` |
| Static package-manager package graph | `ManifestExact` or `LockfileExact` |
| Static BUILD target labels | `Conservative` |
| Tool-reported graph | `ToolReported` |
| Repo-local Rust provider | `ExtensionAsserted` or `ExtensionValidated` |
| Dynamic build script recognized but not executed | `Unsupported` |

## Validation

Compare against:

- `nx graph`/project graph JSON where available;
- `turbo run --graph` where available;
- `bazel query` and `bazel cquery` on small fixtures;
- `pants dependencies`/`pants peek` on small fixtures;
- Maven/Gradle dependency reports for tool-reported validation only.
