# Phase 13 Research: Symbols And References

Date checked: 2026-05-12

## Executive Recommendation

Build Phase 13 as a compiler-backed symbol/reference index, not as another
syntax extractor.

The next implementation step should be:

1. Add polint-owned public fact types and typed SDK views for symbols,
   definitions, and references.
2. Add an internal `symbol_graph` derivation stage that runs only when
   `symbols` or `references` are requested by the `AnalysisPlan`.
3. Fill TS/JS facts from `oxc_semantic` first.
4. Fill Go facts from a small Go sidecar that uses `golang.org/x/tools/go/packages`
   and `go/types`.
5. Make all facts carry explicit precision/status fields so rules and agents
   know whether an answer is exact, module-linked, heuristic, unresolved,
   ambiguous, setup-missing, or unsupported.

Do this before call graphs, CFG, and dataflow. Stable symbols are the identity
layer those later analyses need.

## Phase Context

Phase 13 goal: expose stable definitions, symbols, and references.

Requirements:

- SYM-01: Rule authors can read symbol, definition, and reference facts through
  typed SDK fact views.
- SYM-02: Go symbols/references are populated from typed package information
  where setup is available.
- SYM-03: TS/JS symbols/references are populated from Oxc semantic facts where
  setup is available.
- SYM-04: Facts expose precision tiers and stable IDs suitable for diagnostics
  and cache restore.

Current codebase state:

- `crates/polint/src/core/mod.rs` owns fact structs, IDs, `AnalysisDb`, and
  `Capabilities`.
- `crates/polint/src/sdk/facts.rs` owns typed rule-author views.
- `crates/polint-macros/src/lib.rs` maps typed view parameters to capability
  names.
- `crates/polint/src/module_graph/*` is the strongest existing pattern for a
  cross-file derivation stage.
- TS/JS parsing already depends on Oxc and the workspace already includes
  `oxc_semantic = 0.129.0`, but the TS adapter currently extracts only
  syntax-level facts.
- Go parsing is tree-sitter-only today, so true Go symbol/reference precision
  requires a language-native sidecar.

## Latest Research Signals

### AI-oriented repository understanding is moving toward stable semantic indexes

The 2026 AOCI paper describes a persistent "symbolic-semantic blueprint" for
LLM-driven code comprehension and emphasizes persistent, incremental codebase
knowledge instead of one-shot context stuffing:
https://arxiv.org/abs/2605.02421

The product lesson for polint: expose a stable semantic index that agents can
query directly. Do not make agents infer architecture from raw AST dumps or
diagnostic text.

### TypeScript repository indexing research favors compiler-backed indexing over LSP-style lookup loops

The 2026 TypeScript Repository Indexing paper reports large speedups over a
language-server-first indexing baseline by using the TypeScript compiler API as
the indexing engine:
https://arxiv.org/abs/2604.18413

The product lesson for polint: Oxc is the right first engine for fast local
lexical symbols, but the "max power" TS path should keep room for a later
TypeScript compiler sidecar when rules need type-checker-level cross-file and
member resolution.

### CPG plus high-level agent tools is a strong pattern

The 2026 CodeBadger paper combines Joern's Code Property Graph with high-level
MCP tools for LLM agents:
https://arxiv.org/abs/2603.24837

The product lesson for polint: do not expose only a low-level graph. Expose
agent-friendly typed queries such as "references to symbol", "exported API
surface", "callers of function", "paths from source to sink", and "facts with
unresolved precision".

### Scalable taint work points toward explicit dependency graphs and summaries

Recent language-agnostic taint research focuses on whole-program dependency
graphs and external-library over-approximations:
https://arxiv.org/abs/2506.06247

The product lesson for polint: dataflow should come after symbols, CFG, and
call graph. It should be summary-driven, with explicit precision, not a pile of
ad hoc source/sink regexes.

## Competing Products And What To Learn

