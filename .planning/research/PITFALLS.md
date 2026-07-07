# Pitfalls Research: Static Analysis 2.0 Implementation

**Project:** polint
**Domain:** Adding a durable local semantic store, graph query foundations, summary persistence, registry-ready seams, and search boundary to the existing private analysis engine
**Researched:** 2026-07-07
**Confidence:** HIGH

This report focuses on mistakes likely when adding Static Analysis 2.0 capabilities to the existing polint system. It assumes the locked decisions from `research/static-analysis-2.0/` and `research/local-semantic-store/`: SQLite/rusqlite primary, redb fallback only, no remote registry now, no raw graph/query language, Tantivy first for lexical search, vector search deferred, and registry-ready seams only.

Recommended phase buckets below are topic names, not final roadmap numbers. If the roadmap uses different names, map each pitfall to the first phase that touches that subsystem.

## Phase Placement Summary

| Phase Bucket | Must Own |
|---|---|
| Store Foundation and Migrations | SQLite facade, schema versions, generation manifest, migrations, writer discipline, no public SQL/table contract |
| Durable Fact and Graph Persistence | provider/layer key reuse, dependency indexes, stable IDs, graph adjacency/evidence persistence, pruning |
| Summary Persistence and Registry Seams | summary keys, payload layout, content-addressed package manifests, trust/provenance fields, recompute-and-diff hooks |
| Graph Query Commands | used-by, neighbors, callers/callees, paths, taint-style reachability, agent JSON envelopes, unknown/budget semantics |
| Search Boundary | Tantivy document schema, stable store document IDs, rebuild lifecycle, candidate-only semantics |
| Validation and Recovery Gates | cold/warm parity, partial invalidation, process restart, crash/recovery, benchmarks, no-leak proof |
| Public Boundary and Docs | CLI/docs/facts/skill text alignment, public-surface leak tests, precision/unknown/budget documentation |

## Critical Pitfalls

### P1: Building a Second Cache/Identity System Beside the Existing Kernel

**Pitfall:** The SQLite store grows its own provider IDs, graph IDs, file keys, summary keys, invalidation rules, and precision/status vocabulary instead of persisting the existing provider manifests, `InputSnapshot`, layer/query/summary keys, fact metadata, validation status, and stable keys.

**Warning Signs:**
- Store code hashes source/config/toolchain inputs separately from `analysis_kernel::incremental`.
- A persisted fact can be loaded without its original provider manifest, precision, confidence, provenance, validation status, and schema version.
- Cold and warm runs disagree after a provider manifest or lifecycle setting changes.
- New "store IDs" appear in CLI JSON, docs, or SDK-facing types.

**Prevention:**
- Make the semantic store a backing implementation for existing kernel concepts, not a peer subsystem.
- Persist kernel-owned stable keys and metadata as first-class columns; treat SQLite row IDs as internal storage handles only.
- Require every persisted fact family to declare source provider, schema version, input/layer key, dependency set, precision, confidence, validation status, and generation.
- Add one small facade first and forbid direct `rusqlite::Connection` use outside the store module.

**Phase Placement:** Store Foundation and Migrations, then enforced in every persistence phase.

**Must-Test Items:**
- Cold build, warm restore, and process-restart restore produce byte-identical normalized debug/query JSON.
- Editing one provider manifest invalidates only layers owned by that provider.
- Public leak gate proves store IDs, SQL, table names, provider IDs, parser IDs, and raw graph IDs are not reachable through SDK prelude, public CLI JSON, README, generated skill text, or `docs/facts/`.

### P2: SQLite Insertion Order Leaking Into Deterministic Output

**Pitfall:** Query outputs, path rankings, graph neighbors, search document IDs, or summary manifests depend on SQLite `rowid`, insertion sequence, parallel provider completion order, or unordered Rust collection traversal.

**Warning Signs:**
- Query snapshots differ after changing Rayon worker count or provider insertion order.
- `ORDER BY rowid` or implicit SQLite ordering appears in query code.
- A no-op warm run emits the same facts in a different order.
- Path budgets select different paths across repeated runs.

**Prevention:**
- Define stable semantic ordering for every public/query result: repository-relative path, stable node key, stable edge key, span, relation kind, precision/status, then deterministic tie-breaker.
- Never expose SQLite row IDs or Tantivy internal document IDs; derive public IDs from store stable keys or content digests.
- Canonicalize paths and path sets after SQL/Rust traversal and before JSON rendering.
- Include provider-order shuffle and Rayon worker-count permutations in validation.

