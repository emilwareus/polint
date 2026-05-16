# Algorithm: Workspace Discovery

## Goal

Find all analysis roots without assuming one repository root, one package manager, or one language.

## Core Algorithm

```python
DEFAULT_PRUNE = {
    ".git", "node_modules", ".pnpm-store", ".yarn/cache",
    ".venv", "venv", "__pycache__", ".tox", ".mypy_cache",
    "target", "dist", "build", ".gradle", ".nx", ".turbo",
}

ROOT_SIGNALS = {
    "go.mod": "go-module",
    "go.work": "go-workspace",
    "package.json": "js-package",
    "pnpm-workspace.yaml": "pnpm-workspace",
    "pyproject.toml": "python-project",
    "requirements.txt": "python-requirements",
    "pom.xml": "maven-project",
    "settings.gradle": "gradle-root",
    "settings.gradle.kts": "gradle-root",
    "MODULE.bazel": "bazel-root",
    "WORKSPACE": "bazel-root",
    "Cargo.toml": "cargo-package",
    "pants.toml": "pants-root",
    "nx.json": "nx-root",
    "turbo.json": "turbo-root",
}

def discover_roots(repo_root, config):
    signals = []
    for dir in walk_dirs(repo_root, prune=DEFAULT_PRUNE | config.prune):
        for filename, kind in ROOT_SIGNALS.items():
            if exists(dir / filename):
                signals.append(RootSignal(kind, dir, dir / filename))

    explicit = apply_explicit_config(signals, config)
    grouped = group_nested_roots(explicit or signals)
    roots = []
    for group in grouped:
        roots.extend(resolve_group_to_workspace_roots(group))
    return stable_sort(deduplicate(roots))
```

## Root Grouping Rules

```python
def resolve_group_to_workspace_roots(group):
    if group.has("go.work"):
        return go_work_use_roots(group.go_work)

    if group.has("pnpm-workspace.yaml"):
        return [pnpm_workspace_root(group.path)]

    if group.package_json_has_workspaces():
        return [js_workspace_root(group.path)]

    if group.has("settings.gradle") or group.has("settings.gradle.kts"):
        return [gradle_root(group.path)]

    if group.has("pom.xml") and pom_has_modules(group.pom):
        return [maven_reactor_root(group.path)]

    if group.has("pyproject.toml") and pyproject_has_uv_workspace(group.pyproject):
        return [uv_workspace_root(group.path)]

    return [single_project_root(signal) for signal in group.primary_signals()]
```

## Precision Rules

| Situation | Fact |
|---|---|
| Explicit config root exists | `Exact` |
| Workspace file declares members | `ManifestExact` |
| Single manifest found | `ManifestExact` |
| Multiple conflicting root signals | emit `AmbiguousWorkspaceRoot` |
| Nested workspace unsupported by manager | emit `UnsupportedNestedWorkspace` |
| Build file with no root file | emit `HeuristicRootCandidate` |

## Cache Inputs

Root discovery cache key includes:

- repo root path;
- ignore/prune config;
- list and digest of root signal files;
- explicit root config;
- provider version;
- fact schema version.

## Why Not One Root?

Large repos often contain:

- multiple Go modules;
- examples with their own `package.json`;
- Python tools under `scripts/`;
- Java services and JS frontends;
- Bazel root plus language package managers;
- vendored or fixture repos.

Forcing one root would create false package ownership and false dependency edges.