| Product | What it does well | Relevant limit for polint's wedge |
|---|---|---|
| Semgrep | Fast structural matching, strong rule ecosystem, taint mode. Official docs describe cross-function, cross-file, and Pro engine capabilities. Sources: https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview and https://semgrep.dev/docs/semgrep-code/semgrep-pro-engine-intro | YAML/pattern rules are excellent for generic checks but are not a repo-local Rust SDK for arbitrary codebase-specific quality models. Advanced interfile analysis is product-tier-dependent. |
| OpenGrep | Open-source continuation/fork of Semgrep-compatible scanning. Source: https://www.opengrep.dev/ | Important OSS competitor for pattern and taint scanning. Differentiation should be typed repo-specific analysis facts, Rust rules, and AI-agent consumability rather than only rule syntax. |
| CodeQL | Mature database-backed semantic analysis with local/global dataflow and taint libraries. Source: https://codeql.github.com/docs/writing-codeql-queries/about-data-flow-analysis/ | Very powerful, but requires its own QL/database model. polint should learn from its separation of local vs global dataflow while keeping repo-local Rust rule ergonomics. |
| Joern / Code Property Graph | Proven graph model combining AST, CFG, PDG, and semantic layers. Source: https://docs.joern.io/code-property-graph/ | Strong security-analysis model, but heavy graph/query stack. polint should adopt the layered graph idea, not the operational complexity. |
| Sourcegraph SCIP / LSIF | Stable symbol occurrence formats for code navigation. Sources: https://github.com/sourcegraph/scip and https://microsoft.github.io/language-server-protocol/specifications/lsif/0.6.0/specification/ | Excellent inspiration for symbol IDs and occurrence roles. Not a policy/rule execution platform by itself. |
| GitHub Stack Graphs | Incremental name-resolution model based on path finding through per-file graphs. Source: https://github.com/github/stack-graphs | Strong fallback model for languages without compiler facts. The repo was archived in 2025, so treat it as design inspiration, not a primary dependency. |
| SonarQube, CodeScene, Snyk Code | Mature quality/security products with dashboards and generic rules. | They sell useful quality signals, but their core weakness is generic analysis. polint's wedge is executable, repo-specific knowledge that agents can consume. |

## Standard Stack

Use these engines and keep their output behind polint-owned facts.

| Area | Use | Why |
|---|---|---|
| TS/JS local symbols and references | `oxc_semantic::SemanticBuilder` | Already in the workspace. Oxc semantic exposes scoping, symbols, symbol spans, symbol flags, resolved reference IDs, root unresolved references, and optional CFG. Source: local crate `oxc_semantic-0.129.0`; docs: https://docs.rs/oxc_semantic/latest/oxc_semantic/ |
| TS/JS import/module linking | Existing Phase 12 `module_graph` and `oxc_resolver` path | Cross-file imports should link through already-derived resolved imports and module nodes, not a second resolver. |
| TS/JS future max precision | TypeScript compiler API sidecar | Needed later for type checker, project references, declaration files, and member/property resolution beyond Oxc lexical facts. Source: https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API |
| Go typed symbols and references | Go sidecar using `golang.org/x/tools/go/packages` plus `go/types` | `packages.Load` is the official project-aware package loader; `go/types.Info` gives `Defs`, `Uses`, `Selections`, `Scopes`, and `Implicits`. Sources: https://pkg.go.dev/golang.org/x/tools/go/packages and https://pkg.go.dev/go/types#Info |
| Go stable package object keys | `golang.org/x/tools/go/types/objectpath` where possible | Gives stable paths for package-level `types.Object` values. Source: https://pkg.go.dev/golang.org/x/tools/go/types/objectpath |
| Future Go call graph | `golang.org/x/tools/go/callgraph/static`, CHA/RTA, then pointer analysis where needed | The Go tools already provide call graph algorithms; do not invent them in Rust. Source: https://pkg.go.dev/golang.org/x/tools/go/callgraph/static |
| Graph storage | Existing vectors in `AnalysisDb` plus internal indexes | Matches the current codebase and keeps public SDK simple. Do not introduce a graph DB. |
| Stable IDs | polint-owned deterministic hash of normalized stable keys | Oxc and Go object IDs are process-local or tool-local. They must not leak as public IDs. |

