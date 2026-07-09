# Project Research Summary

**Project:** polint v2.0 - Static Analysis 2.0 Implementation
**Domain:** Durable local semantic store, summary persistence, graph query foundation, and search boundary for a repo-local static-analysis framework
**Researched:** 2026-07-07
**Confidence:** HIGH for store/architecture/product boundaries; MEDIUM for exact search, payload, and large-scale performance choices until validation benchmarks land

## Executive Summary

v2.0 should turn polint's existing private analysis engine into a durable, queryable local semantic layer without changing the core product model. `polint check` remains the repo-local Rust policy engine, and `polint review` remains the diff-focused agentic review workflow. The new foundation is persistence, invalidation, and bounded queryability over already validated facts, summaries, graph edges, evidence, provenance, precision, unknowns, and budgets.

The recommended implementation is a private SQLite/rusqlite semantic store under `analysis_kernel`, with bundled SQLite, generation tracking, migrations, crash-safe commits, and typed internal query services. `AnalysisDb` and the provider DAG remain the computation source of truth; the store indexes validated outputs and later enables summary reuse, graph exploration, and lexical search. Add `blake3` for content-addressed payload IDs if new hashing is needed. Add Tantivy only in the lexical-search phase, keep sqlite-vec experimental for later, and do not build a remote registry now.

The main risks are architectural drift: creating a second cache/identity system, letting SQLite insertion order affect public results, reading mixed generations after crash, under-invalidating summaries or graph rows, collapsing `unknown` into `not_found`, and leaking store internals into SDK/CLI/docs. Mitigate by reusing `InputSnapshot`, `LayerKey`, `SummaryKey`, `QueryKey`, provider manifests, and `FactMeta`; committing complete generations through one writer boundary; enforcing stable ordering; adding cold/warm/crash/migration/query parity gates; and proving that SQL, table names, row IDs, provider IDs, parser IDs, and raw graph internals stay private.

## Key Findings

### Stack Additions

Keep the existing Rust 2024 workspace, Oxc and tree-sitter-go frontends, private analysis kernel, provider manifests, layer cache, typed fact metadata, evaluation harness, and public SDK/query-view boundaries. v2.0 adds persistence and queryability; it does not replace the analyzer stack.

**Add now:**
- `rusqlite = { version = "0.40.1", features = ["bundled"] }`: primary embedded SQLite store. Bundled SQLite avoids host drift and supports transactions, WAL, migrations, indexes, and read-only query connections.
- `blake3 = "1.8.5"`: content-addressed summary/payload IDs and future registry-ready manifest identity. Keep polint's scheduling explicit; do not enable `blake3`'s Rayon feature initially.

**Add later, phase-gated:**
- `tantivy 0.26.1`: lexical search over stable semantic-store document IDs after graph/query IDs exist. Tantivy `DocId`s are never stable semantic IDs.
- `sqlite-vec 0.1.10-alpha.4`: experimental vector side index only after model, chunker, digest, dimension, metric, normalization, and provenance lockfiles are designed.
- `redb 4.1.0`: fallback or adjacent content-addressed blob cache only if SQLite distribution or payload performance fails validation. Do not make it a second graph store.

**Do not add first:** remote registry client/server stack, public SQL/Cypher/Datalog/QL/SPARQL, ORM stack, custom JS/TS parser, vector DB, live embedding stack, or public raw graph SDK views.

### Feature Table Stakes

**Must have:**
- Private SQLite/rusqlite store facade with migrations, schema versioning, generation tracking, recovery, and no public table/SQL contract.
- Durable persistence for normalized validated facts: files, modules/packages, imports/exports, resolutions, symbols, definitions, references, functions, calls, flow, evidence, summaries, precision, status, provenance, provider, validation, and dependency metadata.
- Summary manifests and content-addressed payload seams for dependency package summaries and application function/SCC summaries.
- Invalidation frontier so warm `review` recomputes changed functions/SCCs plus transitive summary dependents, not the full repo.
- `polint check` preservation: output, exit semantics, determinism, and public rule behavior do not drift because a local store exists.
- `polint review` preservation: diff-focused evidence, summaries, unknowns, setup gaps, and budget exhaustion remain honest.
- Exploratory `polint graph` foundation for used-by, neighbors, callers, callees, paths, taint-style reachability, and search. It is not a CI gate.
- Stable agent-friendly JSON envelope with status, precision, nodes, edges, paths, findings, unknowns, and summary counts.
- Structured filters for path glob, tests on/off, minimum precision, provenance, unknown handling, max depth, max paths, and limit.
- Validation gates for cold/warm parity, restored-store parity, migration, crash/recovery, stale reuse prevention, query correctness, unknown/budget preservation, benchmarks, and public-boundary proof.

