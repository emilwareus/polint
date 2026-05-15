# Final Report: Module, Package, Dependency, And Repo Topology Graphs

Date: 2026-05-15

## Executive Decision

polint should implement a native Rust **module graph subsystem** that is broader than package dependency parsing and narrower than a full build tool clone.

The subsystem should build layered facts:

```text
workspace roots
  -> packages/projects/modules/source sets/build targets
  -> declared dependency requirements
  -> resolved dependency edges from lockfiles/native resolvers/tool reports
  -> import-to-package edges
  -> repo topology overlays
  -> extension facts
```

This is the right foundation for multi-language call graphs, data flow, entrypoints, semantic indexes, and AI-agent-authored rules. It also fits the product path: polint should provide strong defaults, explicit uncertainty, and a powerful Rust extension surface for repo-specific topology.

## What State Of The Art Actually Looks Like

There is no single state-of-the-art module graph. There are several overlapping graph engines:

| Area | Best References | What They Prove |
|---|---|---|
| Go modules | Go command, `x/mod`, `gopls` | MVS and workspace selection are relatively simple but lifecycle-sensitive. `go.work`, `replace`, vendoring, build tags, and multi-module workspaces matter. |
| TS/JS package managers | npm Arborist, pnpm, Yarn Berry/PnP, Bun, Turborepo | JS has many install layouts. Workspaces, lockfiles, peer contexts, `exports`/`imports`, tsconfig paths, and PnP all affect graph accuracy. |
| Python package managers | pip/resolvelib, uv, Poetry, PDM, conda | Requirements, extras, markers, groups, environments, URL/path/editable deps, lockfiles, and solver strategy are all first-class. |
| Java/JVM | Maven Resolver, Gradle, Bazel, Pants | Classpath/source-set/target/variant semantics dominate. Gradle and Bazel cannot be exactly reimplemented from static manifest snippets alone. |
| Build/monorepo graph engines | Bazel/Skyframe, Pants rules, Nx project graph, Turborepo package/task graph | Topology includes targets, generated sources, task graphs, visibility, plugins, cache keys, and affected-file analysis. |
| Package management research | Package Managers a la Carte, Dependency Solving Is Still Hard, Hypergraph Dependency Resolution, lockfile design-space research | Package resolution is formally diverse and can be computationally hard. A unified IR is useful, but exact semantics remain ecosystem-specific. |

## Core Finding

The wrong abstraction is:

```text
one PackageGraph { nodes, edges }
```

The right abstraction is:

```text
many topology fact layers
  with ecosystem-specific providers
  normalized into a shared typed contract
  preserving resolution source, conditions, and precision
  connected to semantic imports and build/lifecycle facts
```

Package-manager dependency graphs, source import graphs, build target graphs, and architectural topology graphs overlap but are not the same graph.

## Accuracy Lessons

### Go

Go is the easiest high-accuracy target. The official module reference defines MVS over a directed module graph and describes `go.work` as a workspace of main modules. `x/mod/modfile` gives a small, exact parser for `go.mod` and `go.work`. `gopls` shows the practical workspace layer: find `go.work`, parse `use`, include local `replace` module roots, cap file scanning, and carry module roots into package metadata.

The implementation lesson: support Go first as a full native resolver tier. Parse `go.mod`, `go.work`, `replace`, `exclude`, vendoring metadata, and module roots directly. Represent build tags and `include_tests` as lifecycle inputs.

### TS/JS

JavaScript and TypeScript are not one package manager:

- npm uses Arborist to build an ideal tree from package manifests and lockfiles, with hoisting and peer dependency placement.
- pnpm builds workspace graphs with `workspace:` protocol handling and uses a virtual store/symlink layout that protects undeclared dependencies.
- Yarn Berry can use Plug'n'Play, where `.pnp.cjs` is the dependency map instead of `node_modules`.
- Bun supports npm-style workspaces, `workspace:` ranges, its own lockfile, filters, isolated installs, and catalogs.
- TypeScript adds `baseUrl`, `paths`, project references, declaration output, and host-aware module resolution on top of runtime/package-manager behavior.