## Architecture Patterns

### 1. Add a cross-file symbol derivation stage

Create a module shaped like Phase 12:

```text
crates/polint/src/symbol_graph/
  mod.rs          orchestration and capability support
  model.rs        builder, stable keys, sorting, precision/status enums
  ts.rs           Oxc semantic adapter and TS import/export linking
  go.rs           Go sidecar invocation and JSON conversion
  query.rs        internal indexes used by SDK views
  stable_id.rs    stable hashing and collision checks
```

The runner sequence should become:

```text
discover files
build AnalysisPlan from rule fact-view parameters
run syntax adapters
derive module graph if requested or needed by symbols/references
derive symbol graph if symbols/references/call_graph/dataflow are requested
derive later analyses from symbols
run rules
```

`symbol_graph::derive_requested_symbols(db, loaded, plan)` should return the
same kind of object as `ModuleGraphDerivation`: diagnostics plus capability
support overrides.

### 2. Keep the public API small

Public rule-author surface should be:

- `polint::sdk::prelude::*`
- `Symbols<'_>`
- `References<'_>`
- `Definitions<'_>` only if it clearly improves ergonomics; otherwise
  `Symbols::definition` and `Symbols::definitions`.

Do not expose Oxc scoping, Go `types.Object`, Go package loader output, sidecar
JSON, or raw AST nodes.

### 3. Use SCIP-like occurrence thinking, but keep polint facts domain-specific

SCIP's useful model is "symbols plus occurrences with roles". polint should
translate that into:

- `SymbolFact`: stable semantic identity.
- `DefinitionFact`: where a symbol is declared or defined. Multiple definitions
  are allowed for TS declaration merging and Go implicit objects.
- `ReferenceFact`: where a symbol-like name is used.

Do not collapse definitions into references. Rules often need to ask different
questions about declarations, public API, reads, writes, calls, imports, and
exports.

### 4. Split status from precision

Use both:

- `SymbolPrecision`: how trustworthy the binding is.
- `SymbolResolutionStatus`: whether a reference is resolved, unresolved,
  ambiguous, setup-missing, or unsupported.

This is more useful to agents than a single boolean.

Recommended precision values:

```rust
pub enum SymbolPrecision {
    ExactSemantic,
    ExactLocal,
    ModuleLinked,
    Heuristic,
    Unresolved,
    Ambiguous,
    SetupMissing,
    Unsupported,
}
```

Recommended reference status values:

```rust
pub enum SymbolResolutionStatus {
    Resolved,
    Unresolved,
    Ambiguous,
    SetupMissing,
    Unsupported,
}
```

### 5. Stable IDs must be semantic, not vector positions

Existing fact IDs such as `FunctionId` are deterministic within one run, but
SYM-04 needs restore-friendly symbol identity. `SymbolId` and `ReferenceId`
should be hash IDs derived from stable keys, then facts should still be sorted
deterministically in vectors.

Use a deterministic hash function or the existing stable fingerprint helper;
do not use Rust's randomized `DefaultHasher`.

Recommended stable key inputs:

- language
- package path or module node identity for externally addressable symbols
- package/test variant where relevant
- file-relative path for local-only symbols only
- lexical owner chain
- symbol namespace: value, type, namespace, package, module
- symbol kind
- symbol name
- primary definition span

Store the stable key string or a debug-safe digest input in `SymbolFact` during
early versions. It makes collisions and moved-symbol behavior diagnosable.

Package-level Go symbols should prefer `objectpath`-style identity so a move
inside the same package does not automatically change the symbol ID. Local
function variables can include file path and lexical span because their stable
external identity is weaker.

### 6. Capability semantics

Add two public capabilities:

- `symbols`: definitions and symbol identities.
- `references`: symbol identities plus reference facts.

`references` should be treated as depending on `symbols` inside the analysis
planner/provider. A rule that asks only for `References<'_>` should still get
the symbols needed to resolve `ReferenceFact::target`.

