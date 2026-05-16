# Research Analysis

## Protocol

Question: how should polint implement the first semantic-analysis bootstrap in
modern Rust?

In scope:

- local source-code review;
- Rust API and ownership/design implications;
- internal architecture for MIR, places, direct calls, P0 domains, summaries,
  cache keys, and extension sinks;
- sequencing and non-goals.

Out of scope:

- implementing product code in this pass;
- redoing the full external algorithm research from previous tracks;
- exposing public SDK views immediately.

Confidence labels:

- High: supported by local code inspection or official/local Rust guidance.
- Medium: strong architectural inference, but needs implementation validation.
- Low: plausible future direction.

## Current Code Strengths To Preserve

### 1. Narrow Public API Boundary

Confidence: High.

`crates/polint/src/lib.rs` exposes `runner`, `sdk`, and the `rule` macro, while
keeping `analysis_plan`, `cache`, `core`, parser adapters, graph builders, and
other implementation modules `pub(crate)`. This matches the repository
convention that public API is a liability.

Recommendation: add the semantic kernel as `pub(crate) mod analysis;`, not as a
public crate-root module. Public views should continue to live in
`polint::sdk::facts` only after validation.

### 2. Small Newtype IDs

Confidence: High.

The code already uses `FileId`, `FunctionId`, `PackageId`, `ImportId`,
`ModuleNodeId`, `SymbolId`, and related IDs as small `Copy`, ordered,
serializable newtypes. This is the right Rust shape for hot fact references:
cheap by value, type-safe, and index-friendly.

Recommendation: continue this pattern for semantic facts:

```rust
pub(crate) struct MirBodyId(pub u64);
pub(crate) struct MirOpId(pub u64);
pub(crate) struct PlaceId(pub u64);
pub(crate) struct CallSiteId(pub u64);
pub(crate) struct SummaryId(pub u64);
```

Use dense IDs for in-memory lookup, but pair them with stable keys for cache and
provenance.

### 3. Borrowed Typed SDK Views

Confidence: High.

`SourceFiles<'_>`, `Functions<'_>`, `Imports<'_>`, `Symbols<'_>`,
`References<'_>`, and other views are small `Copy` wrappers over `&AnalysisDb`.
They expose iterators and slices rather than moving or cloning facts.

This is idiomatic Rust and product-aligned: rule authors get ergonomic typed
query surfaces without seeing storage internals.

Recommendation: future `Calls<'_>`, `CallGraph<'_>`, `DataFlow<'_>`,
`Summaries<'_>`, and domain views should follow the same wrapper pattern, but
only after internal stores and validation mature.

### 4. Deterministic Builder Pattern

Confidence: High.

`SymbolGraphBuilder` uses `BTreeMap`/`BTreeSet`, stable keys, collision
diagnostics, and deterministic finish sorting. `ModuleGraphBuilder` uses a
similar draft/builder/finish model for graph facts.

Recommendation: semantic fact builders should copy this pattern:

1. adapters emit drafts;
2. builders normalize and validate;
3. merge gates detect duplicates/conflicts;
4. `finish()` returns sorted facts and diagnostics;
5. `AnalysisDb` or a new semantic store receives already-validated vectors.

### 5. Deterministic Cache Digest Encoding

Confidence: High.

`cache::keys` manually serializes config and rule inputs instead of trusting
generic JSON/TOML serialization ordering. This is exactly the right style for
analysis invalidation.

Recommendation: semantic cache keys should extend this manual deterministic
encoding with artifact kind, provider version, semantic schema, domain versions,
extension manifest digest, lifecycle inputs, and dependency digests.

### 6. Parallel Per-File Parsing Without Shared Mutation

Confidence: High.

Go and TS/JS adapters analyze files in parallel, collect per-file results, sort
by `FileId`, and restore facts into the main database. This avoids shared
mutable state in Rayon workers and preserves deterministic output.

Recommendation: keep the same shape for local MIR extraction. Cross-file
providers should consume deterministic stores after parsing, not mutate global
state from parallel workers.

## Current Code That Should Not Be Stretched Further

### 1. `AnalysisDb` Is Already Too Central For New Semantic Families

Confidence: High.

`AnalysisDb` currently holds files, syntax facts, imports, module graph facts,
symbol graph facts, metrics, components, literals, coverage, and path contexts.
It also owns cache extraction/restoration for per-file facts.

Adding MIR bodies, operations, places, call sites, abstract values, domain
states, summary facts, provider metadata, provenance records, extension facts,
and cache dependency records directly to this struct would create a high-change,
low-cohesion module.

Recommendation: introduce internal sub-stores:

```text
AnalysisDb
  files/syntax facts
  module graph
  symbol graph
  metrics
  semantic: SemanticStore
```

