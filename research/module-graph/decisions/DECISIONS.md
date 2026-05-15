# Decision Log

## D1: Build Layered Facts, Not One Graph

Decision: represent module/package/topology as typed fact layers.

Reason:

- package dependencies, source imports, build targets, and architecture topology answer different questions;
- a single graph would either drop precision or grow untyped edge metadata;
- analysis-kernel research already recommends typed fact layers.

## D2: Native Parsers First, Solvers Later

Decision: parse manifests and lockfiles natively before implementing full npm/pip/conda/Gradle solvers.

Reason:

- committed lockfiles give high value quickly;
- general dependency solving is hard and ecosystem-specific;
- external tools can validate our facts without becoming required runtime dependencies.

## D3: Go Gets First Full Native Resolver

Decision: implement Go MVS and workspace support first.

Reason:

- Go module semantics are documented, deterministic, and relatively small;
- current polint already supports Go syntax;
- `gopls` and `x/mod` provide clear implementation references.

## D4: Gradle/Bazel/Pants Default To Conservative Static Facts

Decision: do not claim exact dynamic build graphs from static parsing.

Reason:

- Gradle build scripts execute arbitrary logic;
- Bazel configured targets require Starlark/config evaluation;
- Pants dependency inference is rule-engine/plugin-driven;
- false exactness would damage downstream call graph/data-flow/rule accuracy.

## D5: Extension Providers Are First-Class

Decision: repo-local Rust providers can add roots, packages, source sets, resolved edges, import mappings, generated facts, and topology facts.

Reason:

- polint's core user is an AI agent that can inspect a repo and write code;
- repo-specific topology is often more accurate than generic inference;
- max capability requires a high-ceiling integration surface.

## D6: Declared And Resolved Edges Stay Separate

Decision: `DependencyRequirementFact` and `ResolvedDependencyFact` are separate fact families.

Reason:

- manifests declare constraints;
- lockfiles/solvers select concrete package instances;
- source imports may use only a subset of declared deps;
- rules need to distinguish missing declarations from transitive selected packages.

## D7: Conditions Are Preserved

Decision: do not flatten Python markers, JS peer contexts, Gradle variants, Java source sets, Go build tags, or Cargo target-specific deps.

Reason:

- flattening creates false edges;
- downstream data-flow and call graphs need selected lifecycle context;
- agents can reason better from explicit conditions.

## Rejected Alternatives

### Use External Package Managers Directly For Everything

Rejected for default mode. External tools are useful validation oracles and optional providers, but mandatory execution would make scans slower, less deterministic, harder to cache, and dependent on installed toolchains.

### Build A Universal SAT Solver First

Rejected for early implementation. Research supports a common dependency IR, but package managers vary in peers, features, virtual packages, variants, markers, source policies, and installed-state semantics. Lockfile readers and native ecosystem tiers produce more value sooner.

### Treat `node_modules` As Truth

Rejected. pnpm and Yarn PnP break this assumption, hoisting can hide undeclared dependencies, and source import rules depend on package manager and tsconfig/bundler semantics.

### Treat Repo Directories As Packages

Rejected. Many directories are source sets, generated outputs, examples, vendored deps, fixtures, or build targets rather than packages.
