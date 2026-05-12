# Symbols And References

`Symbols<'_>` and `References<'_>` define the public rule-authoring contract for
symbol identities, definitions, and references. Rules request these views as
typed parameters in a `#[polint::rule]` function, and polint derives the
`symbols` and `references` capabilities from those parameters.

The capability names are recognized before language providers are promoted. In
this state, requesting rules receive `polint/capability` diagnostics instead of
running with placeholder facts.

Start rules with:

```rust
use polint::sdk::prelude::*;
```

## Symbols<'_>

`Symbols<'_>` exposes symbol identities and their definitions. There is no
separate `Definitions<'_>` view; definitions are queried through `Symbols`.

Query methods:

| Method | Meaning |
|--------|---------|
| `all()` | Returns all symbol facts in deterministic database order. |
| `iter()` | Iterates all symbol facts. |
| `get(symbol)` | Returns one `SymbolFact` by stable `SymbolId`. |
| `for_file(file)` | Iterates symbols owned by one source file. |
| `by_name(name)` | Iterates symbols with the exact public name. |
| `definition(symbol)` | Returns the primary definition for a symbol, if known. |
| `definitions(symbol)` | Iterates all definitions for a symbol. |
| `exported()` | Iterates symbols marked exported by the provider. |

`SymbolFact` fields:

| Field | Meaning |
|-------|---------|
| `id` | Stable polint-owned symbol ID supplied by the provider. |
| `language` | Language family that produced the symbol. |
| `name` | Short symbol name. |
| `qualified_name` | Provider-normalized qualified name. |
| `kind` | Normalized `SymbolKind`. |
| `namespace` | Value, type, namespace, package, module, or unknown namespace. |
| `file`, `package`, `module`, `owner` | Optional ownership links. |
| `primary_span` | Primary source span when the symbol has one. |
| `is_exported` | Whether the symbol is part of the exported surface. |
| `stable_key` | Debug-safe normalized key material for ID diagnostics. |
| `precision` | `SymbolPrecision` for the symbol identity. |

## References<'_>

`References<'_>` exposes uses of symbol-like names. Definitions are not encoded
as references.

Query methods:

| Method | Meaning |
|--------|---------|
| `all()` | Returns all reference facts in deterministic database order. |
| `iter()` | Iterates all reference facts. |
| `to(symbol)` | Iterates resolved references to one `SymbolId`. |
| `for_file(file)` | Iterates references in one source file. |
| `unresolved()` | Iterates references with `Unresolved` status. |
| `ambiguous()` | Iterates references with `Ambiguous` status. |

`ReferenceFact` fields:

| Field | Meaning |
|-------|---------|
| `id` | Stable polint-owned reference ID supplied by the provider. |
| `language` | Language family that produced the reference. |
| `name` | Short referenced name. |
| `qualified_name` | Provider-normalized qualified name when available. |
| `kind` | Normalized `ReferenceKind`. |
| `namespace` | Namespace the reference was interpreted in. |
| `file`, `package`, `module`, `owner` | Optional ownership links. |
| `primary_span` | Source span for the reference when available. |
| `target` | Resolved target symbol, when exact enough. |
| `candidates` | Candidate symbols for ambiguous references. |
| `stable_key` | Debug-safe normalized key material for ID diagnostics. |
| `status` | `SymbolResolutionStatus` for the reference. |
| `precision` | `SymbolPrecision` for the binding. |

## Status And Precision

`SymbolPrecision` includes `ExactSemantic`, `ExactLocal`, `ModuleLinked`,
`Heuristic`, `Unresolved`, `Ambiguous`, `SetupMissing`, and `Unsupported`.

`SymbolResolutionStatus` includes `Resolved`, `Unresolved`, `Ambiguous`,
`SetupMissing`, and `Unsupported`.

Uncertain states are data, not hidden failures. Providers should emit explicit
status and precision instead of dropping facts silently.

## Limits

This page describes the public contract and capability names. Provider support
is staged across Phase 13:

- no TS/JS Oxc semantic population until the TS/JS symbol provider is promoted
- no Go typed package population until the Go symbol provider is promoted
- no call graph, CFG, dataflow, coverage, or whole-program analysis
- no public exposure of Oxc IDs, Go object values, sidecar JSON, raw AST nodes,
  or internal indexes

Until providers are promoted, rules requesting `Symbols<'_>` or `References<'_>`
receive capability diagnostics and do not execute with empty placeholder facts.
