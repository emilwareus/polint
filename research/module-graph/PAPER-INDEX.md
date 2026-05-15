# Paper And Technical Source Index

Papers were downloaded under `research/module-graph/papers/` when available. Official documentation pages and source repositories are linked directly.

## Downloaded Research Papers

| Local File | Source | Relevance |
|---|---|---|
| `papers/package-managers-a-la-carte-2026.pdf` | <https://arxiv.org/abs/2602.18602> | Formal package-calculus model of dependency resolution across package managers. Supports a common IR with ecosystem-specific extensions. |
| `papers/dependency-solving-still-hard-2020.pdf` | <https://arxiv.org/abs/2011.07851> | Reviews dependency solving as a separate concern and the use of SAT/PBO/MILP and related techniques. Supports not building a universal solver first. |
| `papers/lockfile-design-space-2025.pdf` | <https://arxiv.org/abs/2505.04834> | Empirical study of lockfiles across npm, pnpm, Cargo, Poetry, Pipenv, Gradle, and Go. Supports lockfile-specific parsers and schema-aware facts. |
| `papers/hypergraph-dependency-resolution-2025.pdf` | <https://arxiv.org/abs/2506.10803> | Recent dependency resolution formulation using hypergraphs. Relevant to future common solver/IR work, not first implementation. |
| `papers/build-systems-a-la-carte.pdf` | <https://simon.peytonjones.org/assets/pdfs/build-systems-original.pdf> | Build system theory comparing dependency tracking, scheduling, and incrementality. Relevant to Bazel/Pants/analysis-kernel scheduling. |

## Official Technical Sources

| Source | Relevance |
|---|---|
| Go modules reference: <https://go.dev/ref/mod> | MVS, module graph, `replace`/`exclude`, workspaces, vendoring. |
| Go packages API: <https://pkg.go.dev/golang.org/x/tools/go/packages> | Package loading and metadata model. |
| TypeScript module resolution reference: <https://www.typescriptlang.org/docs/handbook/modules/reference> | Node/TS module resolution, `exports`, `imports`, package lookup. |
| TypeScript project references: <https://www.typescriptlang.org/docs/handbook/project-references.html> | TS project graph, declaration outputs, build mode. |
| npm workspaces: <https://docs.npmjs.com/cli/v8/using-npm/workspaces/> | npm workspace declaration and symlink behavior. |
| npm package-lock docs: <https://docs.npmjs.com/cli/configuring-npm/package-lock-json> | npm lockfile behavior and schema. |
| pnpm workspaces: <https://pnpm.io/workspaces> | pnpm workspace root, `workspace:` protocol, cycle behavior, link settings. |
| pnpm symlinked node_modules: <https://pnpm.io/symlinked-node-modules-structure> | Virtual store and strict dependency access. |
| Yarn workspaces: <https://yarnpkg.com/features/workspaces> | Yarn workspace model. |
| Yarn Plug'n'Play: <https://yarnpkg.com/features/pnp> | PnP dependency map and ghost dependency prevention. |
| Bun workspaces: <https://bun.sh/docs/pm/workspaces> | Bun workspace and lockfile conventions. |
| uv resolver internals: <https://docs.astral.sh/uv/reference/internals/resolver/> | PubGrub-style Python resolution, URL dependencies, universal resolution. |
| uv workspaces: <https://docs.astral.sh/uv/concepts/projects/workspaces/> | Python workspace discovery and member semantics. |
| Python dependency specifiers: <https://packaging.python.org/en/latest/specifications/dependency-specifiers/> | PEP 508 names, extras, markers, URLs. |
| Python dependency groups: <https://packaging.pypa.io/en/stable/dependency_groups.html> | Group include expansion, cycle detection, normalized groups. |
| pip dependency resolution: <https://pip.pypa.io/en/stable/topics/dependency-resolution/> | pip's resolver behavior and backtracking. |
| conda-libmamba-solver docs: <https://conda.github.io/conda-libmamba-solver/> | Conda's default libmamba solver status. |
| Maven dependency mechanism: <https://maven.apache.org/guides/introduction/introduction-to-dependency-mechanism.html> | Transitive dependencies, scopes, dependency management, nearest definition. |
| Maven Resolver transitive dependency resolution: <https://maven.apache.org/resolver/transitive-dependency-resolution.html> | Descriptor collection, selectors, managers, version range expansion. |
| Gradle variant-aware resolution: <https://docs.gradle.org/current/userguide/variant_aware_resolution.html> | Attributes, variants, matching algorithm. |
| Bazel query language: <https://docs.bazel.build/versions/main/query.html> | Query, cquery, Sky Query, target graph semantics. |
| Bazel Skyframe: <https://preview.bazel.build/reference/skyframe> | Incremental dependency graph and registered reads. |
| Bazel dependency concepts: <https://bazel.google.cn/concepts/dependencies?hl=en> | Explicit dependencies and limits of missing-dependency checks. |
| Nx project graph docs: <https://nx.dev/docs/features/explore-graph> | Project graph and dependency chain exploration. |
| Nx graph plugin docs: <https://canary.nx.dev/docs/extending-nx/project-graph-plugins> | Plugin-created project graph nodes/edges. |
| Turborepo package/task graphs: <https://turborepo.com/repo/docs/core-concepts/package-and-task-graph> | Package graph vs task graph distinction. |
| Cargo resolver: <https://doc.rust-lang.org/cargo/reference/resolver.html> | Cargo resolver pseudo-code, lockfile result, constraints/heuristics. |
| Cargo workspaces: <https://doc.rust-lang.org/cargo/reference/workspaces.html> | Workspace members, shared lockfile, root-only patch/replace/profile behavior. |

## Research Takeaways

1. Dependency resolution is formally diverse and sometimes computationally hard.
2. Lockfiles are high-value exact inputs, but their schemas and semantics vary by manager.
3. Build systems are separate graph engines with target/source-set/task semantics.
4. Native parsing can deliver strong default facts without mandatory external package-manager execution.
5. A common polint fact model should preserve ecosystem-specific conditions instead of flattening them.