**Phase Placement:** Store Foundation and Migrations for identity rules; Graph Query Commands and Search Boundary for result ordering.

**Must-Test Items:**
- Same repo/config/toolchain across cold, warm, partial invalidation, process restart, randomized insertion order, and multiple Rayon worker counts yields byte-identical graph/query JSON.
- A cycle/path fixture with multiple valid shortest paths always returns the same bounded path set.
- Tantivy rebuild from the same store manifest maps results back to the same stable store document IDs.

### P3: Mixed Generations After Crash, Migration, or Search Rebuild

**Pitfall:** A run crashes during ingest, migration, summary payload write, WAL checkpoint, or Tantivy rebuild. The next run reads a partially updated store: some tables are new generation, some are old, payload files point nowhere, or the search index refers to stale document IDs.

**Warning Signs:**
- Store manifest generation advances before all fact/index/payload writes complete.
- Search index rebuild mutates the live index in place.
- Summary payload files are written without digest verification and manifest commit.
- Recovery code silently rebuilds or drops data without a diagnostic.

**Prevention:**
- Use generation-scoped writes: write all DB rows, payload files, and search indexes into a pending generation; atomically mark the generation active only after integrity checks pass.
- Keep old generation readable until the new generation is complete.
- Treat search indexes as derived artifacts tied to a store manifest digest. Rebuild, then swap.
- On recovery, choose only two states: old generation readable or new generation complete. Anything else yields a clear rebuild-needed diagnostic.

**Phase Placement:** Store Foundation and Migrations, Search Boundary, Validation and Recovery Gates.

**Must-Test Items:**
- Kill process during SQLite ingest transaction, summary payload write, migration, WAL checkpoint, and Tantivy rebuild.
- After restart, queries either use the previous complete generation or report a rebuild-needed diagnostic; never mixed facts.
- Corrupt or missing payload file is detected by digest and does not produce silently incomplete query results.

### P4: Under-Invalidating Cross-Family Graph and Summary Dependencies

**Pitfall:** Durable persistence makes stale facts harder to notice. A source edit, lifecycle config change, rule-pack capability change, extension/model digest change, Go module root change, TS project topology change, solver budget change, or summary dependency change should invalidate affected graph/query/search/summary rows but does not.

**Warning Signs:**
- Warm query results differ from `--no-cache` after edits.
- Rule-pack edits invalidate all analysis layers, or worse, invalidate none when requested capabilities changed.
- Search results point to facts deleted by graph pruning.
- Summary-derived edges survive after callee summary digest changes.

**Prevention:**
- Persist dependency indexes at the same granularity as the query answer: file, package/project, provider, capability, lifecycle input, summary key, extension/model digest, budget profile, and schema version.
- Keep rule execution digests separate from analysis layer digests, but include requested capabilities and analysis-affecting rule options when they change provider output.
- Make graph/search rows invalidation-aware: queryable rows must be scoped to an active generation and dependency state.
- Add mutation fixtures for each upstream input, including "must invalidate" and "must preserve hit" cases.

**Phase Placement:** Durable Fact and Graph Persistence; Summary Persistence and Registry Seams; Search Boundary.

**Must-Test Items:**
- Single-file body edit invalidates affected summaries, graph edges, path answers, and search docs while preserving unrelated packages.
- Go lifecycle changes (`module_roots`, `package_patterns`, `build_tags`, `include_tests`) participate in affected digests.
- Query parameter, budget, and preview API version changes invalidate cached query answers where relevant.
- Cold/warm comparison after every mutation class produces identical answers.

### P5: Hiding Unknown, Unsupported, or Budget-Exceeded as "Not Found"

**Pitfall:** Graph and taint-style queries are implemented as Boolean searches. If traversal hits an unresolved dynamic call, missing setup, unsupported construct, summarized boundary, search index miss, or budget cap, the command returns `not_found` instead of `unknown` or `budget_exceeded`.

**Warning Signs:**
- Query result enum has only found/not-found variants.
- Recursive CTE depth limits are applied with `LIMIT` but no budget/status row is surfaced.
- Search candidates are used to skip graph verification.
- Docs say "no path exists" for dynamic languages or setup-sensitive queries.

