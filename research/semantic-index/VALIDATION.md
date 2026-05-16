# Validation Plan

## Goal

Validate semantic-index facts before they become public precision claims or feed call graph/data-flow providers.

The validation target is fact-level correctness first, diagnostic correctness second.

## Metrics

| Metric | Meaning |
|---|---|
| Symbol precision | Predicted symbols that match expected symbols. |
| Symbol recall | Expected symbols recovered by the engine. |
| Reference precision | Resolved references that point to the expected target. |
| Reference recall | Expected resolvable references found and bound. |
| Unknown rate | References/imports that remain unresolved, ambiguous, dynamic, or unsupported. |
| Import resolution accuracy | Import path/local alias/reexport facts matched against expected module targets. |
| Extension delta | Extended result score minus default result score. |
| Stability | Stable keys unchanged after unrelated edits. |
| Cache correctness | Fact layer cache invalidates only when relevant inputs change. |
| Determinism | Fact ordering and stable keys unchanged across parallel runs. |

Use strict and loose modes:

- **Strict:** stable key, file, span, role, and target match.
- **Loose:** same semantic target and compatible span/name after formatting or parser recovery.

## Fixture Taxonomy

### Cross-Language Core

- shadowing;
- nested scopes;
- block/function/class/module scopes;
- imports and aliases;
- exports/reexports;
- generated/synthetic symbols;
- unresolved references;
- ambiguous references;
- external package references;
- comments/doc references only if explicitly supported;
- incremental edit stability.

### Go

- package declarations and imports;
- aliases, dot imports, blank imports;
- methods and receivers;
- test packages and `_test` variants;
- multi-module repositories;
- build tags;
- generated files.

### TS/JS

- ES imports/exports/reexports;
- default exports;
- namespaces and declaration merging;
- CommonJS `require` and `module.exports`;
- JSX components;
- type-only imports;
- dynamic import marked dynamic;
- `.d.ts` external symbols.

### Python

- local/global/nonlocal;
- class/function/comprehension scopes;
- star imports;
- alias imports;
- flow-sensitive initialization;
- package `__init__` exports;
- dynamic attributes marked unknown/dynamic.

### Java/JVM

- packages/imports/static imports;
- overloaded methods;
- fields vs locals;
- class hierarchy method lookup;
- generics/type parameters;
- module/classpath missing types;
- source plus bytecode symbols.

## External Oracles

Use mature tools as validation oracles, not runtime dependencies:

- gopls or `go/types` for Go expected facts;
- TypeScript compiler language service for TS/JS expected facts;
- Pyright/Ty/Pyrefly for Python expected facts;
- JDT for Java source expected facts;
- SCIP/Kythe output shape for export compatibility.

Oracle mismatch does not automatically mean polint is wrong. Store mismatches as reviewed cases with provenance, because tools disagree on incomplete/dynamic features.

## Extension Validation

Agent-authored providers must pass:

1. schema validation;
2. referential validation: referenced file/scope/symbol exists or is deliberately generated;
3. conflict validation: native exact facts cannot be silently replaced;
4. fixture validation: repo-local expected facts pass;
5. optional benchmark validation: extension improves metrics without hiding unknowns;
6. cache validation: extension digest participates in dependent layer keys.

## Golden Output Schema

```json
{
  "symbols": [
    {
      "key": "go:pkg:example.com/app:func:Handler",
      "kind": "function",
      "file": "app/handler.go",
      "span": "10:1-14:2",
      "export_status": "exported"
    }
  ],
  "references": [
    {
      "file": "app/router.go",
      "span": "7:22-7:29",
      "role": "reference",
      "target": "go:pkg:example.com/app:func:Handler",
      "resolution": "ExactImported"
    }
  ],
  "imports": [
    {
      "file": "app/router.go",
      "path": "example.com/app",
      "local_name": "app",
      "status": "Resolved"
    }
  ]
}
```

## Acceptance Before Public SDK Expansion

Before promoting `Scopes<'_>` or `Imports<'_>`:

- all core fixtures pass for Go and TS/JS;
- unknown/ambiguous/external status is visible in JSON debug output;
- stable keys survive whitespace-only and unrelated edits;
- extension-added generated symbol fixture passes;
- cache tests cover config, lifecycle, provider, schema, and extension digest changes;
- docs state exact limits per language.