`SemanticStore` should be split internally by fact family. `AnalysisDb` can own
it, but `core/mod.rs` should not become the implementation file for the entire
analysis engine.

### 2. `FunctionFact.calls: Vec<String>` Is Not A Call Graph Seed

Confidence: High.

The current call information is a list of string names on `FunctionFact`. That
is useful as a syntactic hint, but it is missing:

- call-site ID;
- caller body/op;
- callee expression span;
- receiver/base expression;
- argument places;
- direct target symbol;
- unresolved reason;
- precision/status/provenance;
- language-specific call kind;
- evidence for later agent fixes.

Recommendation: do not evolve `FunctionFact.calls` into the semantic call
model. Keep it for compatibility if needed, and add a new internal
`CallSiteFact`/`DirectCallFact` family.

### 3. Dense IDs Alone Are Not Enough For Interprocedural Facts

Confidence: High.

Current per-file cache restore remaps function and branch IDs. This works for
file-local syntax facts. It is not enough for summary dependencies, call graph
edges, cross-file data-flow paths, or extension facts that need stable identity
across runs.

Recommendation: every interprocedural fact family needs a stable key. Dense IDs
remain efficient handles after the key is interned/validated.

### 4. File-Centric Cache Keys Are Insufficient For Summaries

Confidence: High.

The existing `CacheKey::for_file(...)` shape is good for parser results. Summary
artifacts are different: a function summary depends on a body, direct callees,
callee summaries, domain versions, lifecycle config, extension manifests, and
semantic schema.

Recommendation: add layer/artifact cache keys rather than forcing everything
through file cache entries.

### 5. `Capabilities` As Booleans Will Become A Maintenance Hotspot

Confidence: Medium.

The current bool struct is simple and readable. As fact families grow, it will
be easy to forget to update the macro, capability support, docs, plan digests,
and setup diagnostics together.

Recommendation: do not refactor this before the bootstrap. But when adding
semantic capabilities, centralize capability metadata behind an internal table
or generated list while preserving the macro-derived public rule model.

## Modern Rust Design Implications

### Visibility And API Liability

Confidence: High.

The repository already follows a modern Rust library posture: small public
surface, `unreachable_pub = "deny"`, and curated SDK re-exports.

Implementation rule: the semantic kernel should start private, even if its
names feel reusable. Internal names can be ugly or changeable; public names
must be supportable.

### Borrowing, Cloning, And Store Shape

Confidence: High.

The Rust skill emphasizes borrowing over cloning and `&str`/`&[T]` parameters.
The current SDK views follow that style. The danger zone is cache extraction:
`facts_for_file()` clones file-local facts. That is acceptable for today, but
semantic facts may be much larger.

Recommendation:

- hot query APIs return `&T`, slices, or iterators;
- cache serialization creates owned export records at the boundary;
- stable keys are interned or stored once when repeated heavily;
- do not clone source text or AST-derived strings in loops without profiling.

### Static Dispatch Versus Dynamic Dispatch

Confidence: High.

The Rust skill recommends static dispatch where performance matters and dynamic
dispatch where plugin/runtime polymorphism is essential. The first semantic
kernel should avoid a trait-object-heavy provider graph in the hot path.

Recommendation:

- native providers: enum-driven or direct functions scheduled by a provider DAG;
- domain law tests: generic over a `Domain` trait where monomorphization helps;
- extension boundary: process-isolated protocol or `dyn` at the boundary only;
- SDK views: concrete wrapper types, not trait objects.

### Typed Errors

Confidence: High.

The code uses `anyhow` effectively at CLI/setup boundaries. For the semantic
kernel, typed errors are more valuable because callers need to distinguish
validation failure, missing provider inputs, unsupported language semantics,
cache decode mismatch, extension rejection, and internal invariant failure.