**Prevention:**
- Reuse existing `Found`, `NotFound`, `Unknown`, and `BudgetExceeded` semantics from policy queries.
- Persist unknown/budget facts as queryable rows, not only telemetry.
- Every public graph/query JSON envelope must include precision, confidence, status, budget state, and unknown reasons.
- Search can only return candidates linked to evidence; graph/provider facts remain the authority.

**Phase Placement:** Graph Query Commands, with docs/no-leak proof in Public Boundary and Docs.

**Must-Test Items:**
- Fixtures for unresolved dynamic call, setup-missing package/project, unsupported construct, summarized boundary, cycle budget, and path-count budget.
- `not_found` appears only when the query has complete enough evidence to say so under configured precision/confidence thresholds.
- Docs and JSON schema distinguish `unknown` from `not_found`.

### P6: Recursive Graph Queries That Duplicate, Miss, or Explode Around Cycles

**Pitfall:** SQLite recursive CTEs or Rust-loaded traversals return duplicate paths, miss valid paths through cycles, ignore barriers/sanitizers, or grow without deterministic caps.

**Warning Signs:**
- Path query correctness is tested only on DAG fixtures.
- Queries use `UNION ALL` recursion without visited/cycle controls.
- A cycle fixture returns many equivalent paths or times out.
- Taint reachability has different barrier behavior in SQL and Rust traversal paths.

**Prevention:**
- Start with typed adjacency/evidence tables and bounded traversal. Use SQL for simple neighbors/callers/callees and Rust scoped traversal for path-heavy queries when status/budget handling is clearer.
- Canonicalize visited state by stable node/edge keys and query options.
- Define max depth, max paths, and path ranking before public CLI exposure.
- Treat sanitizer/barrier behavior as part of the query model, not a post-filter over rendered paths.

**Phase Placement:** Graph Query Commands, with performance proof in Validation and Recovery Gates.

**Must-Test Items:**
- Caller/callee cycle, import cycle, path cycle, taint path with sanitizer/barrier, duplicate-edge, and budget-exceeded fixtures.
- Property test that path results contain no duplicate stable path IDs.
- Benchmark p50/p95 for used-by, neighbors, callers/callees, bounded paths, and taint-style reachability on large synthetic and real datasets.

### P7: Summary Persistence Overpromising Precision or Trust

**Pitfall:** Persisted summaries, especially package/export summaries and future registry-ready payloads, are treated as exact reusable truth. Heuristic, AI-derived, setup-aware, stale, or budgeted summaries flow into graph/data-flow results without visible precision/trust metadata.

**Warning Signs:**
- Summary rows lack precision/confidence/status/provenance/trust fields.
- Package summaries do not identify package/version, schema version, frontend/toolchain digest, config digest, and callee summary digests.
- Whole-program refinement overlays are persisted without representing their full closure in the key.
- Docs imply summary-backed answers are exact.

**Prevention:**
- Persist only canonical local/SCC summaries by default. Keep whole-program refinement overlays run-local unless their full closure, config, and inputs are key-represented and validation-backed.
- Summary-derived facts must carry the existing precision/confidence tiers: conservative/setup-aware/heuristic/unknown.
- Registry-ready seams are manifest and payload shapes only: content address, package/version identity, schema version, provenance, validation metadata, trust hooks, recompute-and-diff. No publish/fetch protocol.
- Evidence crossing summaries must cite compact summary segments and expose expansion handles without expanding by default.

**Phase Placement:** Summary Persistence and Registry Seams; Graph Query Commands for evidence rendering.

**Must-Test Items:**
- Summary boundary path evidence includes package/app digest, callable, transfer, summary digest, precision tier, provenance, and status.
- Recompute-and-diff detects a stale package summary.
- AI/heuristic summary rows, if present later, remain heuristic and never satisfy high-confidence query thresholds by default.
- No remote registry command, protocol, or public corpus assumption appears in v2.0 CLI/docs.

### P8: Payload Layout Bloat and Orphaned Content-Addressed Files

**Pitfall:** Large summaries, evidence bundles, snippets, or serialized graphs are stored directly in SQLite BLOBs before size data exists, or adjacent content-addressed files are introduced without integrity, pruning, and manifest rules. The store bloats, vacuum/pruning is unsafe, and CI cache restore becomes fragile.

**Warning Signs:**
- Full source text, MIR, CFG, or raw AST payloads are persisted by default.
- Payload files are named by transient IDs rather than content digest.
- Deleting a package leaves payload files behind.
- DB/WAL size grows after warm updates and never shrinks.

