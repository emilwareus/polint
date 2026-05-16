# gopls / golang.org/x/tools

## What It Is

gopls is the Go language server. Its semantic index is package-centered: metadata, parsed files, type-checked packages, `types.Info`, import maps, method sets, and serialized cross-reference indexes.

Primary inspected files:

- `gopls/internal/cache/package.go`
- `gopls/internal/cache/metadata/metadata.go`
- `gopls/internal/golang/definition.go`
- `gopls/internal/golang/references.go`
- `gopls/internal/cache/xrefs/xrefs.go`

## Index Shape

Core objects:

- **Metadata:** package ID/path/name, files, deps, module, variants.
- **Package:** metadata plus parsed files, type package, `types.Info`, imports, diagnostics, lazy indexes.
- **types.Info:** Go's authoritative maps: `Defs`, `Uses`, `Types`, `Selections`, `Scopes`, `Implicits`.
- **xrefs index:** serialized outbound cross-package references computed during type checking.
- **objectpath:** stable path to a Go object inside a package.

## Algorithm

```python
def load_go_package(metadata):
    files = parse_go_files(metadata.files)
    pkg, info = typecheck(files, metadata.deps, metadata.options)
    imports = map_imports(files, pkg.imports)
    xrefs = build_xref_index(files, pkg, info)
    return Package(metadata, files, pkg, info, imports, xrefs)

def definition_at_position(pkg, position):
    node = path_enclosing(position)
    if node in pkg.types_info.Uses:
        return pkg.types_info.Uses[node]
    if node in pkg.types_info.Defs:
        return pkg.types_info.Defs[node]
    handle_imports_labels_embeds_doclinks()

def find_references(target):
    target_key = (target.package_path, objectpath(target))
    for pkg in candidate_packages(target):
        yield from lookup_xref_index(pkg.xrefs, target_key)
```

## Accuracy

gopls is strong because `go/types` owns Go semantics:

- lexical scopes;
- packages;
- imports;
- object identity;
- methods and receivers;
- selections;
- labels;
- embedded fields;
- test variants.

Hard cases:

- build tags;
- multiple modules/workspaces;
- generated files;
- incomplete module setup;
- test package variants;
- vendoring and replacement modules.

## Complexity

Parsing is linear in source. Package type checking is usually near linear in package size plus dependencies already loaded, but depends on imports, generics, and build configuration.

Xref index construction walks `types.Info.Uses`:

```text
O(uses in package)
```

Lookup uses target package/object paths and serialized indexes, avoiding full re-typechecking for every global reference query.

## Strengths

- Package-first identity model.
- `objectpath` provides stable object addressing.
- Serializable xrefs are exactly the kind of global reference acceleration polint needs.
- Metadata cleanly separates module/package lifecycle from file syntax.

## Weaknesses

- Go-specific `types.Info` is not portable.
- Accurate behavior depends on correct module/workspace/build-tag lifecycle.
- Internal APIs are not designed as a public reusable Rust engine.

## Polint Implications

Copy:

- package metadata layer;
- object-path-like stable keys;
- xref indexes built during semantic analysis;
- explicit lifecycle inputs in cache keys;
- no placeholder semantic facts when package loading fails.

Avoid:

- hiding module/workspace setup gaps;
- making references require full-workspace rescans.

Recommended Go stable key shape:

```text
go:
  module_root
  package_path
  object_path
  namespace
  declaration_span_for_evidence
```

For the native Rust path, polint can mirror gopls fact shapes even before it reaches full native Go type checking.