### Differentiators

- Local semantic memory for agents: answer "what uses this?", "what path connects these?", and "what changes if this moves?" with stable IDs and evidence.
- Honest graph exploration: precision, provenance, unknowns, setup gaps, and budgets are first-class in every query result.
- O(change) review foundation: persisted summaries and dependency indexes make PR review proportional to changed code and dependent summaries.
- Registry-ready without registry: content-addressed manifests, package/version identity, schema versions, validation metadata, recompute-and-diff hooks, and trust placeholders exist locally before any networked registry.
- Exploratory-to-policy workflow: users investigate with `polint graph`, then encode recurring enforcement as Rust rules consumed by `polint check` and `polint review`.
- Semantic lexical search: Tantivy search points back to stable semantic IDs, evidence spans, symbols, summaries, and selected snippets.

### Architecture Direction

Add a private `SemanticStore` under `crates/polint/src/analysis_kernel/store/`. Keep all store types `pub(crate)` and keep `rusqlite` types inside the module. Providers should not receive SQL connections. Build a `StoreCommitPlan` from validated kernel outputs, commit complete generations after existing fact validation, and expose only complete generations to readers.

**Major components:**
1. Store facade and connection layer: open modes, WAL, foreign keys, busy timeout, read-only query connections, diagnostics.
2. Migrations and generation tracking: `PRAGMA user_version`, manifest, input snapshots, provider generations, active/complete generation selection.
3. Ingest: typed extraction from `AnalysisDb`, `FactMetaStore`, provider output metadata, layer entries, and dependency indexes.
4. Summaries and payloads: summary manifests, payload digests, dependency summary digests, projections, and payload indirection.
5. Graph and query substrate: typed node/edge adjacency, reverse adjacency, evidence, unknown regions, budget events, and internal used-by/neighbors/callers/callees/path/taint services.
6. Search manifest: stable semantic document IDs and derived Tantivy/vector metadata. Search indexes are candidates over store IDs, not facts.
7. Store validation: schema, referential integrity, row metadata, generation completeness, no-leak checks, parity helpers, and recovery fixtures.

Recommended build order: store facade and migrations; run manifest/generation tracking; validated semantic index ingest; summary persistence; graph adjacency/evidence indexes; internal query engine; public graph CLI promotion after gates; lexical search boundary; pruning, compaction, and crash hardening.

## Watch-Outs and Pitfalls

1. **Second cache/identity system:** Persist existing kernel concepts instead of inventing store-specific identities. Reuse `InputSnapshot`, provider manifests, fact metadata, layer keys, summary keys, and query keys.
2. **SQLite order leaking into deterministic output:** Never rely on `rowid`, insertion order, unordered Rust maps, or Tantivy internal IDs. Sort by stable semantic keys before public/query JSON.
3. **Mixed generations after crash or migration:** Write pending generations transactionally, keep old complete generations readable, verify payload digests, and activate only after integrity checks pass.
4. **Under-invalidation:** Persist dependency indexes for files, packages, providers, capabilities, lifecycle inputs, config, schema, summary keys, extension/model digests, budget profile, and query options where relevant.
5. **Unknown hidden as not found:** Persist unknown regions and budget events. Graph/query envelopes must distinguish `complete`, `partial`, `not_found`, `unknown`, `budget_exceeded`, `unsupported`, and `setup_missing`.
6. **Recursive query explosion:** Use SQL for simple indexed neighbors and Rust scoped traversal for path-heavy queries when cycle, barrier, budget, and evidence semantics are clearer.
7. **Summary overtrust:** Summary-derived facts must carry precision, confidence, status, provenance, digest identity, and validation metadata. Heuristic summaries stay labeled heuristic.
8. **Payload bloat:** Do not persist full AST/source/MIR/CFG dumps by default. Benchmark SQLite BLOBs versus adjacent content-addressed files before locking payload layout.
9. **Writer contention:** Parallel providers produce sorted batches; one writer commits generations. Separate read-only query connections from writes.
10. **Public surface leaks:** Add leak tests across SDK prelude, CLI JSON, README, docs/facts, examples, and generated skill text before promoting graph/search commands.

## Requirement Implications

Requirements should be written around user-visible guarantees and validation, not raw storage implementation details.

