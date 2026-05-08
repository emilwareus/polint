# Entry 6: Symbols And References

## Goal

Expose stable definitions, symbols, and references through the public SDK.

## Why

Symbols and references move rules beyond string matching. They are the
foundation for precise call graphs, ownership checks, exported API policies,
dead-code style rules, and security rules.

## Difficulty

**M/L for TS/JS**, **L for Go**, **L/XL for Java**, **XL/XXL for Python
precision**.

## What To Build

- `SymbolFact`
- `ReferenceFact`
- `DefinitionFact`
- `SymbolId`
- `ReferenceKind`
- `SymbolKind`
- `SymbolPrecision`
- `RuleCtx::symbols`
- `RuleCtx::references`
- `RuleCtx::references_to(symbol_id)`
- `RuleCtx::definition(symbol_id)`

## Build Method

1. Define stable symbol keys from language, package/module path, file, lexical
   owner, name, and span.
2. Add shared symbol/reference fact types.
3. Store definitions and references in `AnalysisDb`.
4. For TS/JS, adapt Oxc semantic symbol tables and reference tracking.
5. For Go, use `go/packages.Load` with typed syntax and `TypesInfo`.
6. Store unresolved and ambiguous references explicitly.
7. Make call graph resolution consume symbol/reference facts.

## Done When

- Go and TS/JS expose symbols and references.
- Rules can find all references to a symbol.
- Symbol IDs are stable enough for diagnostics and cache restore.
- Docs explain precision tiers.

## Later Languages

- Python should start with `ast` plus `symtable`, then use import resolution and
  optional type-checker metadata later.
- Java should use javac `JavacTask`/`Trees` or JavaParser symbol solver.