The symbol provider should also be allowed to request module graph derivation
internally because TS/JS import/export linking depends on resolved module
relationships. That dependency should stay an engine detail; rule authors
should not have to request `ModuleGraphFacts<'_>` just to get cross-file symbol
references.

## Data Model Recommendation

Add fact IDs:

```rust
pub struct SymbolId(pub u64);
pub struct DefinitionId(pub u64);
pub struct ReferenceId(pub u64);
```

Add core facts:

```rust
pub struct SymbolFact {
    pub id: SymbolId,
    pub language: Language,
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub namespace: SymbolNamespace,
    pub file: Option<FileId>,
    pub package: Option<PackageId>,
    pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>,
    pub primary_span: Option<Span>,
    pub stable_key: String,
    pub precision: SymbolPrecision,
}

pub struct DefinitionFact {
    pub id: DefinitionId,
    pub symbol: SymbolId,
    pub file: FileId,
    pub span: Span,
    pub kind: DefinitionKind,
    pub is_primary: bool,
    pub precision: SymbolPrecision,
}

pub struct ReferenceFact {
    pub id: ReferenceId,
    pub file: FileId,
    pub span: Span,
    pub name: String,
    pub target: Option<SymbolId>,
    pub candidates: Vec<SymbolId>,
    pub kind: ReferenceKind,
    pub status: SymbolResolutionStatus,
    pub precision: SymbolPrecision,
}
```

Recommended symbol kinds:

- `Package`
- `Module`
- `File`
- `Function`
- `Method`
- `Class`
- `Interface`
- `TypeAlias`
- `Enum`
- `EnumMember`
- `Variable`
- `Constant`
- `Parameter`
- `Field`
- `Property`
- `Namespace`
- `Import`
- `Export`
- `Unknown`

Recommended reference kinds:

- `Read`
- `Write`
- `ReadWrite`
- `Call`
- `TypeUse`
- `Import`
- `Export`
- `MemberAccess`
- `Assignment`
- `DeclarationUse`
- `Unknown`

SDK views:

```rust
impl<'a> Symbols<'a> {
    pub fn all(self) -> &'a [SymbolFact];
    pub fn get(self, symbol: SymbolId) -> Option<&'a SymbolFact>;
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a SymbolFact>;
    pub fn by_name(self, name: &str) -> impl Iterator<Item = &'a SymbolFact>;
    pub fn definition(self, symbol: SymbolId) -> Option<&'a DefinitionFact>;
    pub fn definitions(self, symbol: SymbolId) -> impl Iterator<Item = &'a DefinitionFact>;
    pub fn exported(self) -> impl Iterator<Item = &'a SymbolFact>;
}

impl<'a> References<'a> {
    pub fn all(self) -> &'a [ReferenceFact];
    pub fn to(self, symbol: SymbolId) -> impl Iterator<Item = &'a ReferenceFact>;
    pub fn for_file(self, file: FileId) -> impl Iterator<Item = &'a ReferenceFact>;
    pub fn unresolved(self) -> impl Iterator<Item = &'a ReferenceFact>;
    pub fn ambiguous(self) -> impl Iterator<Item = &'a ReferenceFact>;
}
```

Internal indexes should live outside the public facts:

- `symbols_by_id: BTreeMap<SymbolId, usize>`
- `definitions_by_symbol: BTreeMap<SymbolId, Vec<usize>>`
- `references_by_symbol: BTreeMap<SymbolId, Vec<usize>>`
- `symbols_by_file: BTreeMap<FileId, Vec<usize>>`
- `references_by_file: BTreeMap<FileId, Vec<usize>>`
- `symbols_by_name: BTreeMap<String, Vec<usize>>`

Do not force SDK view methods to scan all facts once the dataset grows.

## Go Implementation

### Why a Go sidecar is required

Rust can parse Go syntax with tree-sitter, but exact Go symbol binding depends
on the Go type checker and module/package loading. The right source of truth is
`go/packages.Load` with modes that include syntax, types, and types info.

