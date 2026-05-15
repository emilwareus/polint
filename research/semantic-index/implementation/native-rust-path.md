# Native Rust Implementation Path

## Principle

Implement the semantic index in Rust as typed fact providers over the analysis kernel. Do not depend on external semantic-index libraries, language servers, SCIP, Kythe, CodeQL, or Datalog engines for the core engine.

External tools can be used as validation oracles and research references.

## Internal Module Plan

```text
crates/polint/src/semantic/
  ids.rs
  fact.rs
  meta.rs
  scope.rs
  symbol.rs
  reference.rs
  import.rs
  alias.rs
  resolution.rs
  xref.rs
  extension.rs
  validation.rs
  providers/
    go.rs
    ts.rs
    python.rs
    java.rs
  export/
    scip.rs
    kythe.rs
```

Everything stays `pub(crate)` until promoted intentionally through `polint::sdk`.

## Data Structures

Use compact arenas:

```rust
pub(crate) struct ScopeId(u32);
pub(crate) struct SymbolId(u32);
pub(crate) struct ReferenceId(u32);
pub(crate) struct ImportId(u32);

pub(crate) struct SemanticFacts {
    pub(crate) scopes: Arena<ScopeFact>,
    pub(crate) symbols: Arena<SymbolFact>,
    pub(crate) references: Arena<ReferenceFact>,
    pub(crate) imports: Arena<ImportFact>,
    pub(crate) aliases: Vec<AliasFact>,
    pub(crate) resolutions: Vec<ResolutionFact>,
    pub(crate) meta: FactMetaStore,
}
```

Stable keys should be interned:

```rust
pub(crate) struct StableSymbolKey(SymbolInternerId);
pub(crate) struct StableReferenceKey(ReferenceInternerId);
```

Use sorted vectors and deterministic maps (`BTreeMap` where output order matters) until profiling requires specialized indexes.

## Provider DAG

```text
source
  -> go.syntax
  -> go.scopes
  -> go.imports
  -> go.symbols
  -> go.references

source
  -> ts.syntax
  -> ts.scopes
  -> ts.imports
  -> ts.symbols
  -> ts.references

imports + symbols
  -> semantic.alias_fixpoint

references + aliases + module graph
  -> semantic.resolution

semantic facts + extension facts
  -> semantic.extension_merge

merged facts
  -> semantic.xref_index
```

This graph should be represented as internal provider manifests from the analysis-kernel research.

## Extension Surface

Advanced repo-local Rust providers should receive typed read views and write to a validated sink:

```rust
pub(crate) trait SemanticExtensionProvider {
    fn id(&self) -> ExtensionProviderId;

    fn provide_semantic_facts(
        &self,
        input: SemanticExtensionInput<'_>,
        sink: &mut SemanticExtensionSink<'_>,
    ) -> ExtensionResult<()>;
}
```

Allowed writes:

- generated symbols;
- generated declarations;
- alias/reexport facts;
- synthetic references;
- candidate targets for unresolved references;
- exact resolution claims, gated by validation;
- suppression/replacement requests, gated by conflict validation.

## Go First

Go should be first because its semantic model is smaller than TS/Python/Java but still real:

- file/package scopes;
- imports and aliases;
- top-level declarations;
- method receivers;
- selectors as type-assisted later;
- test package variants;
- module roots and build tags as lifecycle inputs.

Initial exactness should be conservative:

```text
local variables: BinderExact
package imports: PackageExact when module graph resolved
selectors: Unknown or TypeAssisted only when type facts exist
dot imports: Conservative unless fully modeled
blank imports: import side-effect facts, no symbols
```

## TS/JS Second

TS/JS should copy the binder/checker split:

- lexical scopes;
- imports/exports/reexports;
- type/value namespace distinction;
- declaration merging facts;
- CommonJS as conservative/dynamic;
- generated/framework symbols through extensions.

Do not implement a full TypeScript checker in the first slice. Emit honest `Unresolved`, `Dynamic`, or `TypeRequired` statuses.

## Python Third

Python should start only after module graph research:

- scopes and symbol flags from Pyright/Ty;
- place/use-def from Ty/Pyrefly;
- import/package graph;
- dynamic attributes/generation through extension facts.

## Java/JVM Fourth

Java should start after module graph and type hierarchy research:

- package/import/static import facts;
- binding keys;
- classpath/module lifecycle;
- source/binary external symbols;
- type hierarchy provider.

## Export Later

Add export after stable keys:

```text
polint semantic export --format scip
polint semantic export --format kythe-json
```

Export adapters should be pure projections from internal facts and should not feed rule-time analysis.

## First Milestone Acceptance

- `SymbolFact`, `ReferenceFact`, `ScopeFact`, `ImportFact`, and `ResolutionFact` exist internally.
- Go and TS/JS emit scopes/imports/symbols/references with stable keys.
- JSON debug output can show resolution status and provenance.
- At least one extension fixture adds a generated symbol and resolves a previously unresolved reference.
- Cache tests prove extension digest invalidates dependent semantic layers.
- Existing `Symbols<'_>` and `References<'_>` SDK behavior remains compatible or is intentionally migrated with docs.
