# Go structural type facts

- View: `polint::sdk::facts::GoTypeDecls<'_>` (capability: `syntax`)
- Fact row: `polint::sdk::prelude::GoTypeDeclFact` with `GoTypeDeclKind`
- Producer: `polint.go.syntax` (the Go tree-sitter frontend), extracted during the same
  per-file walk that produces packages, functions, imports, and test facts.

`GoTypeDecls` exposes typed Go structural facts so rules can stop re-parsing Go source with
regexes and brace matching. The Go adapter already walks every declaration; these facts
retain the structural detail that walk used to discard.

## Row shapes

Two row shapes share `GoTypeDeclFact`:

1. **Named declarations** — every `type_spec` / `type_alias` node, including grouped
   `type ( ... )` members and function-local declarations:
   - `name` is `Some(identifier)`;
   - `span` is the declared **name token** span (1-based line/character columns);
   - `declaration_start_byte` points at the `type` keyword of the enclosing
     `type_declaration`;
   - `kind` is `Struct` (struct body), `Interface` (interface body), or `Named` (any
     other underlying expression, including aliases to named types);
   - `is_alias`, `is_grouped`, `is_top_level`, and `has_type_parameters` describe the
     declaration form;
   - `direct_name` is the final identifier of a named underlying expression
     (`type X Y.Z` -> `Z`, `type X Y[T]` -> `Y`); `None` for struct/interface bodies and
     for expressions headed by pointer/map/slice/func/chan forms.
2. **Anonymous struct occurrences** — every `struct_type` node that is not the body of a
   top-level, non-grouped, non-alias declaration (those are fully described by their
   named row):
   - `name` is `None`;
   - `span` is the `struct { ... }` node span;
   - `declaration_start_byte` equals `span.start_byte` (the `struct` keyword);
   - `kind` is `Struct`; all form flags are `false`.

Grouped specs, function-local specs, and alias specs therefore appear twice: once as a
named row (declaration form) and once as an anonymous struct occurrence (body location),
because line-anchored consumer patterns historically only matched the
`^type NAME struct {` shape.

## Byte ranges

`body_range` is the byte range **inside** the braces of a struct or interface body
(`None` for other kinds). Consumers can slice the file source directly:

```rust
let (start, end) = fact.body_range.expect("struct body");
let body = &source[start as usize..end as usize];
```

All offsets are UTF-8 byte offsets into the file source; `Span` carries the matching
1-based line/column positions.

## Guarantees

- Rows are emitted in a single deterministic source-order walk per file and are
  serialized in the per-file syntax cache (`go_types` family).
- Facts are syntax-level: no type checking, no cross-file resolution.
- `for_file` iteration uses a dense per-file index and preserves database order.