Recommended sidecar:

```text
tools/polint-go-symbols/
  go.mod
  main.go
  internal/emit/...
```

Rust invokes it only when a plan requests `symbols`, `references`, `call_graph`,
or `dataflow` for Go.

Command shape:

```text
polint-go-symbols symbols \
  --root <repo-root> \
  --patterns ./... \
  --tests=true \
  --build-tags <tags> \
  --json
```

Use `packages.Load` with:

- `NeedName`
- `NeedFiles`
- `NeedCompiledGoFiles`
- `NeedImports`
- `NeedSyntax`
- `NeedTypes`
- `NeedTypesInfo`
- `NeedTypesSizes`
- `NeedModule` where available

Avoid loading full dependency syntax in Phase 13 unless the rule needs it.
Future call graph/dataflow can opt into heavier modes.

### Go fact extraction algorithm

For each loaded package:

1. Build a map from absolute file path to polint `FileId`.
2. Walk `pkg.TypesInfo.Defs`.
   - Each non-nil `types.Object` becomes a `SymbolFact` and `DefinitionFact`.
   - Package-level objects use `objectpath` where possible.
   - Local objects use package ID, file path, lexical owner chain, name, and
     position.
3. Walk `pkg.TypesInfo.Uses`.
   - Each identifier becomes a `ReferenceFact` with `target = Some(SymbolId)`.
4. Walk `pkg.TypesInfo.Selections`.
   - Selector expressions become field/method `ReferenceFact`s.
   - This is required for method calls and field ownership.
5. Walk `pkg.TypesInfo.Implicits`.
   - Emit implicit definitions with precision `ExactSemantic`.
6. Use `pkg.TypesInfo.Scopes` to compute lexical owner paths for local stable
   keys.
7. Convert token positions to byte/line spans before returning JSON.

If `packages.Load` returns partial package errors, keep facts that are exact and
emit capability diagnostics describing the package errors. Do not silently
return an empty symbol set.

### Go setup/caching rules

Cache keys must include:

- sidecar version
- Go version
- `go env GOMOD`, `GOWORK`, `GOOS`, `GOARCH`, `CGO_ENABLED`
- build tags
- `go.mod`, `go.sum`, `go.work`, and workspace file content hashes
- package patterns
- `tests` mode
- relevant source file content hashes

If `go` is unavailable, report `symbols`/`references` as setup-missing for Go
and keep syntax-level facts available.

## TS/JS Implementation

### Use Oxc semantic first

Oxc semantic already exposes the exact local data needed for Phase 13:

- `SemanticBuilder::new().with_check_syntax_error(true).build(&program)`
- `semantic.scoping().symbol_ids()`
- `symbol_name`, `symbol_span`, `symbol_flags`, `symbol_scope_id`,
  `symbol_declaration`, `symbol_redeclarations`
- `semantic.symbol_references(symbol_id)`
- `scoping.root_unresolved_references()`
- `reference.flags()` and `reference.node_id()`

This provides exact lexical symbols and local references within a file. It does
not by itself provide TypeScript type-checker project-wide member resolution.

### TS/JS fact extraction algorithm

For each TS/JS file:

1. Parse with the existing Oxc parser path.
2. Build Oxc semantic information for the parsed program.
3. For each Oxc symbol:
   - Map `SymbolFlags` to `SymbolKind` and `SymbolNamespace`.
   - Use the Oxc declaration span as the primary span.
   - Build a local stable key from module/file, scope path, flags, name, and
     span.
   - Emit `SymbolFact`.
   - Emit one or more `DefinitionFact`s for redeclarations.
4. For each resolved Oxc reference:
   - Use `reference.node_id()` to get the AST node span.
   - Map `ReferenceFlags` to `ReferenceKind`.
   - Emit `ReferenceFact { target: Some(symbol_id), status: Resolved }`.
5. For root unresolved references:
   - Emit explicit unresolved facts.
   - Treat globals and missing symbols as unresolved until a later global
     environment model exists.
