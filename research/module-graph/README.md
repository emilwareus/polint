# Module, Package, Dependency, And Repo Topology Graph Research

Date: 2026-05-15

This folder researches how polint should model package managers, workspace roots, lockfiles, import resolution, build-system targets, monorepo project graphs, and repo topology.

The core conclusion is:

```text
Do not build "one dependency graph."
Build layered topology facts:
  workspace roots
  packages/projects/modules
  declared requirements
  resolved dependency edges
  source-set/build-target edges
  import-to-package edges
  repo architecture overlays
with explicit provenance, precision, cache keys, and extension merges.
```

## Deliverables

| File | Purpose |
|---|---|
| `FINAL-REPORT.md` | Main synthesis and executive recommendation. |
| `RECOMMENDED_IMPLEMENTATION.md` | Concrete native Rust implementation path for polint. |
| `STANDARD.md` | Normalized vocabulary and fact model. |
| `REPO-INDEX.md` | OSS repositories cloned and implementation files inspected. |
| `PAPER-INDEX.md` | Research papers, official docs, and local paper artifacts. |
| `RESEARCH-ANALYSIS.md` | Accuracy, complexity, algorithm, and product-fit analysis. |
| `VALIDATION.md` | Test, benchmark, oracle, and regression plan. |
| `languages/*.md` | Language/ecosystem-specific reports. |
| `tools/*.md` | Package-manager, resolver, and build/monorepo tool reports. |
| `algorithms/*.md` | Python-ish pseudo-code for core algorithms. |
| `implementation/native-rust-path.md` | Internal module layout and staged build plan. |
| `oss/implementation-comparison.md` | Comparative table across inspected systems. |
| `benchmarks/evaluation-plan.md` | Benchmarks and fixture strategy. |
| `decisions/DECISIONS.md` | Decision log and rejected alternatives. |

Third-party repositories are cloned in `research/module-graph/repos/`, which is gitignored. Research papers are downloaded in `research/module-graph/papers/`.

## State Of The Art Today

The mature systems agree on a few things:

- **Package-manager semantics are ecosystem-specific.** Go MVS, npm Arborist, pnpm workspace protocols and virtual store, Yarn PnP, Cargo feature unification, Maven nearest-wins conflict mediation, Gradle variant/capability matching, pip/resolvelib backtracking, uv PubGrub, Poetry Mixology, and Conda SAT solving do not reduce cleanly to one small resolver.
- **Lockfiles are not interchangeable.** They differ in what they record: selected versions, package locations, integrity, peer contexts, groups, markers, environments, variants, and sometimes not enough graph structure.
- **Build systems are topology engines.** Bazel, Pants, Nx, Turborepo, Maven, Gradle, and Go workspaces model projects, source sets, targets, generated code, task dependencies, and visibility. They are not just package managers.
- **Import resolution bridges semantic indexes and package graphs.** A dependency edge says "package A may use package B"; an import edge says "this file actually references this package/module under these conditions."
- **Dynamic build logic cannot be perfectly reimplemented from manifests alone.** Gradle scripts, Bazel/Starlark macros, Pants plugins, package-manager hooks, Python editable installs, generated source, and custom monorepo conventions need explicit precision tiers and extension facts.
- **The agent-extensible product path changes the design.** polint should expose exact facts, unresolved facts, and dynamic gaps so an AI agent can add repo-local Rust providers for custom workspaces, generated packages, framework module aliases, deployment units, or boundary rules.

## Recommended Shape

```text
repository files
  -> discovery roots
  -> manager/lifecycle detection
  -> manifests and lockfiles
  -> package/project/source-set facts
  -> declared dependency requirements
  -> lockfile/tool-reported/native-resolved edges
  -> import-to-package resolution
  -> build-target and generated-source overlays
  -> repo topology/ownership/layer facts
  -> extension merge and validation
  -> typed SDK views
```

The first public-facing views should be conservative:

- `Packages<'_>`: packages/projects/modules/workspace members with manager and precision.
- `Dependencies<'_>`: declared and resolved edges with kind, scope, status, and provenance.
- `Imports<'_>`: source import facts once semantic-index import facts are stable.
- `RepoTopology<'_>` later: architecture layers, deploy units, generated zones, and ownership when validated.

## Fit With Existing Research

This track consumes:

- `research/semantic-index/`: imports, symbols, references, resolution facts.
- `research/analysis-kernel/`: fact layers, provider DAG, provenance, cache keys, extension merge.
- `research/evaluation-harness/`: default-vs-extension metrics and benchmark schema.
- `research/framework-entrypoints/`: framework lifecycle and generated dispatch as topology overlays.
- `research/call-graphs/`: package/module boundaries and external call targets.
- `research/data-flow/`: source/sink reachability needs dependency and import boundaries.

It feeds:

- CFG and control dependence: source-set/build-target selection determines which files exist.
- Type/value/alias analysis: classpaths, module paths, stubs, generated code, and external packages are inputs.
- Call graph/data-flow implementation: cross-package call and flow needs package/import topology.
