# Kythe

## What It Is

Kythe is a cross-language semantic indexing and graph schema system originally built from Google's large-scale code-indexing experience. It is the strongest reference for durable semantic graph identity and storage.

Primary inspected files:

- `kythe/proto/storage.proto`
- `kythe/proto/schema.proto`
- `kythe/go/services/graphstore/graphstore.go`
- `kythe/go/util/schema/schema.go`

## Index Shape

Core objects:

- **VName:** signature, corpus, root, path, language.
- **Entry:** source VName, optional edge kind/target VName, fact name, fact value.
- **Node facts:** facts attached to source VName without edge target.
- **Edge facts:** facts attached to source-target edge.
- **Schema:** node kinds and edge kinds for anchors, files, functions, records, packages, references, definitions, calls, overrides, types, inheritance, child relationships.
- **Graphstore:** read/scan/write API over entries.

Kythe intentionally excludes revision/time from VName identity. Revision is a graph versioning concern, not part of semantic object identity.

## Algorithm

```python
def emit_kythe_entries(index):
    for file in index.files:
        file_vname = vname_for_file(file)
        emit_fact(file_vname, "node/kind", "file")

        for symbol in file.symbols:
            sym_vname = vname_for_symbol(symbol)
            emit_fact(sym_vname, "node/kind", symbol.kind)

        for ref in file.references:
            anchor = vname_for_anchor(file, ref.range)
            emit_fact(anchor, "node/kind", "anchor")
            emit_edge(anchor, "ref", vname_for_symbol(ref.target))
            emit_edge(anchor, "childof", vname_for_enclosing_symbol(ref))
```

## Accuracy

Kythe does not make facts accurate by itself; indexers do. Its schema supports rich semantic expression:

- definitions and references;
- call edges;
- child-of relationships;
- types;
- inheritance/overrides;
- generated semantic nodes;
- anchors for source spans.

This makes it a strong reference for polint export/evidence and stable identity.

## Complexity

Emitting entries is linear in facts:

```text
O(nodes + edges + facts)
```

Graphstore read is efficient by source. Scan by target/kind/fact prefix has less predictable cost and may require sharding. This is a warning for polint: design query indexes around actual rule queries, not just generic graph scans.

## Strengths

- Durable cross-language identity model.
- Rich semantic graph schema.
- Separates anchors from semantic nodes.
- Entry model is flexible and extensible.
- Good generated-symbol support.

## Weaknesses

- More general and storage-oriented than polint's first internal needs.
- Generic graph scans are not enough for fast rule-time queries.
- Requires high-quality language indexers.

## Polint Implications

Copy:

- VName-like stable export identity;
- anchor vs semantic node separation;
- generated semantic node concepts;
- fact/edge provenance thinking;
- schema vocabulary for calls, refs, definitions, overrides, types.

Avoid:

- using graphstore as rule-time storage;
- query plans that require arbitrary graph scans;
- losing typed SDK ergonomics behind generic node/edge APIs.

Recommended identity mapping:

```text
StableSymbolKey
  -> optional KytheVName {
       corpus,
       root,
       path,
       language,
       signature
     }
```

Kythe should influence stable identity and evidence export, not the initial in-memory engine.