6. For imports/exports:
   - Map Oxc import binding symbols to existing `ImportFact`s where possible.
   - Use Phase 12 `ResolvedImportFact` and `ModuleGraphFacts` to connect import
     aliases to exported symbols in the resolved target module.
   - Mark those cross-file links `ModuleLinked`, not `ExactSemantic`.

### Future TS/JS max-precision path

Add a TypeScript compiler sidecar later when rules need:

- project references
- declaration files
- path aliases with type checker semantics
- symbol identity across source and `.d.ts`
- property/member resolution through inferred types
- JS with `checkJs`

Do not block Phase 13 on that. Oxc gives useful high-signal local symbols now.

## Sequencing Toward Max Power

Recommended order after Phase 13:

1. Symbol/reference index.
   - Identity layer for everything else.
2. Export/API surface and ownership rules.
   - Immediate product value from symbols plus module graph.
3. Direct call graph.
   - Go: use Go tools call graph packages.
   - TS/JS: start with call expressions bound to symbol references.
4. Per-function CFG.
   - TS/JS: adapt Oxc CFG where available.
   - Go: build from Go syntax/type info or sidecar output.
5. Local dataflow.
   - Intraprocedural reaching definitions/use-def chains over CFG.
6. Interprocedural summaries.
   - Function summaries, source/sink propagation, return/parameter effects.
7. Taint and policy packs.
   - Semgrep/CodeQL-like power, but with repo-local Rust rule ergonomics.
8. Agent graph API.
   - High-level queries over the graph, not raw dumps.

This avoids boiling the ocean while keeping the architecture pointed at a full
static-analysis platform.

## Don't Hand-Roll

- Do not hand-roll Go name resolution. Use `go/packages` and `go/types`.
- Do not hand-roll TS lexical scoping. Use `oxc_semantic`.
- Do not expose Oxc IDs or Go object addresses as public IDs.
- Do not add a graph database for Phase 13.
- Do not make rules parse raw ASTs or sidecar JSON.
- Do not claim exact TS cross-file member resolution until a TypeScript
  type-checker-backed mode exists.
- Do not report setup-missing capability requests as empty fact arrays.
- Do not make `SymbolId` a vector index if it is advertised as stable.
- Do not build CFG/dataflow before symbol identity exists.

## Common Pitfalls

- Import alias precision: an imported TS binding can be exact locally but only
  module-linked across files unless the target export is resolved.
- Declaration merging: TS can have multiple declarations for one symbol.
- Go test variants: `packages.Load` can produce normal and test package
  variants; stable keys must include the package/test identity.
- Generated files: facts need source paths and precision that make generated
  origins visible to rules.
- Globals: Oxc unresolved root references may be real globals or missing names.
  Do not overclaim.
- Caching: Go symbol facts are package-level, not cleanly per-file. Cache by
  package/setup digest, not only by individual source file.
- Diagnostics: setup errors must mention the language analyzer that failed.
- Performance: sidecars should run only when the plan requests deeper facts.
- Public API: expose only typed SDK views and fact structs intended for
  downstream rule authors.

## Code Examples

Capability additions:

```rust
pub struct Capabilities {
    pub symbols: bool,
    pub references: bool,
    // existing fields...
}

impl Capabilities {
    pub fn symbols(mut self) -> Self {
        self.symbols = true;
        self
    }

    pub fn references(mut self) -> Self {
        self.references = true;
        self
    }
}
```

Macro mapping:

```rust
match segment.ident.to_string().as_str() {
    "Symbols" => "symbols",
    "References" => "references",
    // existing views...
}
```

Runner orchestration shape:

```rust
let mut module_derivation = ModuleGraphDerivation::default();
if plan.requests_any_capability(&["resolved_imports", "module_graph", "symbols", "references"]) {
    module_derivation = module_graph::derive_requested_module_graph(&mut db, &loaded, &plan);
}

let symbol_derivation = symbol_graph::derive_requested_symbols(&mut db, &loaded, &plan);
```

