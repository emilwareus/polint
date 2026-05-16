# Repository Index

All implementation repositories were cloned under
`research/incremental-query-engine/repos/`, which is ignored by the repository
root `.gitignore` via `research/*/repos/`.

## Repositories Inspected

| Repository | Commit inspected | Key paths | Why it matters |
|---|---:|---|---|
| Salsa, <https://github.com/salsa-rs/salsa> | `7e77c49f2721` | `src/revision.rs`, `src/durability.rs`, `src/function/fetch.rs`, `src/function/maybe_changed_after.rs`, `src/function/memo.rs`, `src/zalsa_local.rs` | Rust-native demand query engine with revisions, durability, red-green verification, dependency recording, and backdating. |
| rust-analyzer, <https://github.com/rust-lang/rust-analyzer> | `1a68212c5683` | `docs/book/src/contributing/architecture.md`, `crates/base-db/src/change.rs`, `crates/base-db/src/lib.rs`, `crates/vfs/src/lib.rs` | Production analyzer using Salsa, stable file IDs, VFS changes, item-tree shape separation, and cancellation. |
| Go tools/gopls, <https://github.com/golang/tools> | `a3954b5c7496` | `gopls/internal/cache/cache.go`, `snapshot.go`, `view.go`, `parse_cache.go`, `analysis.go` | Snapshot-based incremental workspace, parse cache, package invalidation, and analysis cache keys. |
| TypeScript, <https://github.com/microsoft/TypeScript> | `f350b5233149` | `src/compiler/builderPublic.ts`, `builderState.ts`, `tsbuildPublic.ts`, `commandLineParser.ts` | Builder programs, file versions, shape signatures, affected-file propagation, and `.tsbuildinfo`. |
| Pyright, <https://github.com/microsoft/pyright> | `b13157b0fac4` | `packages/pyright-internal/src/analyzer/program.ts`, `sourceFileInfo.ts`, `importResolver.ts`, `service.ts` | Import graph invalidation, recursive dependent dirtying, resolver-cache resets, and library change batching. |
| Pyrefly, <https://github.com/facebook/pyrefly> | `b22d42473e93` | `ARCHITECTURE.md`, `pyrefly/pyrefly/state/dirty.rs`, `require.rs`, `state.rs` | Modern Rust module-level incrementality with epochs, transactions, require levels, and consistency checks. |
| Pyre/Pysa, <https://github.com/facebook/pyre-check> | `34af3721bc04` | `source/interprocedural_analyses/taint/cache.ml`, `configuration.ml` | Saved-state cache for interprocedural analyses, shared memory, and config/source-change invalidation. |
| Bazel, <https://github.com/bazelbuild/bazel> | `b04821c979e5` | `SkyKey.java`, `InMemoryGraph.java`, `NodeEntry.java`, `DirtyBuildingState.java`, `InMemoryMemoizingEvaluator.java` | Skyframe's dirty checking, reverse-dependency invalidation, equality pruning, and graph edge policy. |
| Buck2, <https://github.com/facebook/buck2> | `05b7c66e98e5` | `dice/dice/src/lib.rs`, `api/key.rs`, `api/computations.rs`, `api/projection.rs`, `api/transaction.rs`, `api/invalidation_tracking.rs` | DICE: Rust incremental computation engine with projections, injected values, transactions, equality, and invalidation paths. |
| Souffle, <https://github.com/souffle-lang/souffle> | `c3861e0d3b82` | `src/ram/Query.h`, `src/ram/Relation.h`, `src/ram/analysis/Index.cpp`, `src/synthesiser/Synthesiser.cpp`, `src/RelationTag.h` | Semi-naive Datalog implementation, delta relations, indexes, provenance, and relation backends. |
| Ruff/Ty, <https://github.com/astral-sh/ruff> | `409c13f3ec50` | `crates/ty_project/src/db.rs`, `files.rs`, `lib.rs`, `walk.rs`, `metadata/settings.rs` | Current Rust Python analyzer path using Salsa; exposes risks around untracked project state and persistent cache keys. |

## Implementation Lessons By Tool

### Salsa

Salsa's core lesson is not "use the crate." It is the query protocol:

```text
input changes bump revision
query reads record dependencies
cached memo stores changed_at and verified_at
hot path shallow-verifies
cold path deep-verifies previous dependencies
equal recompute backdates changed_at
durability skips checks when only lower-impact inputs changed
```

For polint, this maps well to expensive demand queries, but not to every layer.
Parser facts, module topology, and extension validation need explicit cache
manifests and file/lifecycle digests whether or not a daemon is running.

### rust-analyzer

rust-analyzer demonstrates a production split between stable file IDs, VFS
changes, build-system inputs, and Salsa queries. The `ItemTree` design is
especially relevant: body edits should not invalidate global item shape if
imports and item signatures are unchanged.

Polint should copy the shape-digest principle for:

- imports and export declarations;
- public symbol signatures;
- framework entrypoint declarations;
- summary effect signatures;
- diagnostic-relevant spans.

### gopls

gopls shows that language lifecycle is part of the cache key. Workspace roots,
Go versions, `go.mod`, `go.sum`, vendor mode, build tags, metadata, and package
handles affect parsing, package loading, and analysis.

Polint should not have one generic file cache. It needs lifecycle-aware layer
keys for Go, TypeScript/JavaScript, Python, Java/JVM, and future languages.

### TypeScript

TypeScript's builder separates file text version from shape signature. A body
change can avoid invalidating dependents if the declaration shape is unchanged.
Its affected-file propagation through reverse references is a direct model for
polint module/symbol invalidation.

Polint should use this for TypeScript/JavaScript and also generalize it:

```text
file text digest
  -> syntax shape digest
  -> public API/export digest
  -> summary/effect digest
  -> diagnostic digest
```

### Pyright

Pyright is conservative and practical. It maintains import/imported-by edges,
marks changed files dirty, recursively marks dependents dirty, and resets import
resolver caches for structural filesystem changes.

Polint should follow the conservative default: if dependency discovery is
uncertain, recompute or quarantine rather than reuse.

### Pyrefly

Pyrefly's module-level incrementality is an important counterweight to Salsa.
Not every analyzer needs ultra-fine query graphs at the beginning. Module-level
dirty epochs, retained module states, transaction consistency, and require
levels can be enough for a high-performance type checker.

Polint should start with layer/module-level cache boundaries, then add fine
queries where global analyses need them.

### Pyre/Pysa

Pysa's saved-state logic highlights two hard truths:

- interprocedural analysis caches are large and configuration-sensitive;
- source changes and model/config changes must invalidate different slices.

Polint extension/model digests must be first-class invalidation inputs, not
metadata afterthoughts.

### Bazel Skyframe

Skyframe is the clearest mature design for graph invalidation:

```text
nodes store reverse deps
dirty nodes retain previous values and deps
children are checked before parent rebuild
equal new value can keep downstream version stable
graph edges are optional only when incrementality is intentionally disabled
```

Polint should keep dependency edges for any cached result whose reuse matters.
An edgeless cache is a key-value store, not an incremental analyzer.

### Buck2 DICE

DICE is relevant because it is Rust, modern, and key/value based. Projection
keys are especially important: a dependent can read one projected property of a
large value and avoid invalidation when unrelated properties change.

Polint should use projection-style keys for large layers:

- one file's symbols, not the entire symbol layer;
- one function's summary, not all summaries;
- one package's resolved imports, not the full module graph;
- one rule's diagnostics, not all diagnostics.

### Souffle

Souffle is the reference for semi-naive relation evaluation, delta relations,
indexes, and provenance. It should influence future relation/fixpoint internals
for reachability, recursive call/data-flow relations, and summary propagation.

It should not become the top-level cache architecture. File edits, toolchain
digests, extension validation, diagnostic fingerprints, and public SDK query
ergonomics are outside Souffle's core model.

### Ruff/Ty

Ty's Salsa usage is a useful modern Rust/Python signal. It also shows the danger
of untracked state: a stable project handle or untracked project access can make
queries stale if it changes outside the query graph.

Polint must treat untracked state as a correctness bug. Extension providers
should either declare inputs or force broad invalidation/quarantine.
