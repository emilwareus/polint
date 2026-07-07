# Stack Research: Static Analysis 2.0

**Project:** polint v2.0 Static Analysis 2.0 implementation
**Question:** What stack additions/changes are needed for the new v2.0 milestone?
**Researched:** 2026-07-07
**Confidence:** HIGH for the immediate SQLite/rusqlite direction and rejection list; MEDIUM for search/vector timing because those are phase-gated behind store validation.

## Recommendation

Keep the existing Rust 2024 workspace, parser stack, analysis kernel, layer cache, provider manifests, typed fact metadata, eval harness, and public SDK/query-view boundaries. v2.0 needs **persistence and queryability**, not a replacement analyzer stack.

Add **SQLite via `rusqlite` with bundled SQLite** as the primary private local semantic store. Implement it behind a narrow crate-private facade under `crates/polint/src/analysis_kernel/store/` (or equivalent), not in `sdk`, `runner`, language adapters, or public CLI contracts.

Add **`blake3`** for content-addressed summary payloads and future registry-ready manifests. The current 64-bit deterministic FNV-style digests are useful internal cache keys, but registry-ready package summaries need stronger content identities.

Treat **Tantivy** as the first search engine, but add it only when the lexical-search phase begins. Keep **`redb`** as an optional fallback/content blob cache candidate, not a default dependency. Keep **`sqlite-vec`** experimental and off by default until embedding lockfiles, unsafe-boundary handling, and deterministic invalidation are designed.

## Additions

### Add Now

| Crate | Version Checked | Cargo Shape | Capability | Why |
|---|---:|---|---|---|
| `rusqlite` | `0.40.1` | `rusqlite = { version = "0.40.1", features = ["bundled"] }` | Embedded semantic store | Bundled SQLite avoids system SQLite drift, supports single-file local persistence, transactions, WAL, migrations via `PRAGMA user_version`, indexes, recursive CTEs, JSON-ish metadata fields where needed, and read-only query connections. |
| `blake3` | `1.8.5` | `blake3 = "1.8.5"` | Content-addressed payload IDs | Needed for summary manifests, package/version payload seams, recompute-and-diff metadata, and future registry trust hooks. Do not enable the `rayon` feature initially; polint should keep parallelism scheduling explicit. |

Recommended workspace sketch:

```toml
[workspace.dependencies]
rusqlite = { version = "0.40.1", features = ["bundled"] }
blake3 = "1.8.5"

[dependencies]
rusqlite.workspace = true
blake3.workspace = true
```

### Add Later, Phase-Gated

| Crate | Version Checked | When | Why |
|---|---:|---|---|
| `tantivy` | `0.26.1` | Lexical-search phase, after store document IDs exist | Best fit for local full-text search over symbols, summaries, evidence, comments, and doc-like text. Store stable IDs in SQLite and mirror them into Tantivy fields; do not treat Tantivy segment `DocId`s as stable semantic IDs. |
| `redb` | `4.1.0` | Only if SQLite distribution or content-blob performance fails validation | Pure-Rust embedded fallback or adjacent content-addressed blob cache. Not suitable as the default graph/query store because v2.0 needs relational filters, joins, migrations, and graph-adjacent query indexes. |
| `sqlite-vec` | `0.1.10-alpha.4` | Experimental vector-search work only, after embedding lockfiles | Good future SQLite-aligned vector side index, but it is pre-v1 and its Rust setup currently involves SQLite extension registration patterns that need explicit unsafe-boundary review under `unsafe_code = "forbid"`. |

### Reuse, Do Not Replace

| Existing Stack | Current Version | v2.0 Role |
|---|---:|---|
| `petgraph` | `0.8.3` | Scoped in-memory path traversal loaded from SQLite rows when recursive CTEs are not auditable enough. |
| `serde`, `serde_json`, `toml` | existing pins | Manifest/config/result JSON, deterministic query envelopes, migration fixtures. |
| `rayon` | `1.12.0` | Parallel provider work around deterministic ingest/query boundaries, not concurrent writes to the same SQLite transaction. |
| `insta`, `assert_cmd`, `predicates`, `tempfile`, `proptest` | existing pins | Store snapshots, CLI fixtures, crash/restart subprocess tests, migration and query invariants. |
| Oxc / tree-sitter-go / module graph / semantic graph / summaries / evidence | existing pins/modules | Continue producing facts. v2.0 persists/indexes validated facts; it does not swap parser or frontend technology. |

