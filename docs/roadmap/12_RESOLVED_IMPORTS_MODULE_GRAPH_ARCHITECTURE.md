# Phase 12 Architecture: Resolved Imports And Module Graph

## Goal

Phase 12 turns syntactic import facts into a first real codebase graph layer.
Rules should be able to ask what an import points to, why it did not resolve,
and which file/package/module relationships exist in the repository.

This phase should stay narrow. It is not symbols, call graph, CFG, or dataflow.
It creates the project relationship layer those later capabilities will use.

## Architectural Position

The feature should be implemented as a project-wide derived fact provider:

1. `runner` builds the `AnalysisPlan` from enabled rules.
2. `fs` loads files into `AnalysisDb`.
3. Go and TS/JS adapters populate syntactic facts such as `ImportFact`.
4. The new module-graph provider consumes `AnalysisDb`, `LoadedConfig`, and the
   plan, then appends resolved import and graph facts.
5. Rules run through typed SDK views.

Do not bury this in the per-file Go or TS adapters. Import resolution is
project-wide: it needs all discovered files, config, package metadata, and
resolver setup. The language adapters should continue to own syntax extraction;
the module-graph provider should own relationship construction.

Recommended internal module:

```text
crates/polint/src/module_graph/
  mod.rs        # orchestration, public-to-crate entrypoint
  model.rs      # internal builder/index structs if they are not SDK facts
  query.rs      # reachability and deterministic graph queries
  ts.rs         # TS/JS resolver integration
  go.rs         # Go package metadata integration
  paths.rs      # repo-relative path normalization helpers
```

Keep the public fact structs in `core` and the rule-facing views in
`sdk::facts`, matching existing fact families.

## Public SDK Surface

Rule authors should consume only `polint::sdk::prelude::*`.

Add typed views:

```rust
ResolvedImports<'_>
ModuleGraphFacts<'_>
```

Add fact/model types:

```rust
ResolvedImportId
ModuleNodeId
ModuleEdgeId
ResolvedImportFact
ModuleNode
ModuleEdge
ModuleNodeKind
ModuleEdgeKind
ResolutionStatus
ResolutionPrecision
UnresolvedReason
```

The capability names should be distinct from syntactic `imports`:

```rust
Capabilities::resolved_imports()
Capabilities::module_graph()
```

`ResolvedImports<'_>` should provide import-centric queries:

```rust
all() -> &[ResolvedImportFact]
iter() -> Iter<'_, ResolvedImportFact>
for_file(file: FileId) -> impl Iterator<Item = &ResolvedImportFact>
for_import(import: ImportId) -> Option<&ResolvedImportFact>
```

`ModuleGraphFacts<'_>` should provide graph-centric queries:

```rust
nodes() -> &[ModuleNode]
edges() -> &[ModuleEdge]
outgoing(node: ModuleNodeId) -> impl Iterator<Item = &ModuleEdge>
incoming(node: ModuleNodeId) -> impl Iterator<Item = &ModuleEdge>
node_for_file(file: FileId) -> Option<ModuleNodeId>
reachable_from(node: ModuleNodeId) -> Vec<ModuleNodeId>
```

Return borrowed facts and iterators where practical. Returning `Vec` is
acceptable for transitive graph algorithms such as reachability because the
result is computed and owned.

Use `#[non_exhaustive]` on public enums that will grow, especially status,
precision, node kind, edge kind, and unresolved reason. That keeps the API
extensible without forcing a breaking change every time a language adds a new
resolution case.

## Core Storage Model

Store relationships by stable IDs, not cloned source structs.

Suggested shape:

```rust
pub struct ResolvedImportFact {
    pub id: ResolvedImportId,
    pub import: ImportId,
    pub from_file: FileId,
    pub target_node: Option<ModuleNodeId>,
    pub status: ResolutionStatus,
    pub precision: ResolutionPrecision,
    pub reason: Option<UnresolvedReason>,
}

pub struct ModuleNode {
    pub id: ModuleNodeId,
    pub kind: ModuleNodeKind,
    pub label: String,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub language: Option<Language>,
}

pub struct ModuleEdge {
    pub id: ModuleEdgeId,
    pub from: ModuleNodeId,
    pub to: ModuleNodeId,
    pub import: Option<ImportId>,
    pub resolved_import: Option<ResolvedImportId>,
    pub kind: ModuleEdgeKind,
}
```

