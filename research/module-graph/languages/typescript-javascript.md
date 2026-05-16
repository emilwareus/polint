# TypeScript And JavaScript Package Graph Report

## Scope

TS/JS support must cover the most common package managers and resolver inputs:

- npm;
- pnpm;
- Yarn classic and Yarn Berry/PnP;
- Bun;
- `package.json`;
- lockfiles;
- workspaces;
- `node_modules` layout;
- package `exports` and `imports`;
- TypeScript `baseUrl`, `paths`, `rootDirs`, and project references;
- CommonJS, ESM, and dynamic import status.

## State Of The Art

The ecosystem has two distinct layers:

1. **Package-manager graph:** packages, workspace members, dependency ranges, peer dependencies, selected versions, install layout, and lockfile records.
2. **Runtime/compiler import resolution:** given a specifier in a file, find the module/package it means under Node, bundler, PnP, tsconfig, or package `exports` semantics.

The TypeScript documentation says TypeScript follows Node's `imports` and self-reference resolution until a file path is resolved, then TypeScript-specific extension and declaration lookup applies. TypeScript project references split programs into referenced projects whose outputs and declarations affect imports.

## Package Managers To Support

| Manager | Must Support | Notes |
|---|---|---|
| npm | `package.json`, workspaces, `package-lock.json`, Arborist-style dependency sections | npm workspaces symlink packages into root `node_modules`. |
| pnpm | `pnpm-workspace.yaml`, `pnpm-lock.yaml`, `workspace:` protocol, virtual store, peer contexts | Strict dependency access and symlink layout mean physical paths are not enough. |
| Yarn v1 | `yarn.lock`, package workspaces | Common in older repos. |
| Yarn Berry | `yarn.lock`, `.yarnrc.yml`, `.pnp.cjs`, workspaces | PnP has no `node_modules`; `.pnp.cjs` is the dependency map. |
| Bun | `bun.lock`, `package.json#workspaces`, `workspace:` protocol, filters/catalogs | Bun supports npm-style workspaces plus Bun-specific lock/install semantics. |

## OSS Implementation Evidence

Local repositories:

- `repos/npm-cli`: npm Arborist ideal tree, virtual/actual trees, edges, peer placement.
- `repos/pnpm`: workspace graph, workspace range resolver, lockfile readers.
- `repos/yarn-berry`: Project/Workspace/Resolver/LockfileResolver/PnP.
- `repos/bun`: Zig package manager and lockfile implementation.
- `repos/TypeScript`: compiler module resolver and project references.
- `repos/oxc-resolver`: Rust-native Node/TS resolver, tsconfig discovery and references.
- `repos/enhanced-resolve`: webpack resolver and tsconfig paths plugin behavior.
- `repos/turborepo`: package graph and package-manager abstraction.

Key files:

- `npm-cli/workspaces/arborist/lib/arborist/build-ideal-tree.js`
- `npm-cli/workspaces/arborist/lib/node.js`
- `npm-cli/workspaces/arborist/lib/edge.js`
- `pnpm/workspace/projects-graph/src/index.ts`
- `yarn-berry/packages/yarnpkg-core/sources/Project.ts`
- `yarn-berry/packages/yarnpkg-core/sources/Workspace.ts`
- `yarn-berry/packages/yarnpkg-core/sources/LockfileResolver.ts`
- `yarn-berry/packages/yarnpkg-core/sources/WorkspaceResolver.ts`
- `oxc-resolver/src/tsconfig_resolver.rs`
- `enhanced-resolve/lib/ResolverFactory.js`
- `enhanced-resolve/lib/TsconfigPathsPlugin.js`
- `turborepo/crates/turborepo-repository/src/package_graph/builder.rs`

## Algorithm

```python
def detect_js_manager(root):
    if package_json.packageManager:
        return parse_package_manager_field(package_json.packageManager)
    if exists("pnpm-lock.yaml") or exists("pnpm-workspace.yaml"):
        return "pnpm"
    if exists("yarn.lock") or exists(".yarnrc.yml"):
        return "yarn"
    if exists("bun.lock") or exists("bun.lockb"):
        return "bun"
    if exists("package-lock.json") or exists("npm-shrinkwrap.json"):
        return "npm"
    return "npm-compatible"

def discover_js_workspaces(root, manager):
    if manager == "pnpm":
        patterns = parse_yaml("pnpm-workspace.yaml").packages
    else:
        patterns = parse_package_json(root).workspaces
    return expand_workspace_patterns(patterns, pruned_dirs=DEFAULT_IGNORES)

def build_js_declared_graph(workspaces):
    packages = index_by_name_and_path(workspaces)
    for pkg in packages:
        for dep in all_dependency_sections(pkg.package_json):
            emit_requirement(pkg, dep)
            if dep.uses_workspace_protocol():
                emit_internal_candidate(pkg, packages.lookup(dep.name))
```

Import resolution:

```python
def resolve_ts_js_import(specifier, from_file, context):
    if is_relative(specifier):
        return resolve_file_directory_or_package_json(from_file, specifier, context)
    if context.tsconfig.paths_match(specifier):
        return resolve_tsconfig_path(specifier, context)
    if context.pnp:
        return resolve_yarn_pnp(specifier, owner_package(from_file), context)
    package_name, subpath = split_package_name(specifier)
    package = resolve_package_from_manager(package_name, owner_package(from_file), context)
    return apply_exports_imports_and_conditions(package, subpath, context)
```

## Accuracy

High for:

- static `package.json` dependencies and workspaces;
- lockfile-selected versions where schema is supported;
- internal workspace package links;
- TypeScript paths/project references when config is valid;
- package `exports`/`imports` with known conditions.

Lower or conditional for:

- bundler plugins;
- custom Babel/Vite/Webpack aliases;
- dynamic `require`;
- generated package manifests;
- peer dependency contexts when collapsed;
- hoisted undeclared dependencies;
- package manager hooks or patching.

## Complexity

- Workspace discovery: `O(candidate dirs + workspace patterns)`.
- Manifest parse: `O(package.json bytes)`.
- Lockfile parse: `O(lockfile entries + edges)`.
- Internal workspace graph: `O(P + R)`, with version/range matching.
- Import resolution: `O(path depth * candidate extensions * condition branches)`, reduced by caches.
- PnP map lookup: approximately `O(1)` or `O(log P)` depending on representation after `.pnp.cjs` is interpreted or parsed.

## Polint Implementation

Implement:

- native `package.json` parser;
- package-manager detector;
- workspace glob expander;
- lockfile readers for npm, pnpm, Yarn, Bun;
- Node package specifier parser;
- `exports`/`imports` resolver;
- tsconfig resolver;
- package ownership index from source files to workspace packages.

Defer:

- full npm/pnpm/Yarn/Bun solving from registry metadata;
- arbitrary bundler plugin execution;
- exact dynamic `require` resolution.

## Extension Hooks

Repo-local Rust providers should add:

- custom aliases;
- Vite/Webpack/Turbo/Nx generated module maps;
- internal package boundaries;
- generated packages;
- monorepo deployment-unit tags;
- validated fixes for unresolved imports.
