# SCIP

## What It Is

SCIP, the Sourcegraph Code Intelligence Protocol, is a language-agnostic code-indexing format for code navigation. It is the best modern reference for semantic index export: documents, occurrences, symbols, relationships, and external symbols.

Primary inspected files:

- `scip.proto`
- `docs/scip.md`
- `docs/DESIGN.md`

## Index Shape

Core objects:

- **Index:** metadata, documents, external symbols.
- **Document:** relative path, language, occurrences, symbols, text, position encoding.
- **Occurrence:** range, symbol, roles, syntax kind, enclosing range, diagnostics.
- **SymbolInformation:** symbol, documentation, relationships.
- **Relationship:** related symbol plus roles such as definition/reference.
- **Symbol string grammar:** scheme, package, descriptors, or local IDs.

Descriptor kinds include namespace, type, term, method, type parameter, parameter, meta, and macro.

## Algorithm

```python
def export_scip(project_index):
    scip = Index(metadata=build_metadata(project_index))
    for file in project_index.files:
        doc = Document(relative_path=file.path, language=file.language)
        for occ in file.occurrences:
            doc.occurrences.append(Occurrence(
                range=encode_range(occ.range),
                symbol=export_symbol_key(occ.symbol),
                roles=occ.roles,
                syntax_kind=occ.syntax_kind,
            ))
        for symbol in file.local_symbols:
            doc.symbols.append(export_symbol_information(symbol))
        scip.documents.append(doc)
    scip.external_symbols = export_external_symbols(project_index)
    return scip
```

## Accuracy

SCIP does not compute semantic accuracy; it records what an indexer emits. Its design improves interoperability and debuggability.

Important design choices:

- symbol strings instead of opaque integer graph IDs;
- occurrence roles as bitsets;
- no graph adjacency as the core storage shape;
- protobuf compatibility and streamability;
- external symbols separated from per-document occurrences.

## Complexity

Export cost is linear:

```text
O(documents + occurrences + symbols + relationships)
```

Storage size depends on occurrence count and symbol string length. Symbol strings cost more than integer IDs, but the design chooses debuggability and reduced cross-file blast radius.

## Strengths

- Best export model for code navigation.
- Simple mental model: documents and occurrences.
- Symbol grammar is reusable.
- Less fragile than LSIF graph adjacency.
- Good fit for future polint export.

## Weaknesses

- Not an analysis engine.
- Does not define how to resolve names.
- Does not represent all provenance/precision states polint needs internally.

## Polint Implications

Copy:

- symbol key grammar concepts;
- occurrence roles;
- document-local occurrence layout;
- external symbol separation;
- export as transmission format.

Avoid:

- using SCIP as internal fact storage;
- losing provenance/precision in the internal model just because SCIP has a smaller schema.

Recommended polint path:

```text
Internal typed facts
  -> stable export keys
  -> SCIP export adapter
```

Add SCIP export only after stable keys and role semantics are settled.
