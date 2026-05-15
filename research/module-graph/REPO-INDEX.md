# Repository Index

Third-party repositories were cloned under `research/module-graph/repos/`, which is gitignored. They are local research artifacts, not vendored dependencies.

## Cloned Repositories

| Repository | Commit | Why It Was Studied | Key Local Evidence |
|---|---:|---|---|
| <https://github.com/golang/go> | `9be7615aa247` | Go command module loading, MVS, vendor, and workspace behavior. | `src/cmd/go/internal/mvs/`, `src/cmd/go/internal/modload/` |
| <https://github.com/golang/mod> | `343ee60345a1` | Canonical `go.mod` and `go.work` parser/formatter. | `modfile/rule.go`, `module/module.go` |
| <https://github.com/golang/tools> | `a3954b5c7496` | gopls workspace roots, metadata, package loading, and import graph lifecycle. | `gopls/internal/cache/workspace.go`, `gopls/internal/cache/snapshot.go`, `gopls/internal/cache/metadata/metadata.go` |
| <https://github.com/microsoft/TypeScript> | `f350b5233149` | TypeScript module resolution, project references, compiler options, and package lookup. | `src/compiler/moduleNameResolver.ts`, `src/compiler/types.ts`, `src/compiler/tsbuildPublic.ts` |
| <https://github.com/oxc-project/oxc-resolver> | `374bc8e02c74` | Rust-native Node/TS resolver with tsconfig discovery, references, and path resolution. | `src/tsconfig_resolver.rs`, `src/tsconfig.rs`, `src/resolution.rs`, `src/options.rs` |
| <https://github.com/webpack/enhanced-resolve> | `d09bdb281efe` | Webpack-style resolver, package exports/imports, tsconfig paths plugin. | `lib/ResolverFactory.js`, `lib/TsconfigPathsPlugin.js`, `lib/ExportsFieldPlugin.js` |
| <https://github.com/npm/cli> | `c97b39b1e343` | npm Arborist ideal/actual/virtual tree, lockfile, peer placement, dependency edges. | `workspaces/arborist/lib/arborist/build-ideal-tree.js`, `node.js`, `edge.js`, `shrinkwrap.js`, `place-dep.js`, `can-place-dep.js` |
| <https://github.com/pnpm/pnpm> | `a62f959242b6` | pnpm workspace graph, workspace protocol, lockfile packages, strict dependency layout. | `workspace/projects-graph/src/index.ts`, `workspace/projects-reader/src/findPackages.ts`, `workspace/range-resolver/src/index.ts`, `lockfile/*` |
| <https://github.com/yarnpkg/berry> | `4287909fa6a0` | Yarn workspaces, lockfile resolver, PnP resolver/fetcher/linker architecture. | `packages/yarnpkg-core/sources/Project.ts`, `Workspace.ts`, `LockfileResolver.ts`, `WorkspaceResolver.ts`, `packages/yarnpkg-pnp/` |
| <https://github.com/oven-sh/bun> | `4d443e54022c` | Bun package manager, workspaces, lockfile, isolated/hoisted installs. | `src/install/PackageManager.zig`, `src/install/lockfile.zig`, `src/install/dependency.zig`, `src/install/npm.zig`, `src/install/pnpm.zig`, `src/install/yarn.zig` |
| <https://github.com/vercel/turborepo> | `aea4138686a9` | Package graph, task graph, lockfile abstraction, package manager detection. | `crates/turborepo-repository/src/package_graph/builder.rs`, `package_manager/`, `crates/turborepo-lockfiles/src/`, `turborepo-lib/src/task_graph/mod.rs` |
| <https://github.com/nrwl/nx> | `40420b0aec32` | Project graph, plugin-created dependencies, file map cache, affected analysis. | `packages/nx/src/project-graph/build-project-graph.ts`, `project-graph-builder.ts`, `nx-deps-cache.ts`, `plugins/` |
| <https://github.com/pypa/pip> | `f7bfe280f008` | pip's resolvelib adapter and install order graph. | `src/pip/_internal/resolution/resolvelib/resolver.py`, `provider.py`, `factory.py` |
| <https://github.com/sarugaku/resolvelib> | `a0cb7c50b780` | Generic Python dependency resolver with provider API, criteria, graph result, backtracking. | `src/resolvelib/resolvers/resolution.py`, `providers.py`, `structs.py` |
| <https://github.com/astral-sh/uv> | `a4d9e42197d6` | Modern Rust Python resolver, workspaces, PubGrub, universal marker-aware resolution, lockfiles. | `crates/uv-workspace/src/workspace.rs`, `crates/uv-resolver/src/resolver/mod.rs`, `crates/uv-resolver/src/pubgrub/`, `crates/uv-resolver/src/lock/` |
| <https://github.com/python-poetry/poetry> | `c04069d97f2f` | Poetry solver, marker aggregation, overrides, lockfile model. | `src/poetry/puzzle/solver.py`, `src/poetry/puzzle/provider.py`, `poetry.lock` |
| <https://github.com/pdm-project/pdm> | `904e1dded8f3` | PDM resolvelib integration, lock strategies, groups, markers. | `src/pdm/resolver/resolvelib.py`, `src/pdm/resolver/graph.py`, `src/pdm/project/lockfile.py` |
| <https://github.com/conda/conda> | `9ed9f1335e18` | Conda MatchSpec/PackageRecord model and solver lifecycle. | `conda/resolve.py`, `conda/core/solve.py`, `docs/source/dev-guide/deep-dives/solvers.md` |
| <https://github.com/microsoft/pyright> | `b13157b0fac4` | Python import resolution and environment/source-file model for later import-to-package mapping. | `packages/pyright-internal/src/analyzer/importResolver.ts`, `program.ts`, `sourceFile.ts` |
| <https://github.com/astral-sh/ty> | `a63e55929645` | Rust-native Python semantic/import model and uv-style project context. | `ty/ruff/crates/`, `ty_python_semantic/`, `pyproject.toml`, `uv.lock` |
| <https://github.com/apache/maven> | `cee3c33c74a8` | Maven project model and integration. | `pom.xml`, Maven core modules |
| <https://github.com/apache/maven-resolver> | `41f2c7b113b6` | Maven dependency collection, conflict resolution, selectors/managers/transformers. | `maven-resolver-impl/.../DefaultDependencyCollector.java`, `maven-resolver-util/.../ConflictResolver.java`, `NearestVersionSelector.java` |
| <https://github.com/gradle/gradle> | `58b2728482ab` | Gradle dependency graph builder, variants, capabilities, conflict resolution. | `DependencyGraphBuilder.java`, `ComponentResolutionState.java`, `DefaultResolutionStrategy.java`, variant/capability classes |
| <https://github.com/bazelbuild/bazel> | `c896d24a3114` | Bazel target graph, query/cquery, packages, Skyframe incremental dependency graph. | `src/main/java/.../query2/`, `packages/Package.java`, `skyframe/TransitiveTargetFunction.java`, `SkyQueryEnvironment.java` |
| <https://github.com/pantsbuild/pants> | `9ce66ef54006` | Pants target graph, source ownership, generated targets, dependency inference. | `src/python/pants/engine/internals/graph.py`, `backend/python/dependency_inference/`, `backend/java/dependency_inference/` |
| <https://github.com/rust-lang/cargo> | `4d1f984518c7` | Cargo workspaces, lockfile, resolver, feature and package identity reference. | `Cargo.toml`, `Cargo.lock`, resolver/workspace implementation files |

