# Final Report

## Practical Conclusion

The next implementation should be a private Rust analysis-kernel bootstrap, not
a public call graph/data-flow feature and not a large expansion of
`core::AnalysisDb`.

The correct first slice is:

```text
analysis module
  -> stable semantic IDs and FactMeta sidecars
  -> semantic MIR extraction
  -> PlaceId / PlaceKey substrate
  -> CallSiteFact / DirectCallFact
  -> P0 local domains
  -> direct summaries
  -> artifact cache keys
  -> extension sinks with validation, but no dynamic loading yet
```

Public SDK views come later, after fixtures, docs, cache tests, and temp-repo
rule tests.

## Why This Is The Right Shape

polint already has good Rust foundations:

- a narrow public API (`runner`, `sdk`, `rule`);
- small `Copy` ID newtypes;
- borrowed typed SDK views;
- deterministic builders for module and symbol graphs;
- deterministic cache digest encoding;
- per-file parallel parsing with deterministic merge;
- macro-derived rule capabilities.

The implementation risk is that the next semantic features are much denser than
today's facts. MIR, places, values, direct call facts, abstract domains,
summaries, provenance, and extension facts will multiply storage and dependency
edges. If they are added as one more set of vectors and ad hoc methods in
`AnalysisDb`, the engine will become hard to cache, hard to validate, and hard
to expose safely.

## Code Review Findings

### Preserve: public API discipline

`crates/polint/src/lib.rs` keeps almost everything internal and documents that
rule authors primarily use `sdk` and `runner`. This should stay true for the
semantic kernel. The kernel should start as `pub(crate) mod analysis;`.

### Preserve: ID and view style

`core::FileId`, `FunctionId`, `SymbolId`, and related IDs are small `Copy`
newtypes. `sdk::facts` exposes small borrowed views. This is idiomatic Rust and
fits high-volume fact querying.

### Preserve: stable-key builder style

`symbol_graph::model` and `symbol_graph::stable_id` are the strongest local
precedent for future semantic facts: build from drafts, create stable keys,
detect collisions, sort output, and emit diagnostics.

### Preserve: deterministic parallelism

Go and TS adapters avoid shared mutable state by using local per-file databases,
sorting results, and restoring facts. Semantic MIR extraction should use the
same shape.

### Change: do not stretch `FunctionFact.calls`

`FunctionFact.calls: Vec<String>` is not enough for call graphs or summaries. It
lacks call-site identity, spans, argument places, receiver/base, direct target,
precision, unresolved reason, and provenance.

Add a new semantic call fact family instead.

### Change: do not make `AnalysisDb` the semantic engine

`AnalysisDb` already stores many unrelated fact families and indexes. It can own
a `SemanticStore`, but it should not become the file where MIR, domains,
summaries, extensions, and cache logic all live.

## Modern Rust Design Guidance

The local Rust skill and official Rust guidance push the same direction:

- prefer private/internal APIs until contracts are stable;
- use small `Copy` values for IDs;
- borrow facts through views instead of cloning;
- use typed errors in library/kernel internals and `anyhow` at app boundaries;
- prefer static dispatch/generics in hot paths;
- use dynamic dispatch at plugin/extension boundaries only;
- keep `unsafe` unnecessary;
- use clippy and property tests to protect invariants.

## Recommended Internal Module Boundary

Add a private module tree:

```text
crates/polint/src/analysis/
  mod.rs
  ids.rs
  meta.rs
  store.rs
  provider.rs
  schedule.rs
  stable_key.rs
  mir/
  places.rs
  calls.rs
  domains/
  summaries/
  cache_key.rs
  validate.rs
  extensions/
```

`core::AnalysisDb` can own `SemanticStore`, but implementation should live in
`analysis/*`.

## The First Vertical Slice

1. Add IDs, stable keys, `FactMeta`, typed semantic store skeleton, and typed
   kernel errors.
2. Add a deterministic provider registry/schedule that can run only requested
   providers.
3. Add MIR extraction for a small but useful subset of Go and TS/JS:
   assignments, returns, branches, calls, literals, identifiers, member/index
   access, unknown/unsupported operations.
4. Add `PlaceId` and `PlaceKey` from MIR operands.
5. Add `CallSiteFact` and direct target facts, with unresolved reasons.
6. Add P0 local domains: reachability, constants, nil/nullish, truthiness.
7. Add direct function summaries and summary dependency digests.
8. Add semantic artifact cache keys.
9. Add extension sinks and merge validation fixtures, but no dynamic loading.
10. Only then revisit public SDK views and refined call/data-flow.

## Key Non-Goals

- No public `CallGraph<'_>`/`DataFlow<'_>` behavior in the first slice.
- No full query engine or Salsa adoption before the provider DAG proves its
  limits.
- No external plugin loading before extension sink validation exists.
- No attempt to make default analysis universally complete.
- No `unsafe`.

## Main Decision

Build the bootstrap as a private, deterministic, typed kernel that favors
stable internal contracts over public API speed. Use direct facts and local
domains to validate the engine shape before global call graph and data-flow
work.