**Prevention:**
- Keep line-offset tables, spans, file digests, fact rows, summary metadata, and compact evidence in the DB; lazy re-read source only for snippets and omit snippets when digest mismatches.
- Benchmark SQLite BLOBs versus adjacent content-addressed payload files before locking layout.
- If adjacent files are used, make manifest rows authoritative, verify digest on read, and implement generation-scoped garbage collection.
- Drop or avoid persisting MIR/CFG after summaries unless demanded for evidence/debug.

**Phase Placement:** Durable Fact and Graph Persistence; Summary Persistence and Registry Seams; Validation and Recovery Gates.

**Must-Test Items:**
- Payload-size benchmark for 100k/500k/1M edge-scale datasets, then larger if hardware permits.
- Package deletion/pruning removes unreachable payloads and keeps old active generation readable until commit.
- Source digest mismatch omits snippet rather than reading stale source.

### P9: One-Writer SQLite Discipline Lost Under Parallel Analysis

**Pitfall:** Existing deterministic parallel analysis writes directly to SQLite from multiple worker threads or multiple `polint` processes. This produces `database is locked` failures, non-deterministic busy retries, long WAL growth, or partial ingest ordering leaks.

**Warning Signs:**
- `rusqlite::Connection` is cloned or shared across Rayon workers without a writer boundary.
- Worker code opens its own write connection to the active store.
- Busy timeout/retry policy is ad hoc per callsite.
- WAL size spikes during ingestion.

**Prevention:**
- Use deterministic parallel providers to produce sorted batches, then commit through one writer boundary per generation.
- Keep read connections separate from write generation commits; do not let public queries observe pending generations.
- Centralize PRAGMA, WAL, busy timeout, checkpoint, and transaction policy in the store facade.
- Consider process-level lock or generation lease so two `polint` invocations cannot both write the same store generation.

**Phase Placement:** Store Foundation and Migrations; Durable Fact and Graph Persistence.

**Must-Test Items:**
- Parallel ingest with different Rayon worker counts produces identical DB manifest and query output.
- Two concurrent `polint` processes against the same repo/store either serialize safely or one uses a clear read-only/rebuild-needed path.
- WAL size and checkpoint behavior measured during large ingest benchmarks.

### P10: Tantivy Search Becoming a Fact Source or Stable Ranking Contract

**Pitfall:** Lexical search results are treated as semantic truth, or ranking/document IDs become a public compatibility promise. Query commands start relying on search hits as if they were validated graph facts.

**Warning Signs:**
- Search result JSON omits the backing store fact/document stable ID.
- Search command returns "found vulnerability/path" without graph verification.
- Docs promise stable ranking across Tantivy versions or index rebuilds.
- Absolute paths or raw snippets leak through search results.

**Prevention:**
- Search returns candidates only, linked to stable semantic-store document IDs and evidence pointers.
- The canonical truth remains provider facts plus validation metadata.
- Define a narrow Tantivy document schema after graph/evidence envelopes exist; do not index raw internals.
- Ranking may be deterministic for the same index build but should not be documented as semantic proof.
- Search index is a derived artifact tied to the store manifest digest.

**Phase Placement:** Search Boundary, after initial graph/evidence envelope shape exists.

**Must-Test Items:**
- Deterministic Tantivy rebuild from the same store manifest.
- Search result points back to a valid active store fact/document and never to stale/deleted rows.
- No absolute path leakage; paths remain repo-relative.
- Search hit must be verified by graph/provider facts before any rule/diagnostic claims a policy violation.

### P11: Vector Search Creeping Into Deterministic `check`

**Pitfall:** Vector search, live embeddings, or model downloads slip into the first implementation because the search boundary already exists. This breaks byte-stable `check`, adds model/runtime drift, and muddies provenance.

**Warning Signs:**
- `polint check` invokes embedding generation or downloads a model.
- Vector index fields appear in the primary semantic-store schema instead of an experimental side index.
- Embedding results lack model digest, chunker version, dimensions, metric, normalization, and source digest.
- Docs present vector results as precise analysis facts.

**Prevention:**
- Keep vector search deferred and experimental behind an explicit store/search boundary.
- No live embedding inference in deterministic commands.
- Require lockfiles for model/chunker/provenance before any vector experiment.
- Mark vector similarity as heuristic candidates unless graph/provider verification confirms a fact.