- The store is private: no SDK or public CLI contract exposes SQL, tables, raw row IDs, provider internals, parser IDs, solver IDs, or raw graph structures.
- `polint check` behavior is preserved: store corruption or invalid schema causes rebuild, skipped persistence, or controlled internal diagnostics, not changed policy answers.
- `polint review` should be the first workflow to benefit from warm summary reuse and invalidation frontier behavior.
- Store rows must carry enough metadata to prove precision, status, provenance, validation state, schema/config/provider identity, and active generation.
- Query commands must be bounded and honest. `not_found` is allowed only when the query has complete enough evidence; otherwise return `unknown`, `partial`, `unsupported`, `setup_missing`, or `budget_exceeded`.
- Summary persistence must be registry-ready locally: package/version identity, schema version, content digest, provenance, validation metadata, recompute-and-diff, and future trust hooks. No publish/fetch protocol.
- Search must be candidate retrieval over stable store document IDs. Search results do not create semantic facts and must not become inputs to deterministic `polint check`.
- Validation requirements are first-class: parity, migration, crash/recovery, determinism matrix, mutation invalidation fixtures, query correctness, public-boundary proof, and large-scale benchmarks.

### Suggested Phase Structure

Every phase must name which milestone outcome gate it advances (scale, latency, honesty, accuracy visibility). Benchmarks and regression budgets from Phase 0 run at every phase boundary — the store must never silently re-inflate the memory or cold-latency wins already landed (capability-gated pipeline, rule-scoped discovery).

**Phase 0: Ground Truth and Performance Baseline**
Rationale: the locked research puts measurement first; without baselines, no later phase can prove it moved an outcome gate, and store overhead regressions stay invisible.
Delivers: real-repo benchmark suite (production-scale monorepo + OSS repos), RSS/latency/store-size curves vs repo and diff size, store-disabled baselines for `check`/`review`, budget-exhaustion telemetry, persisted-graph recall/precision baseline on existing callgraph benchmarks.
Avoids: benchmarks landing after public surfaces, untracked ingest overhead, unrecorded accuracy baseline.

**Phase 1: Store Foundation and Boundary Proof**
Rationale: every later feature depends on a private, crash-safe store boundary.
Delivers: `rusqlite` bundled dependency, private store module, connection policy, migrations, manifest table, no-op integration, no-leak tests.
Avoids: public SQL/table contract, second cache identity, direct provider SQL access.

**Phase 2: Generation Manifest and Metadata Mirroring**
Rationale: invalidation and recovery need the existing kernel vocabulary in the store before facts are broadly ingested.
Delivers: input snapshots, provider manifests, layer entries/dependencies, validation events, provider generations, store stats.
Avoids: stale reuse, mixed generations, machine-specific path leakage.

**Phase 3: Validated Fact and Graph Index Ingest**
Rationale: graph/query commands need normalized validated facts and stable semantic IDs before public surfaces exist.
Delivers: files, packages/modules, imports/resolutions, symbols/definitions/references, FactMeta metadata, adjacency/evidence foundations, internal xref tests.
Avoids: raw AST/source dumping, unordered output, store row IDs escaping.

**Phase 4: Summary Persistence and Invalidation Frontier**
Rationale: O(change) `review` is the main practical payoff and must precede broad query promotion. This phase carries the milestone's scale and latency gates: once dependency summaries validate, dependency bodies are never re-parsed while their identity matches (O(working set) memory), and warm `review` recomputes only the instrumented invalidation frontier with a measured p50/p95 win over the Phase 0 baseline.
Delivers: summary manifests, content-addressed payload seams with `blake3`, dependency summary digests, warm reuse after from-scratch parity, recompute-and-diff hooks, frontier instrumentation, warm-review latency gate.
Avoids: summary overtrust, stale summaries, remote registry scope creep, warm reuse shipping without byte-identical cold/warm review parity.

**Phase 5: Internal Query Engine and Envelope**
Rationale: query semantics should be proven privately before CLI promotion.
Delivers: internal used-by, neighbors, callers, callees, path, and taint services; stable envelope structs; status/precision/unknown/budget propagation; query correctness fixtures.
Avoids: graph-as-CI, generic query language, unknown collapsed to not found.

**Phase 6: Public Graph CLI Promotion**
Rationale: only promote commands after determinism, correctness, no-leak, documentation, and benchmark gates are green.
Delivers: selected `polint graph` commands with JSON/human rendering, structured filters, docs with limits, public schema snapshots.
Avoids: SDK raw graph promotion, public query language, unstable internal IDs.

**Phase 7: Lexical Search Boundary**
Rationale: search should build on stable semantic document IDs and evidence envelopes. This phase is the designated scope-cut if the milestone runs long: it advances no scale/latency gate and can move to v2.1 without weakening the keystone (store + summaries + frontier + graph CLI).
Delivers: `SearchCorpus`, Tantivy manifest, lexical search over symbols/evidence/diagnostics/summaries/snippets, active-store back-references, deterministic rebuild.
Avoids: Tantivy IDs as semantic IDs, search as fact source, vector creep.