`AnalysisDb` should own:

```rust
resolved_imports: Vec<ResolvedImportFact>
module_nodes: Vec<ModuleNode>
module_edges: Vec<ModuleEdge>
```

The builder may keep internal indexes while constructing facts:

```rust
BTreeMap<FileId, ModuleNodeId>
BTreeMap<String, ModuleNodeId>
BTreeMap<ModuleNodeId, Vec<ModuleEdgeId>>
```

Use `BTreeMap`/sorted insertion for deterministic output. Avoid `HashMap` unless
the result is explicitly sorted before insertion into `AnalysisDb`.

## Resolution Status Contract

Every syntactic import should produce a resolved import fact. Hard cases should
not disappear.

Recommended statuses:

| Status | Meaning |
|---|---|
| `Resolved` | Target is a repo file/package/module node. |
| `External` | Target is outside the repo, such as stdlib or dependency package. |
| `Unresolved` | Setup existed, but the resolver could not find a target. |
| `SetupMissing` | Required resolver setup was absent or invalid. |
| `Dynamic` | Import shape is dynamic and cannot be resolved statically. |
| `Unsupported` | Language/import form is known but not implemented yet. |

Recommended precision tiers:

| Precision | Meaning |
|---|---|
| `ExactFile` | The import resolves to one concrete repo file. |
| `Package` | The import resolves to a package/module, possibly with many files. |
| `ExternalPackage` | The import resolves to an external package or stdlib module. |
| `Heuristic` | The result came from conservative fallback matching. |
| `None` | No target was produced. |

This status model is what makes the graph useful to agents. An agent can tell
the difference between "the repo does not import this" and "polint could not
prove the target because setup is missing."

## Resolution Algorithms

### Shared Build Algorithm

1. Build a repo file index from `AnalysisDb.files()`:
   - repo-relative path to `FileId`
   - normalized path without following symlinks
   - package facts by file and package name
2. Seed one `File` node per analyzed source file.
3. Seed package/module nodes from existing `PackageFact` where available.
4. Iterate `ImportFact` in deterministic order: source file path, span, import
   path, import ID.
5. Dispatch to the language resolver for the import language.
6. Convert the resolver result into one `ResolvedImportFact` and zero or one
   import edge.
7. Sort/dedupe graph nodes and edges before appending them to `AnalysisDb`.

Use lexical path normalization for repo-relative paths. Do not use
`std::fs::canonicalize` for graph identities; it follows symlinks, requires the
target to exist, and can produce machine-specific absolute paths.

### TS/JS

Use `oxc_resolver` behind `module_graph::ts`.

Inputs:

- importer absolute path
- import specifier from `ImportFact.path`
- repo root
- `tsconfig` `baseUrl` and `paths`
- common JS/TS extensions
- package `exports`/`imports`
- condition names and main fields

Start with these cases:

- relative imports to repo files
- tsconfig path aliases to repo files
- package imports to external nodes
- unsupported/dynamic forms marked explicitly

The resolver should be constructed once per run/config, not once per import.
The implementation should return a small owned result type, not mutate
`AnalysisDb` directly.

### Go

Use Go package metadata behind `module_graph::go`.

Preferred first implementation:

- run `go list -json ./...` when a Go module is present
- parse package import paths and source file lists
- map package files back to `AnalysisDb` file IDs
- map local package imports to package/file nodes
- map stdlib and dependency imports to external nodes

If Go setup is missing or `go list` fails, emit `SetupMissing` facts for imports
that need package resolution, plus a diagnostic when `resolved_imports` or
`module_graph` was requested.

Do not add a Go sidecar or full type analysis in this phase. That belongs to
symbols/references and call graph work.

## Error Handling