**Phase Placement:** Search Boundary as a negative gate; vector implementation stays out of v2.0 unless a later experimental phase is explicitly added.

**Must-Test Items:**
- `polint check` does not read model files, embedding lockfiles, vector indexes, or network resources.
- If an experimental vector command exists later, missing/stale lockfile fails closed and marks results heuristic.
- Public docs say lexical first, vector deferred/experimental.

### P12: Public Surface Leak Through Graph Query Convenience

**Pitfall:** Because agents need graph answers, the implementation exposes SQL, raw graph traversal, provider IDs, parser IDs, internal node/edge IDs, or a query language through SDK, CLI JSON, examples, docs, generated skill text, or `polint graph` shortcuts before the contract is stable.

**Warning Signs:**
- `polint::sdk::prelude::*` grows during a store/graph persistence phase.
- Public JSON includes table names, SQL snippets, provider names as identifiers, raw semantic graph IDs, or internal solver fields.
- Docs or templates teach users to query `CallGraph<'_>`/`Cfg<'_>` or raw graph APIs rather than policy views and purpose-built graph commands.
- The public leak gate allowlist changes without an API visibility promotion record.

**Prevention:**
- Treat every CLI command, JSON field, docs page, README example, generated skill instruction, and SDK export as public API.
- Build graph commands as purpose-built envelopes: used-by, neighbors, callers/callees, bounded paths, taint-style reachability. No public raw SQL, no graph query language, no table names.
- Expand `public_surface_leak.rs` to forbid semantic store, SQL, raw graph, solver, provider, parser, and internal ID namespaces.
- Keep raw store inspection behind internal tests/debug fixtures only, not public CLI.

**Phase Placement:** Public Boundary and Docs; leak checks should be introduced in Store Foundation and run throughout.

**Must-Test Items:**
- External probe crate imports only `polint::sdk::prelude::*` and cannot name semantic-store, analysis-kernel, raw graph, solver, provider, parser, SQL, or internal ID types.
- CLI JSON schema snapshot contains only reviewed public envelope fields.
- README, docs/facts, generated skill text, and examples contain no raw SQL/table/query-language instructions.

### P13: redb Fallback Diverging From SQLite Semantics

**Pitfall:** redb starts as a fallback but becomes a second semantic store with weaker graph/filter/query semantics, separate invalidation, and separate migration behavior. Bugs reproduce only on one backend.

**Warning Signs:**
- Feature branches implement graph query behavior twice.
- redb fallback returns fewer statuses, missing provenance, or no recursive/path behavior.
- Tests run only against SQLite.
- Users can select redb for capabilities that require relational graph queries.

**Prevention:**
- Keep redb fallback scoped to pure-Rust fallback and possible content-addressed blob/cache role unless a later phase proves full parity.
- Define a backend facade where unsupported backend capabilities fail closed with capability diagnostics, not degraded silent answers.
- Do not let fallback requirements force the SQLite schema into KV-shaped compromises.

**Phase Placement:** Store Foundation and Migrations; Summary Persistence and Registry Seams if redb is used for payload/cache.

**Must-Test Items:**
- Backend capability matrix test: SQLite supports v2.0 graph/query requirements; redb either returns identical envelopes for supported subset or fails closed with explicit unsupported status.
- No public docs imply redb is equivalent for graph/filter queries unless parity tests exist.

### P14: Query Commands Coupled Too Tightly To Rust Rule Execution

**Pitfall:** `polint graph` or future query commands reuse rule execution paths in a way that requires repo-local Rust rule compilation, rule options, or diagnostics machinery for simple exploration. Conversely, rule execution starts depending on graph command JSON instead of typed internal query services.

**Warning Signs:**
- Graph command code shells through `polint check` or a generated rule to answer used-by/callers.
- Rule APIs parse CLI graph JSON to get facts.
- Query cache keys include unrelated rule-pack digests for read-only exploration.
- Changing CLI envelope fields breaks internal rule behavior.

**Prevention:**
- Put shared query logic behind crate-private typed services over the semantic store.
- Public graph commands and SDK policy views both project from those services into their own public envelopes.
- Rule-pack digests affect rule execution and analysis-affecting requested capabilities, not read-only graph exploration unless the query explicitly depends on rule-provided extension/model facts.