**Phase 8: Recovery, Pruning, and Scale Gates**
Rationale: default store reuse is only credible after crash, pruning, WAL, payload, and large-repo behavior are measured.
Delivers: stale-generation pruning, payload garbage collection, WAL/checkpoint policy, BLOB-vs-file decision, 100k/500k/1M+ row benchmarks, CI restore parity.
Avoids: orphaned payloads, unbounded DB/WAL growth, non-portable cache restore.

### Research Flags

Needs deeper phase research:
- **Phase 4:** payload layout: SQLite BLOBs, adjacent content-addressed files, or hybrid.
- **Phase 5:** path/taint traversal model, cycle handling, barriers, ranking, and budget semantics.
- **Phase 6:** first stable public graph JSON schema and promotion checklist per command.
- **Phase 7:** Tantivy code tokenization, field schema, rebuild lifecycle, and exact candidate wording.
- **Phase 8:** pruning/vacuum/checkpoint policy and large-store benchmark thresholds.

Standard patterns, likely no extra research needed:
- **Phase 1:** rusqlite facade, migrations, open modes, and public-boundary tests are well understood.
- **Phase 2:** mirroring existing kernel metadata follows current `InputSnapshot`, provider manifest, layer cache, and validation patterns.
- **Phase 3:** normalized fact ingest can reuse existing fact metadata and stable key discipline.

## Anti-Features and Non-Goals

- No remote package-summary registry in v2.0. Build only local registry-ready seams.
- No public raw SQL, Cypher, Datalog, QL, SPARQL, generic graph shell, or table inspection command.
- No public raw graph SDK views or broad graph internals. Promote only deliberate typed SDK views after validation.
- No `polint graph` CI pass/fail semantics. Enforce policy through Rust rules and `polint check`/`polint review`.
- No stable vector search in the first implementation. `sqlite-vec` remains experimental and off by default.
- No live embedding generation or model downloads in deterministic commands.
- No replacement parser/frontend stack. Continue using Oxc for TS/JS and tree-sitter-go plus existing semantic providers for Go.
- No full AST/source/MIR/CFG dumps as the product store.
- No dual-store graph architecture by default and no redb parity claim unless tests prove it.
- No search result treated as semantic truth or policy violation without graph/provider verification.
- No daemon/server requirement for local analysis.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | SQLite/rusqlite bundled and `blake3` are locked by local research; Tantivy/sqlite-vec/redb are correctly phase-gated. |
| Features | HIGH | Table stakes, differentiators, anti-features, and workflows are consistent across `FEATURES.md`, project context, and locked local-store research. |
| Architecture | HIGH | Private `analysis_kernel::store` direction reuses the existing provider DAG, `AnalysisDb`, metadata, cache keys, validation, and public-boundary discipline. |
| Pitfalls | HIGH | Pitfalls are concrete, phase-mapped, and testable; most correspond to existing polint risks around determinism, invalidation, provenance, and public API leaks. |

**Overall confidence:** HIGH.

### Gaps to Address

- Payload layout needs benchmark evidence before choosing DB BLOBs, adjacent files, or a hybrid.
- Exact schema and migration count should evolve with implementation, but the migration harness and `PRAGMA user_version` pattern should land first.
- First public graph envelope needs command-by-command schema snapshots before promotion.
- Query performance needs large synthetic and real-repo benchmarks for recursive CTEs versus Rust scoped traversal.
- Search tokenization for code identifiers needs design during the Tantivy phase.
- Store cleanup semantics for `polint cache status/clean/prune` need a product decision before default large-repo reuse.

## Sources

### Primary

- `.planning/research/STACK.md` - stack additions, versions, rejections, integration points, risks.
- `.planning/research/FEATURES.md` - feature table stakes, differentiators, workflows, requirement seeds, MVP order.
- `.planning/research/ARCHITECTURE.md` - module plan, data flow, build order, validation hooks.
- `.planning/research/PITFALLS.md` - critical and moderate pitfalls, phase placement, must-test matrix.
- `.planning/PROJECT.md` - current v2.0 milestone goal, constraints, product boundaries, prior validated substrate.
- `research/static-analysis-2.0/README.md` - locked product vision, goals, rough workstream order, registry deferral.
- `research/local-semantic-store/README.md` - locked SQLite/rusqlite store decision, search/vector boundaries, excluded first-implementation scope.

---
*Research completed: 2026-07-07*
*Ready for roadmap: yes*