The implementation lesson: parse manifests and lockfiles natively, but separate install layout from declared/resolved package identity. Build a Node/TS resolver provider that can answer "what package does this import mean?" using package manager, tsconfig, Node `exports`/`imports`, file extensions, and PnP/virtual-store metadata.

### Python

Python dependency topology is environment-driven. PEP 508 names, extras, URL/path requirements, environment markers, dependency groups, Python version constraints, editable installs, lockfile formats, virtualenv site paths, and stubs all affect the graph.

pip uses resolvelib backtracking. uv uses PubGrub-style resolution, supports universal resolution across marker environments, and has explicit workspace discovery/cache-key logic. Poetry uses Mixology/PubGrub-style solving and marker aggregation. PDM builds on resolvelib and supports lock strategies/groups. Conda uses MatchSpec/PackageRecord models and SAT/libmamba solving over channel metadata.

The implementation lesson: native polint should parse common Python manifests and lockfiles first, not solve PyPI from scratch. Exact dependency solving can wait. For analysis, most repos have committed lockfiles or enough manifest data to build useful declared and selected topology facts. Environment markers and groups must be represented, not flattened away.

### Java/JVM

Java/JVM topology is build-system dominated:

- Maven has a comparatively static POM model, dependency management, scopes, exclusions, BOM imports, and nearest-wins mediation.
- Maven Resolver collects descriptors, applies selectors/managers, expands version ranges, then transforms the graph with conflict resolution. Current Maven Resolver includes a path-based conflict resolver with `O(N)` intent versus a legacy `O(N^2)` worst case.
- Gradle is more dynamic: dependency substitution, configurations, attributes, variants, capabilities, metadata downloads, and source sets. Its graph builder resolves conflicts during traversal and selects variants through attribute matching.
- Bazel and Pants model build targets, generated targets, visibility, source ownership, Starlark/plugin logic, and configured graphs.

The implementation lesson: Maven static POM support is a reasonable native target. Gradle/Bazel/Pants exactness needs a lower precision default plus optional tool-reported or extension-provided facts. Do not claim exact Gradle/Bazel graphs from static parsing alone.

### Monorepos

Nx and Turborepo are important because they show what product users expect from "repo graph":

- project nodes;
- internal and external dependency nodes;
- package-manager-driven workspace graph;
- source-file-to-project ownership;
- plugin-created dependencies;
- affected-file caching;
- task graph layered above package graph.

polint should not become a task runner, but it should copy their distinction between package/project graph and task/build graph.

## Research Takeaways

1. **Package manager support is a multi-phase feature.** Manifest parsing gives declared edges; lockfile parsing gives selected edges; import resolution gives actual source usage; build-tool query gives target/source-set precision.
2. **Precision must be per-edge.** One package can have an exact lockfile edge, a heuristic import edge, and an unsupported Gradle variant edge in the same run.
3. **Root detection is product-critical.** Wrong roots poison every downstream fact. Use explicit root files first, then nearest-root discovery, then heuristics.
4. **Native implementation does not mean pretending dynamic tools are static.** A native Rust core can parse and model static facts while leaving dynamic build script exactness to future native interpreters, tool-reported adapters, or repo-local Rust extensions.
5. **Extensions are not optional for max capability.** Repo-specific package aliases, generated source, custom module maps, internal deployment units, ownership zones, and architecture layers should be first-class extension facts.
6. **Lockfile research supports a layered design.** Lockfiles are reproducibility and cache artifacts, but each manager encodes different semantics. The fact model must store lockfile source and schema version.
7. **Dependency solving research argues against one early universal solver.** A unified dependency IR is useful, but fully solving npm/pip/conda/Gradle/Maven exactly is a long-term goal. For static analysis, exact lockfile readers create more value earlier.

## Recommended Polint Design

### 1. Build Native Providers By Ecosystem

