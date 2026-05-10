# Entry 4: Resolved Imports And Module Graph

## Goal

Turn syntactic import strings into resolved file, package, and module
relationships.

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