## Rejections

| Do Not Add | Why | Use Instead |
|---|---|---|
| Remote registry client/server stack (`reqwest`, HTTP server crates, auth/signing infra) | The registry is explicitly deferred. Adding network machinery now expands product, security, and ops scope before the local store is proven. | Local content-addressed manifests, package/version identity, trust placeholders, recompute-and-diff seams. |
| Public SQL, Cypher, Datalog, QL, SPARQL, or raw graph query language | Violates the public-boundary gate. Raw tables/provider IDs/SQL are private implementation detail. | Purpose-built internal query APIs first; later stable CLI JSON envelopes for `used-by`, `neighbors`, `callers`, `callees`, `path`, and `taint`. |
| `sqlx`, `diesel`, or ORM/query-builder stack | Compile-time SQL and ORM models add friction without value for a private embedded schema that must evolve quickly. | Hand-written SQL behind typed Rust methods on `SemanticStore`. |
| `rusqlite_migration` by default | Useful crate, but the first migration runner is small: numbered SQL, `PRAGMA user_version`, transaction tests, and controlled diagnostics. | Reconsider `rusqlite_migration 2.6.0` only if homegrown migration code grows beyond the skeleton. |
| RocksDB, Kuzu, DuckDB, sled as default store | Larger footprint, different query model, or weaker fit for indexed mutable local graph/fact queries. | SQLite/rusqlite primary; redb only as fallback. |
| Vector DB or live embedding stack | Deterministic `polint check` cannot depend on live inference or model downloads. | Tantivy lexical search first; sqlite-vec side index later with committed embedding lockfiles. |
| New parser/front-end crates for Go or TS/JS | The milestone is durable store/query foundation, not frontend replacement. | Persist outputs from existing tree-sitter-go/Oxc providers and existing semantic/refined graph layers. |
| Public raw graph SDK views | Locked research says policy-query views first; raw CG/CFG/DF internals stay private until proven. | Existing public policy-query SDK views and future deliberately scoped query objects. |

## Integration Points

### Cargo and Module Boundary

- Add `rusqlite` and `blake3` to workspace dependencies and `crates/polint/Cargo.toml`.
- Create a private store module owned by the analysis kernel:

```text
crates/polint/src/analysis_kernel/store/
  mod.rs
  connection.rs
  migrations.rs
  schema.rs
  ids.rs
  ingest.rs
  query.rs
  graph.rs
  payloads.rs
  search_manifest.rs
```

- Keep `rusqlite` types inside that module. Providers should call typed store methods, not accept `rusqlite::Connection` or SQL strings.

### Cache Layout

- Extend `CacheLayout` with a semantic-store path/category under the existing `.polint/cache` root, likely `.polint/cache/semantic/store.sqlite3`.
- Keep existing JSON layer cache in place during initial rollout. The semantic store indexes validated facts and summaries; it should not replace every layer payload in one change.
- Store path/manifest identity must include workspace/config identity so separate checkouts or incompatible configs do not share a graph accidentally.

### Store Lifecycle