## Why These Are The Relevant Set

- **Language-native package systems:** Go modules, Cargo.
- **Node package managers and resolvers:** npm, pnpm, Yarn, Bun, TypeScript, Oxc resolver, enhanced-resolve.
- **Python package managers and solvers:** pip, resolvelib, uv, Poetry, PDM, conda.
- **JVM/build tools:** Maven, Maven Resolver, Gradle, Bazel, Pants.
- **Monorepo graph systems:** Nx, Turborepo.
- **Semantic import consumers:** Pyright, Ty, TypeScript, gopls.

This covers the practical state of the art for package, module, workspace, import, target, and project topology across the languages polint wants to support.

## Not Treated As Primary Sources

- Lerna, Rush, Lage, and Moon were not cloned in this pass because Nx/Turborepo/pnpm/Yarn cover the primary monorepo graph patterns needed for the first architecture.
- SBT was not cloned because Java/JVM support should begin with Maven and Gradle; Scala-specific build graph support can be researched later.
- Deno was not cloned because the user asked for popular package managers per current language targets; Deno is important but less central than npm/pnpm/Yarn/Bun for TS/JS repos polint is likely to scan first.
- Poetry-core, packaging, and installer were not separately cloned because pip/uv/Poetry/PDM plus official PyPA specs covered the required packaging concepts.
