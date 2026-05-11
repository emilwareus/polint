# Entry 4: Resolved Imports And Module Graph

## Goal

Turn syntactic import strings into resolved file, package, and module
relationships.

Detailed implementation architecture:
[`12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md`](12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md).

## Why

Most practical architecture rules need to know what an import points to, not
just the text of the import.

## Difficulty

**M for TS/JS**, **M/L for Go**, **L for Python later**, **XL for Java later**.

## What To Build

- `ResolvedImportFact`
- `ModuleGraph`
- `ResolutionStatus`
- `UnresolvedReason`
- `ResolvedImports<'_>`
- `ModuleGraphFacts<'_>`

## Build Method

1. Keep current `ImportFact` as the syntactic source of truth.
2. Add `ResolvedImportFact` with `from_file`, `import`, `target_file`,
   `target_package`, `resolution_status`, and `unresolved_reason`.
3. Add `ModuleGraph` nodes for files, packages, and modules, exposed through
   typed SDK views.
4. For TS/JS, use `oxc_resolver::ResolveOptions` with `tsconfig`, extensions,
   condition names, main fields, and package exports/imports settings.
5. For Go, use `go/packages.Load` and map package IDs and `GoFiles` to polint
   files.
6. Preserve unresolved imports with explicit reasons.

## Implementation Notes

- Represent the module graph as deterministic typed nodes and edges rather than
  a CLI-only `petgraph` rendering. Good first node kinds are `File`, `Package`,
  `Module`, and `External`.
- Normalize all paths through the existing repo-relative path context before
  creating node IDs, so cache restore and diagnostics stay stable.
- TS/JS resolution should start from `ImportFact.path` and use
  `oxc_resolver` with project roots, `tsconfig` paths/baseUrl, common
  extensions, package exports/imports, and condition names. Mark dynamic or
  unsupported forms explicitly instead of dropping them.
- Go resolution should start with `go list` / `go/packages` package metadata,
  then map package files back to `AnalysisDb` files. External stdlib/module
  imports should still become graph nodes even when they do not resolve to repo
  files.
- The first graph algorithm should be reachability over resolved module edges:
  `ModuleGraphFacts::reachable_from(file_or_package)` and direct edge iteration.
  Strongly connected components can follow for cycle/layer rules once the base
  graph is stable.
- Keep precision visible: `Resolved`, `External`, `Unresolved`,
  `SetupMissing`, and `Dynamic` are more useful to rules and agents than an
  empty graph.

## Done When

- TS/JS and Go imports resolve to repo files/packages where possible.
- Unresolved imports remain visible with reasons.
- Rules can write layer-boundary policies without string-only matching.
- Docs explain setup requirements.

## Later Languages

- Python should combine AST imports, repo roots, `pyproject.toml`, interpreter
  path, virtualenv metadata, and `importlib` behavior.
- Java should consume Maven/Gradle classpaths and resolve packages/classes
  through javac or JavaParser symbol solver.
