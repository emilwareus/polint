# Standard Semantic Index Vocabulary

This file defines the comparison vocabulary used across the per-tool reports.

## Implementation Profile

Each tool report uses these fields:

| Field | Meaning |
|---|---|
| Language scope | Single language, JVM bytecode, multi-language, or exchange format. |
| Semantic unit | File, module, package, crate, project, database, workspace, or class loader. |
| Core objects | Scope, symbol, binding, declaration, reference, occurrence, package, module, type, member. |
| Identity model | How semantic objects remain stable across files, packages, runs, and exports. |
| Resolution ladder | Syntax-only, binder, import-aware, type-aware, whole-program, extension-assisted. |
| Incrementality model | Query engine, builder state, file versioning, package cache, serialized xrefs, database rebuild. |
| Search index | Name index, symbol index, xref index, occurrence index, graph store, relational DB. |
| Precision strategy | Exact, conservative, heuristic, dynamic, generated, external, unresolved. |
| Complexity drivers | AST nodes, imports, scopes, references, overloads, type inference, classpath, points-to state, fixpoint iterations. |
| Failure modes | Dynamic imports, reflection, generated code, macros, incomplete classpath, ambiguous aliases, monkey patching. |
| Polint implication | What to copy or avoid. |

## Semantic Fact Families

### `ScopeFact`

A lexical or semantic container that determines lookup.

```text
ScopeFact {
  id: ScopeId,
  owner: Option<SymbolId>,
  parent: Option<ScopeId>,
  file: FileId,
  range: TextRange,
  kind: Module | Package | File | Class | Function | Block | Comprehension | TypeParameter | Generated,
  namespaces: NamespaceSet,
  visibility: Public | Private | Exported | Local | Synthetic,
  provenance: ProvenanceId,
}
```

### `SymbolFact`

A semantic entity that references may resolve to.

```text
SymbolFact {
  id: SymbolId,
  stable_key: StableSymbolKey,
  local_key: LocalSymbolKey,
  language: Language,
  kind: Module | Package | Type | Function | Method | Field | Variable | Parameter | ImportAlias | Label | Generated,
  namespace: Type | Value | Macro | Package | Label | Field | Attribute,
  name: String,
  qualified_name: Option<String>,
  declarations: Vec<DeclarationId>,
  definitions: Vec<DefinitionId>,
  export_status: Local | Exported | Reexported | External | Unknown,
  provenance: ProvenanceId,
}
```

### `ReferenceFact`

An occurrence that may point to a symbol.

```text
ReferenceFact {
  id: ReferenceId,
  file: FileId,
  range: TextRange,
  spelling: String,
  role: Read | Write | Call | Receiver | Import | Export | TypeUse | Definition | Declaration,
  enclosing_scope: ScopeId,
  candidates: Vec<SymbolId>,
  chosen: Option<SymbolId>,
  resolution: ResolutionStatus,
  confidence: Confidence,
  provenance: ProvenanceId,
}
```

### `ImportFact`, `ExportFact`, `AliasFact`

Import/export/alias facts are separate from references because they participate in recursive fixpoints.

```text
ImportFact {
  id: ImportId,
  from_module: ModuleId,
  import_path: String,
  imported_name: Option<String>,
  local_name: Option<String>,
  resolved_module: Option<ModuleId>,
  status: Resolved | Ambiguous | Missing | External | Dynamic | Unsupported,
}

AliasFact {
  alias_symbol: SymbolId,
  target_candidates: Vec<SymbolId>,
  status: Exact | Ambiguous | Unresolved | ExtensionAsserted,
}
```

### `ResolutionFact`

Resolution facts are the audit trail for why a reference resolved or did not resolve.

```text
ResolutionFact {
  reference: ReferenceId,
  provider: ProviderId,
  input_facts: Vec<FactRef>,
  candidates_before_filter: Vec<SymbolId>,
  candidates_after_filter: Vec<SymbolId>,
  selected: Option<SymbolId>,
  status: ResolutionStatus,
  precision: Precision,
  explanation: ResolutionExplanationId,
}
```

## Resolution Status

| Status | Meaning |
|---|---|
| `ExactLocal` | Bound inside a lexical scope without ambiguity. |
| `ExactImported` | Bound through a resolved import/package/module edge. |
| `AliasResolved` | Bound through alias/reexport/facade facts. |
| `TypeAssisted` | Bound using type or receiver facts. |
| `WholeProgram` | Bound using class hierarchy, method set, or package graph facts. |
| `Generated` | Bound to generated/synthetic symbol with provenance. |
| `ExtensionAsserted` | Bound by repo-local extension before validation raises trust. |
| `ValidatedExtension` | Extension fact passed schema, referential, and fixture validation. |
| `Ambiguous` | Multiple plausible targets remain. |
| `Unresolved` | No target found. |
| `Dynamic` | Runtime behavior is known to affect resolution but static target is not available. |
| `Unsupported` | Language feature or framework pattern is recognized but not modeled. |
| `External` | Target is outside selected analysis roots. |

## Precision Labels

| Label | Definition |
|---|---|
| `SyntaxExact` | Derived directly from syntax with exact span. |
| `BinderExact` | Scope/binder rules resolve the target exactly. |
| `PackageExact` | Package/module system resolves the target exactly. |
| `TypeExact` | Type checker resolves the target exactly under selected lifecycle inputs. |
| `Conservative` | Over-approximates possible targets. |
| `Heuristic` | Useful but may be incomplete or wrong. |
| `ExtensionUnvalidated` | Extension-provided fact has not passed fixtures/benchmarks. |
| `ExtensionValidated` | Extension-provided fact passed validation gates. |
| `Unknown` | Engine cannot make a precision claim. |

## Complexity Vocabulary

Use `N` for AST nodes, `D` for declarations, `R` for references, `I` for imports/exports, `S` for scopes, `T` for type facts, `C` for classes/types, and `E` for graph edges.

Typical phases:

- Parse: `O(source bytes)` per file.
- Scope/declaration bind: `O(N + D)`.
- Local reference lookup: usually `O(R * lexical_depth)` without indexes; close to `O(R)` with parent links and scope maps.
- Import/export fixpoint: `O(iterations * (I + aliases + exports))`.
- Name index construction: `O(D + R)` plus sort/FST/hash-map cost.
- Type-aware resolution: language-specific; normal cases are near linear in semantic graph size, but overloads, generics, unions, dynamic features, and classpaths can be superlinear.
- Whole-program JVM call/resolution layers: can range from class-hierarchy linear scans to high-polynomial pointer analysis.
- Export: `O(symbols + occurrences + relationships)`.

## Pseudo-Code Style

Use Python-ish pseudo-code:

```python
def build_semantic_index(project):
    syntax = parse(project.files)
    scopes, declarations = bind_declarations(syntax)
    imports = resolve_imports(scopes, declarations)
    refs = bind_references(scopes, imports)
    refs = type_assist(refs)
    refs = merge_extensions(refs)
    return validate_and_index(scopes, declarations, refs)
```

The Rust implementation should use typed IDs, arenas, stable keys, deterministic iteration, sidecar metadata, and provider outputs validated through the analysis kernel.
