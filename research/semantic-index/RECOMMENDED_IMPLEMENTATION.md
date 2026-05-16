# Recommended Implementation: Semantic Index

## Goal

Create a native Rust semantic-index substrate that supports high-precision repo-local rules and agent-authored analysis extensions without depending on external language servers or closed tooling.

The implementation should preserve the current product contract:

- public rule authors use typed SDK views;
- internal implementation details stay `pub(crate)`;
- every heuristic remains honest;
- every extension-provided fact carries provenance and validation status.

## Target Architecture

```text
crates/polint/src/semantic/
  mod.rs
  ids.rs
  facts.rs
  scope.rs
  symbol.rs
  reference.rs
  import.rs
  resolution.rs
  alias.rs
  xref.rs
  provider.rs
  validation.rs
  export/
    scip.rs
    kythe.rs
```

Keep this module internal. Public SDK views should live under existing SDK fact modules once promoted.

## Fact Families

Start with these internal families:

| Family | Public View Timing | Notes |
|---|---|---|
| `ScopeFact` | later `Scopes<'_>` | Needed before exact reference claims. |
| `SymbolFact` | deepen existing `Symbols<'_>` | Add stable keys, namespace, export status, declaration/definition roles. |
| `ReferenceFact` | deepen existing `References<'_>` | Add role, resolution status, candidate list, confidence. |
| `ImportFact` | later `Imports<'_>` | Required for module graph and alias/reexport closure. |
| `AliasFact` | internal first | Required for TS/Python exports, Go package aliases, generated facades. |
| `ResolutionFact` | internal first | Audit trail for why a reference resolved or stayed unknown. |

## Stable Identity

Use three layers:

```rust
pub(crate) struct SymbolId(u32);        // arena-local, fast
pub(crate) struct StableSymbolKey(...); // deterministic inside repo/cache
pub(crate) struct ExportSymbolKey(...); // SCIP/Kythe-like external identity
```

Stable keys should include:

- language;
- repository root digest or corpus root;
- package/module/crate/classloader context;
- file path when local identity is file-scoped;
- declaration path or object path;
- namespace/kind;
- generated-symbol discriminator where needed.

Do not use line numbers alone as stable symbol identity. Use spans as evidence, not primary identity.

## Provider Stack

### Phase 1: Internal Metadata Without Behavior Change

Wrap existing symbol/reference derivation with fact metadata:

```text
provider id
provider version
input layer digests
stable key
precision
confidence
validation
```

Acceptance:

- existing tests pass;
- existing public SDK shape still works;
- debug output can explain symbol/reference provenance.

### Phase 2: Add Scopes And Declarations

For Go and TS/JS:

```python
def build_scopes(ast):
    root = new_scope(kind="file_or_package")
    for node in preorder(ast):
        if opens_scope(node):
            push_scope(node)
        if declares_symbol(node):
            declare_symbol(current_scope, node)
        if closes_scope(node):
            pop_scope()
```

Acceptance:

- every emitted symbol points to an owning scope;
- every local reference points to an enclosing scope;
- shadowing fixtures pass;
- unresolved references are represented explicitly.

### Phase 3: Add Import Facts And Import Resolution

Create language-owned import providers:

- Go: import path and optional alias/dot/blank import.
- TS/JS: static imports, exports, reexports, CommonJS first as conservative facts.
- Python and Java later after module graph research.

```python
def resolve_imports(files, module_graph):
    for imp in import_facts:
        candidates = module_graph.lookup(imp.import_path, imp.from_module)
        emit_import_resolution(imp, candidates)
```

Acceptance:

- import resolution status is `Resolved`, `External`, `Missing`, `Dynamic`, or `Unsupported`;
- lifecycle config participates in cache keys;
- no placeholder exact facts are emitted for missing module roots.

### Phase 4: Add Alias/Reexport Fixpoint

Use a small typed relation/fixpoint helper:

```python
def alias_fixpoint(imports, exports, declarations):
    changed = True
    while changed:
        changed = False
        for edge in imports + exports + aliases:
            if can_resolve(edge):
                changed |= add_alias_target(edge)
    return alias_facts
```

Acceptance:

- cycles terminate with diagnostics and bounded iteration;
- star exports/imports can be represented as conservative closure facts;
- generated/provider-added aliases go through the same merge rules.

### Phase 5: Reference Resolution Ladder

Each language provider should follow a visible ladder:

```python
def resolve_reference(ref):
    for step in [
        lexical_lookup,
        import_alias_lookup,
        package_or_module_lookup,
        member_or_field_lookup,
        type_assisted_lookup,
        extension_lookup,
    ]:
        result = step(ref)
        record_resolution_step(ref, result)
        if result.is_exact():
            return result
    return unresolved_or_ambiguous(ref)
```

Do not force every language to implement every step immediately.

### Phase 6: Extension Merge

Allow repo-local Rust providers to emit:

- generated symbols;
- synthetic declarations;
- alias/reexport facts;
- framework references;
- resolution hints;
- suppressions/replacements of low-confidence facts.

Merge policy:

| Extension Operation | Allowed? | Rule |
|---|---|---|
| Add new symbol/reference/import facts | Yes | Validate stable key, span, language, owner, provenance. |
| Add candidates to unresolved reference | Yes | Confidence remains extension-labeled until validated. |
| Mark reference exact | Yes, gated | Requires referential validity and fixture/benchmark validation for high confidence. |
| Override native exact fact | Rare | Requires explicit conflict diagnostic and stronger validation. |
| Suppress native fact | Rare | Must retain suppressed fact in evidence side table. |

### Phase 7: Xref Index

Build a searchable index separate from the fact store:

```text
name -> occurrence ids
symbol -> reference ids
file -> occurrence ids
scope -> child scopes/symbols
module -> exported symbols
```

Use deterministic sorted vectors first. Add FST/compact encodings only when benchmark data justifies it.

## SDK Path

Initial user-facing shape:

```rust
#[polint::rule]
fn no_forbidden_reference(
    ctx: &mut RuleCtx<'_>,
    refs: References<'_>,
    symbols: Symbols<'_>,
) -> RuleResult {
    for r in refs.iter() {
        if r.resolution().is_exact() && symbols.get(r.symbol()).is_forbidden_api() {
            ctx.diagnostic(...);
        }
    }
    Ok(())
}
```

Add `Scopes<'_>` and `Imports<'_>` only when they are stable enough to document honestly.

## Language Order

1. **Go:** lexical scopes, package imports, declarations, `go/types`-compatible identity model, package object path semantics.
2. **TS/JS:** binder-like scopes, imports/exports/reexports, declaration merging where possible, CommonJS as conservative/dynamic facts.
3. **Python:** after module graph/type-alias research, copy Ty/Pyrefly-style place/use-def foundation.
4. **Java/JVM:** after module graph and type hierarchy research, copy JDT/WALA identity lessons.

## Cache Keys

Semantic layer keys must include:

```text
source content digest
parser version
language adapter version
semantic provider version
fact schema version
module/package lifecycle inputs
config affecting resolution
extension provider digest
input layer digests
```

Rule code digests should not invalidate syntax/scopes/imports unless rule options affect those providers.

## First Vertical Slice

Implement this before global call graph/data flow:

```text
Go + TS/JS:
  ScopeFact
  SymbolFact stable keys
  ReferenceFact roles/status
  ImportFact
  ResolutionFact audit trail
  xref index
  extension-added generated symbol fixture
```

This slice is small enough to verify but large enough to prove the kernel, extension merge, cache, SDK, and evaluation harness design.