- Open with `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, bounded `busy_timeout`, and explicit transaction boundaries.
- Use `PRAGMA user_version` for schema versioning.
- Commit writes at provider/layer generation boundaries. A crash must leave either the old generation or the complete new generation, never a mixed graph.
- Prefer read-only connections for query commands once the store exists.

### Persisted Families

Start with durable metadata and cheap indexes before broad graph queries:

1. `store_manifest`, migrations, provider generations, input snapshots, layer entries.
2. Files, modules/packages, dependencies, symbols, definitions, references, imports, exports, resolutions, fact metadata.
3. Nodes/edges/evidence, adjacency forward/reverse, unknown regions, budget events.
4. Summary manifests, payload digests, summary dependencies, projections.
5. Search manifests and search document manifests.

### Query Execution

- Use SQLite covering indexes for common xref/filter queries.
- Use recursive CTEs only for bounded traversals with cycle guards, explicit budgets, deterministic ordering, and status propagation.
- For path-heavy queries, load a scoped subgraph into `petgraph` or a custom deterministic adjacency structure so evidence ranking and unknown/budget handling remain auditable.
- Preserve statuses: `Found`, `NotFound`, `Unknown`, `BudgetExceeded`. Never translate unknown into no result.

### Search

- Build a `SearchCorpus` abstraction over stable semantic-store document IDs before adding Tantivy.
- Store Tantivy manifests in SQLite.
- Rebuild indexes by content digest/generation, not wall-clock state.
- Search results point back to store IDs and evidence spans; search does not create semantic facts.

## Risks

| Risk | Mitigation |
|---|---|
| SQLite schema becomes an accidental public API | Keep all store modules `pub(crate)`, add public-output leak tests for table names/provider IDs/SQL, and expose only typed SDK/query envelopes. |
| Store writes mix generations after crash | Use one transaction per generation, staged payload writes, generation tables, and recovery fixtures that kill the process mid-ingest/migration/WAL checkpoint. |
| Query performance degrades on large edge sets | Run the validation microbench sizes from `research/local-semantic-store/VALIDATION.md`; compare covering indexes, recursive CTEs, and Rust-loaded scoped traversal. |
| Digest vocabulary splits between FNV cache keys and BLAKE3 content IDs | Introduce a typed `ContentDigest`/`PayloadDigest` wrapper and document which digest is for cache invalidation versus content addressing. Do not silently change existing cache keys. |
| Tantivy internal document IDs are mistaken for stable IDs | Persist semantic document IDs in SQLite and in a stored Tantivy field; treat Tantivy `DocId` as an index-local lookup handle only. |
| Bundled SQLite increases build time or native-linking surface | Keep it as the primary because deterministic SQLite behavior matters more. Track build cost in CI and keep redb fallback documented but off by default. |
| sqlite-vec conflicts with `unsafe_code = "forbid"` or deterministic check guarantees | Keep it experimental, off by default, and outside `polint check` until extension loading and embedding lockfiles have explicit validation. |

## Open Questions

- Should summary payloads live as SQLite BLOBs, adjacent content-addressed files, or a hybrid? Validation must compare DB size, WAL growth, crash behavior, and read latency.
- Which `rusqlite` optional features are actually needed after the skeleton? Start with `bundled`; consider `limits`, `blob`, or `backup` only when implementation requires them.
- Should migration handling stay homegrown or adopt `rusqlite_migration 2.6.0` once schema count grows?
- What exact cache category names and cleanup/prune semantics should `polint cache status/clean/prune` expose for the semantic store?
- What is the first stable CLI query envelope, and when is a hidden `polint graph` experiment allowed to become public?
- What Tantivy feature set/tokenizers should code search use? Code identifiers likely need custom tokenization rather than natural-language stemming.
- What threshold justifies adding `redb` as a fallback instead of fixing SQLite packaging/performance?

## Sources

- Primary local research: `research/static-analysis-2.0/README.md`, `research/static-analysis-2.0/OPEN-QUESTIONS.md`, `research/local-semantic-store/README.md`, `research/local-semantic-store/RECOMMENDED_IMPLEMENTATION.md`, `research/local-semantic-store/VALIDATION.md`.
- Workspace inputs: `Cargo.toml`, `crates/polint/Cargo.toml`, `crates/polint/src/analysis_kernel/*`, `crates/polint/src/analysis/*/store.rs`.
- Crate metadata checked via `cargo search` / `cargo info` on 2026-07-07: `rusqlite 0.40.1`, `blake3 1.8.5`, `tantivy 0.26.1`, `redb 4.1.0`, `sqlite-vec 0.1.10-alpha.4`, `rusqlite_migration 2.6.0`.
- Official/current docs: [rusqlite docs](https://docs.rs/rusqlite/), [rusqlite features](https://docs.rs/crate/rusqlite/latest/features), [Tantivy docs](https://docs.rs/tantivy/), [redb docs](https://docs.rs/redb), [sqlite-vec repository](https://github.com/asg017/sqlite-vec), [sqlite-vec crate](https://docs.rs/sqlite-vec/0.1.10-alpha.4).
- Context7 lookups: `/rusqlite/rusqlite`, `/quickwit-oss/tantivy`, `/asg017/sqlite-vec`.