Use typed internal errors for resolver setup and execution:

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum ModuleGraphError {
    #[error("failed to read resolver setup: {0}")]
    Setup(String),
    #[error("failed to resolve import `{import}` in `{file}`: {reason}")]
    Resolve {
        file: String,
        import: String,
        reason: String,
    },
}
```

The provider should convert expected failures into facts and diagnostics, not
panic. `anyhow` remains fine at CLI/runner boundaries, but module-graph internals
should prefer `thiserror` so tests can assert exact failure modes.

No `unwrap()` or `expect()` in production resolver code. Use `let Some(...) =
... else` for expected missing state and `?` for real fallible operations.

## Cache And Invalidations

Do not store resolved imports in per-file parser caches. A resolved import fact
depends on project state, not only one source file.

The first implementation can recompute the module graph every run after cached
syntax facts are restored. That is correct and simpler. Measure before adding a
project-level graph cache.

If a project-level cache is added, use a separate schema such as
`module-graph-v1` and a project digest including:

- `AnalysisPlan.digest()`
- relevant resolver config
- sorted `(relative_path, content_hash)` for files with imports
- sorted import facts
- resolver setup file hashes such as `tsconfig.json`, `package.json`, `go.mod`,
  and `go.sum` when they exist
- resolver tool version or output schema for Go metadata

The cache API may need a `CacheKey::for_project(...)` constructor rather than
overloading `for_file`.

## Rust Maintainability Principles

- Keep new modules `pub(crate)` by default. Only `sdk`, `runner`, and documented
  prelude exports are public rule-author surfaces.
- Use small `Copy` ID newtypes for graph IDs, matching `FileId`, `ImportId`, and
  `FunctionId`.
- Store relationships by ID and borrow from `AnalysisDb` in SDK views. Do not
  clone `SourceFile`, source strings, or large graph collections in query paths.
- Prefer simple functions and enums over trait objects for the first two
  language resolvers. Add a trait only when Python/Java make the duplication
  real.
- If a trait becomes useful later, keep it crate-private:

```rust
pub(crate) trait ImportResolver {
    fn resolve_imports(&self, input: ResolverInput<'_>) -> Vec<ResolvedImportDraft>;
}
```

- Keep raw language-tool output out of public facts. Oxc resolver results and
  `go list` JSON are adapter details.
- Use deterministic ordering everywhere the output can affect diagnostics,
  cache keys, or tests.
- Keep graph algorithms small and named. Start with direct edge iteration and
  BFS/DFS reachability; add SCC/cycle detection only after the node/edge model
  is stable.

## Testing Strategy

Add tests at four levels:

1. Core/model tests:
   - stable node and edge insertion
   - dedupe behavior
   - path normalization
   - reachability from a node
   - unresolved/setup-missing facts stay visible
2. TS/JS resolver tests:
   - relative import resolves to repo file
   - `tsconfig` alias resolves to repo file
   - package import becomes external
   - dynamic/unsupported import is explicit
3. Go resolver tests:
   - local package import maps to package/file node
   - stdlib import becomes external
   - missing `go.mod` or failed `go list` creates setup-missing facts
4. External-consumer test:
   - generated `.polint/rules` imports only `polint::sdk::prelude::*`
   - a rule requests `ResolvedImports<'_>` or `ModuleGraphFacts<'_>`
   - the rule enforces a simple architecture boundary through `polint check
     --format json`

Also verify determinism with repeated runs and parallel parsing enabled.

## Suggested Implementation Slices

1. Model and SDK:
   add capabilities, fact types, `AnalysisDb` storage, SDK views, macro mapping,
   docs, and unsupported diagnostics until the provider exists.
2. Provider skeleton:
   add `module_graph` orchestration, deterministic node/edge builder, and
   syntax-only unresolved facts.
3. TS/JS resolution:
   integrate `oxc_resolver`, support relative paths and aliases, add fixtures.
4. Go resolution:
   integrate package metadata from `go list -json ./...`, map local packages,
   add fixtures.
5. External proof:
   add a temp-repo rule that consumes the new views through the public prelude
   and asserts JSON diagnostics.

This order keeps the public contract clear before adding resolver complexity,
and each slice can be reviewed without mixing public API, graph algorithms, and
language-specific setup in one change.

## Non-Goals

- No symbol/reference identity.
- No call graph.
- No CFG or dataflow.
- No exact Node package-manager behavior beyond what `oxc_resolver` supports.
- No Go type checking.
- No public CLI graph export unless a user-facing workflow is deliberately
  promoted later.