```text
polint.module_graph.discovery
polint.module_graph.manifest
polint.module_graph.lockfile
polint.module_graph.requirements
polint.module_graph.resolved_edges
polint.module_graph.import_resolution
polint.module_graph.source_sets
polint.module_graph.topology
polint.module_graph.extension_merge
```

Every provider emits typed facts and sidecar metadata:

- provider id/version;
- source files read;
- lifecycle inputs;
- fact schema version;
- precision/confidence;
- cache key;
- validation status.

### 2. Implement Support In This Order

1. Cross-language root discovery, manager detection, manifest parsing, and fact schema.
2. Go modules: `go.mod`, `go.work`, local `replace`, `exclude`, `go.sum`, vendor markers, build tags as lifecycle inputs.
3. TS/JS: `package.json`, npm/pnpm/Yarn/Bun workspace discovery, lockfile readers, Node package names, `exports`/`imports`, tsconfig paths/project references.
4. Python: `pyproject.toml`, requirements files, uv/Poetry/PDM lockfiles, dependency groups, PEP 508 markers/extras, editable/path deps.
5. Java/JVM static tier: Maven POMs, Maven scopes/dependency management/BOMs, Gradle project/source-set discovery with conservative precision.
6. Monorepo overlays: Nx, Turborepo, Pants/Bazel target discovery where static, plus extension hooks.
7. Tool-reported validation adapters and optional exact dynamic-tool mode later.

### 3. Expose Public Views Only After Internal Facts Stabilize

Do not expose raw graph internals. Public SDK should use typed views:

```rust
#[polint::rule]
fn no_cross_layer_dependency(
    ctx: &mut RuleCtx<'_>,
    packages: Packages<'_>,
    deps: Dependencies<'_>,
    topology: RepoTopology<'_>,
) -> RuleResult {
    for edge in deps.resolved_edges() {
        if edge.is_exact_or_validated() && topology.violates_layer_policy(edge) {
            ctx.diagnostic(...);
        }
    }
    Ok(())
}
```

### 4. Make Unknowns Actionable

Examples of facts that should be emitted, not hidden:

```text
UnsupportedDynamicBuildScript: Gradle build logic read but not executed
MissingLockfile: package manager detected but lockfile absent
AmbiguousWorkspaceRoot: package.json workspace and pnpm-workspace disagree
UnresolvedImportTarget: tsconfig path maps to multiple package roots
GeneratedSourceUnknown: source root references generated directory not produced by known provider
ExternalPackageUnresolved: dependency declared but selected version unknown
```

These are integration tasks for agents and extension authors.

### 5. Treat Repo Topology As A First-Class Layer

polint's value is repo-specific policy. Package manager edges alone do not know:

- ownership;
- deploy units;
- domain boundaries;
- architecture layers;
- generated zones;
- test-only visibility;
- internal API/public API boundaries;
- source-of-truth directories.

These should be modeled as `RepoTopologyFact`, usually from config or repo-local Rust extensions.

## What To Avoid

- Do not build a single graph type and force all ecosystems into it.
- Do not claim exact Gradle/Bazel/Pants resolution from static parsing.
- Do not depend on external package managers as mandatory runtime dependencies for normal scans.
- Do not hide missing lockfiles, unsupported dynamic scripts, peer-context ambiguity, or marker/environment splits.
- Do not expose package-manager-specific internals as the public SDK.
- Do not collapse declared dependencies and actually imported dependencies.
- Do not flatten Python markers, JS peer contexts, Gradle variants, or Java source sets into unconditioned edges.

## Final Recommendation

Build a native Rust module graph as the next implementation substrate after the analysis kernel:

```text
Roots + Packages + SourceSets + Requirements + ResolvedEdges + ImportToPackage + RepoTopology
```

Use exact native parsing where formats are static. Use explicit lower-precision facts where formats are dynamic. Let agent-authored Rust extensions fill the repo-specific and framework-specific gaps. This gives polint the highest ceiling without baking false precision into the foundation.