**Phase Placement:** Graph Query Commands, with cache identity checks in Durable Fact and Graph Persistence.

**Must-Test Items:**
- A used-by/callers query works without any repo-local rule crate.
- Rule-policy query tests still use typed SDK views, not CLI graph JSON.
- Editing a rule-only file does not invalidate store graph layers unless requested capabilities or extension/model facts changed.

## Moderate Pitfalls

### P15: Migration Tests Cover Happy Path Only

**Pitfall:** Migrations are added, but tests only create a fresh current schema. Real users will carry stores across schema versions, interrupted migrations, and downgraded binaries.

**Warning Signs:**
- Test setup always starts from an empty DB.
- Schema version exists but no fixture DBs are checked in.
- Downgrade behavior is undefined.

**Prevention:** Keep small fixture databases for each committed schema version; migration code must be idempotent and generation-aware. Unsupported future schema should fail with a clear rebuild or upgrade diagnostic, not attempt best-effort reads.

**Phase Placement:** Store Foundation and Migrations; Validation and Recovery Gates.

**Must-Test Items:** v0/empty-to-current, previous-to-current, interrupted migration recovery, future-schema diagnostic.

### P16: Store Paths and CI Cache Restore Leak Machine-Specific State

**Pitfall:** Absolute paths, temp directories, user home paths, local Go/TS cache paths, or machine-specific SQLite settings enter store manifests, search docs, or public query output. CI restore becomes non-portable and public JSON leaks local paths.

**Warning Signs:** Query/search JSON contains `/Users/...`, `/tmp/...`, package manager cache paths, or absolute `go env` paths.

**Prevention:** Store canonical repo-relative paths for facts and public envelopes. Keep machine/toolchain paths in private input snapshots only when needed for invalidation, and hash them where display is not required.

**Phase Placement:** Store Foundation and Migrations; Search Boundary; Public Boundary and Docs.

**Must-Test Items:** Move repo to a different temp root, restore store/CI cache, and compare normalized query JSON. Search output must contain no absolute paths.

### P17: Validation Gates Land After Public Commands

**Pitfall:** `polint graph` or search commands ship before cold/warm parity, recovery, query correctness, budgets, and benchmarks are enforced. The first public shape becomes hard to change.

**Warning Signs:** CLI help advertises graph/search commands while fixtures cover only unit-level store reads.

**Prevention:** Hide or keep commands internal until JSON schema, determinism tests, unknown/budget behavior, Go+TS fixtures, large-repo latency, and docs are complete.

**Phase Placement:** Validation and Recovery Gates before Public Boundary and Docs marks anything stable.

**Must-Test Items:** Promotion checklist per command: schema snapshot, determinism matrix, unknown/budget fixture, large-repo benchmark, no-leak proof, docs with limits.

## Cross-Phase Must-Test Matrix

| Test Area | Required Coverage |
|---|---|
| Determinism | cold, warm, partial invalidation, process restart, randomized provider/file insertion order, different Rayon worker counts |
| Crash/recovery | kill during ingest transaction, summary payload write, migration, WAL checkpoint, search rebuild |
| Query correctness | used-by, cross-file reference, cross-package import, direct call, refined call, unresolved dynamic call, caller/callee cycle, path cycle, taint path with sanitizer/barrier, budget-exceeded path, summary boundary segment, extension-provided edge |
| Store performance | ingest time, warm update time, DB/WAL size, peak RSS, p50/p95 query latency, pruning/vacuum behavior at 100k/500k/1M+ scale |
| Public boundary | SDK prelude allowlist, external probe crate, CLI JSON snapshots, README/docs/facts/skill text scan for raw internals |
| Search | stable document IDs, deterministic rebuild, active-store back-reference, schema migration, no absolute path leakage, candidate-only wording |
| Registry seams | content-addressed payload integrity, package/version identity, schema version, provenance/status/trust fields, recompute-and-diff, no remote operations |

## Sources

- `.planning/PROJECT.md`
- `AGENTS.md`
- `research/static-analysis-2.0/OPEN-QUESTIONS.md`
- `research/local-semantic-store/RESEARCH-ANALYSIS.md`
- `research/local-semantic-store/VALIDATION.md`
- `research/local-semantic-store/decisions/DECISIONS.md`
- `docs/facts/README.md`
- `docs/API-VISIBILITY-PLAN.md`
- `crates/polint/tests/public_surface_leak.rs`