TS extraction shape:

```rust
let semantic = SemanticBuilder::new()
    .with_check_syntax_error(true)
    .build(&program);
let scoping = semantic.semantic.scoping();

for oxc_symbol in scoping.symbol_ids() {
    let name = scoping.symbol_name(oxc_symbol);
    let span = scoping.symbol_span(oxc_symbol);
    // map to SymbolFact and DefinitionFact

    for reference in semantic.semantic.symbol_references(oxc_symbol) {
        let node_span = semantic
            .semantic
            .nodes()
            .get_node(reference.node_id())
            .kind()
            .span();
        // map to ReferenceFact
    }
}
```

Go sidecar shape:

```go
cfg := &packages.Config{
    Mode: packages.NeedName |
        packages.NeedFiles |
        packages.NeedCompiledGoFiles |
        packages.NeedImports |
        packages.NeedSyntax |
        packages.NeedTypes |
        packages.NeedTypesInfo |
        packages.NeedTypesSizes,
    Dir: root,
    Tests: includeTests,
}

pkgs, err := packages.Load(cfg, patterns...)
```

Go extraction shape:

```go
for ident, obj := range pkg.TypesInfo.Defs {
    if obj == nil {
        continue
    }
    emitDefinition(pkg, ident, obj)
}

for ident, obj := range pkg.TypesInfo.Uses {
    emitReference(pkg, ident, obj)
}

for selector, selection := range pkg.TypesInfo.Selections {
    emitSelectorReference(pkg, selector, selection.Obj())
}
```

## Verification Plan

Add external-consumer tests that generate a temp repo with `.polint/rules` and
use only `polint::sdk::prelude::*`.

Minimum tests:

- TS local variable definition and references.
- TS import alias linking through a resolved local file import.
- TS unresolved global/missing reference is visible as unresolved.
- TS declaration merging produces multiple definitions.
- Go package function definition and call reference.
- Go method selector reference.
- Go package load failure produces setup-missing diagnostics, not silent empty
  facts.
- Rule parameter `Symbols<'_>` maps to `symbols`.
- Rule parameter `References<'_>` maps to `references`.
- Stable `SymbolId` survives cache restore for unchanged source.
- Moving an unrelated file does not change a package-level Go symbol ID.

## Source Notes

Primary sources checked:

- Oxc semantic docs and local crate source:
  https://docs.rs/oxc_semantic/latest/oxc_semantic/
- Go package loader:
  https://pkg.go.dev/golang.org/x/tools/go/packages
- Go type checker info:
  https://pkg.go.dev/go/types#Info
- Go object paths:
  https://pkg.go.dev/golang.org/x/tools/go/types/objectpath
- CodeQL dataflow:
  https://codeql.github.com/docs/writing-codeql-queries/about-data-flow-analysis/
- CodeQL JS/TS dataflow:
  https://codeql.github.com/docs/codeql-language-guides/analyzing-data-flow-in-javascript-and-typescript/
- Semgrep taint and Pro engine:
  https://semgrep.dev/docs/writing-rules/data-flow/taint-mode/overview
  https://semgrep.dev/docs/semgrep-code/semgrep-pro-engine-intro
- OpenGrep:
  https://www.opengrep.dev/
- SCIP:
  https://github.com/sourcegraph/scip
- LSIF:
  https://microsoft.github.io/language-server-protocol/specifications/lsif/0.6.0/specification/
- Stack Graphs:
  https://github.com/github/stack-graphs
- Joern CPG:
  https://docs.joern.io/code-property-graph/
- TypeScript Compiler API:
  https://github.com/microsoft/TypeScript/wiki/Using-the-Compiler-API
- AOCI paper:
  https://arxiv.org/abs/2605.02421
- TypeScript repository indexing paper:
  https://arxiv.org/abs/2604.18413
- CodeBadger paper:
  https://arxiv.org/abs/2603.24837
- Language-agnostic taint paper:
  https://arxiv.org/abs/2506.06247