Recommendation:

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AnalysisError {
    #[error("required fact family {family} is unavailable")]
    MissingInput { family: &'static str },

    #[error("provider {provider} emitted invalid facts: {reason}")]
    InvalidProviderOutput { provider: &'static str, reason: String },

    #[error("extension {extension} fact was rejected: {reason}")]
    ExtensionRejected { extension: String, reason: String },
}
```

Return diagnostics for user-facing setup gaps; reserve errors for engine
invariants or failed internal operations.

### Typestate

Confidence: Medium.

Typestate is useful when invalid state transitions are common and valuable to
prevent at compile time. It would be overkill for every fact family.

Recommendation: use typestate sparingly, likely for extension registration or
provider build/validate/commit stages:

```rust
ExtensionBatch<Collected>
ExtensionBatch<Validated>
ExtensionBatch<Committed>
```

Do not use typestate for normal fact rows or domain states; it will add generic
noise and slow iteration.

### Thread Safety

Confidence: High.

The current parallel adapters avoid shared mutable state. Continue this.

Recommendation:

- `Arc<str>` for shared source text is good.
- Avoid `Arc<Mutex<SemanticStore>>` in provider workers.
- Use local sinks per worker/provider, then deterministic merge.
- Keep parser/allocator lifetimes local to adapters.

## Algorithmic And Complexity Notes

### Provider Scheduling

Model: deterministic DAG over requested fact families.

Expected complexity:

- building dependency graph: `O(P + D)` for providers and dependency edges;
- topological scheduling: `O(P + D)`;
- running providers: dominated by provider-specific work;
- merge sorting: usually `O(F log F)` per fact family if builders use maps or
  final sorting.

Recommendation: make provider dependencies explicit and inspectable in tests.
Avoid implicit phase order as the number of analyses grows.

### MIR Extraction

Model: per-function semantic operation sequence plus CFG references.

Expected complexity:

- extraction: `O(AST nodes visited)`;
- memory: `O(operations + operands + spans)`;
- no interprocedural fixpoint.

Recommendation: the first MIR should be intentionally incomplete but
semantically honest. Unsupported constructs should emit unsupported/unknown
operations, not silently disappear.

### Place Identity

Model: stable access-path identities plus dense `PlaceId`.

Expected complexity:

- local place collection: near `O(uses * path_length)`;
- canonicalization via deterministic map: `O(P log P)` with `BTreeMap`, or
  expected `O(P)` if a hash map is later justified by profiling.

Recommendation: start with deterministic maps unless profiling shows they are a
problem. Debuggability and reproducibility matter more in the first slice.

### Direct Call Facts

Model: call sites with direct syntactic/static binding when known.

Expected complexity:

- extraction: `O(call expressions)`;
- optional symbol binding: depends on semantic index lookup, usually
  `O(log symbols)` or index-based lookup.

Recommendation: direct call facts are the bridge between syntax and later call
graph tiers. They should carry unresolved reasons from day one.

### P0 Local Domains

Model: forward transfer over MIR/CFG for reachability, constants, nullish, and
truthiness.

Expected complexity:

- acyclic local body: near `O(edges * transfer_cost)`;
- loops: `O(edges * iterations * transfer_cost)`, bounded by finite height or
  widening;
- reduced product: adds reduction cost per join/transfer point.

Recommendation: property-test lattice laws before optimizing. For P0 domains,
precision bugs are more dangerous than raw speed bugs.

### Direct Summaries

Model: context-insensitive per-function summary without refined global call
graph.

Expected complexity:

- local summary: same order as local domain pass;
- direct same-SCC iteration: `O(SCC_edges * iterations * transfer_cost)`;
- cache dependency checking: proportional to dependency digest count.

Recommendation: start with direct summaries only. Refined call graph and global
data flow come after the summary contract is validated.

## Rejected Paths

### Rejected: Put All New Facts Directly In `core::AnalysisDb`

Reason: it maximizes short-term speed but creates a central module with too many
reasons to change. It also makes cache, provenance, extension merges, and SDK
views harder to evolve independently.

### Rejected: Expand `FunctionFact.calls` Into A Call Graph

Reason: string call lists cannot carry the identity/provenance/precision needed
for call graphs, data flow, summaries, and agent extension.

### Rejected: Build A General Salsa-Like Query Engine First

Reason: prior analysis-kernel research already recommends a hybrid approach.
The first slice needs deterministic provider scheduling and artifact keys, not
a whole query engine.

### Rejected: Trait-Object Provider Graph Everywhere

Reason: dynamic dispatch is useful at extension boundaries, but it is not needed
for native P0 providers. It would make hot-path performance and type-specific
testing worse too early.

### Rejected: Public SDK Views Before Internal Validation

Reason: public views are contract. The repo already reserves `Cfg`,
`CallGraph`, and `DataFlow` as unsupported views. Keep using that pattern.

## Highest-Risk Open Questions

1. Should stable semantic keys live as strings, hashed IDs plus optional debug
   strings, or interned keys?
   - Recommendation: start with string stable keys plus hashed dense IDs, then
     profile memory before interning.
2. Should `SemanticStore` live inside `AnalysisDb` or beside it?
   - Recommendation: inside `AnalysisDb` ownership-wise, separate module-wise.
3. Should P0 domains be generic traits or concrete enums?
   - Recommendation: concrete built-ins for runtime; generic traits for law
     tests and extension validation helpers.
4. How much extension infrastructure belongs in the first slice?
   - Recommendation: only typed sinks and validation fixtures, no dynamic
     loading yet.
