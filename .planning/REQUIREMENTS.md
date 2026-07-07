# Requirements: polint v2.0 Static Analysis 2.0 Implementation

**Defined:** 2026-07-07
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Milestone goal:** Build the Static Analysis 2.0 implementation foundation that turns polint's private analysis engine into a durable, queryable local semantic layer for custom Rust rules, agentic review, and future local graph exploration.

**Source context:** v1.2 built the private analysis engine substrate. v1.3 and v1.4 promoted graph precision and policy-query ergonomics. The locked Static Analysis 2.0 research says the next implementation step is a private local semantic store, durable summaries, invalidation, bounded queryability, and search foundations. `polint check` and `polint review` remain the core products. A remote registry of pre-computed package summaries is explicitly deferred.

## Product and Architecture Contract

Static Analysis 2.0 is not a replacement for repo-local Rust rules. It is the layer that helps AI agents and developers understand, explore, and enforce code and architecture quality in large codebases.

The long-term promise is that polint becomes a local semantic layer for AI-assisted engineering: an agent can ask what uses an API, why a path exists, whether user-controlled data can reach a sink, which unknowns block certainty, and then turn recurring findings into repo-local Rust rules enforced by `polint check` and `polint review`.

The v2.0 implementation must therefore optimize for:

1. Durable, validated local analysis state.
2. Honest graph and summary query results with precision, provenance, unknowns, and budgets.
3. Fast warm `polint review` through summary reuse and invalidation.
4. Registry-ready package summary manifests without a remote registry.
5. A future CLI query surface over the program graph without exposing raw internal storage or solver APIs.

## Locked Technology Decisions

- Use `rusqlite` with bundled SQLite as the primary embedded local semantic store.
- Keep the store private under the analysis kernel; `rusqlite` types, SQL, tables, row IDs, provider generation IDs, and parser/solver internals must not become public contracts.
- Add `blake3` for content-addressed summary and payload identities where strong content IDs are needed. Do not silently replace existing cache keys.
- Add Tantivy only in the lexical-search phase, after stable semantic document IDs exist.
- Keep vector search deferred behind an unstable/experimental boundary requiring model, chunker, dimension, metric, normalization, provenance, and content-digest lockfiles.
- Do not build a remote package-summary registry in v2.0. Build local registry-ready seams only.
- Do not add public SQL, Cypher, Datalog, QL, SPARQL, a generic graph shell, or public raw graph SDK views.

## v2.0 Requirements

Requirements for the v2.0 milestone. Each requirement should map to exactly one roadmap phase after roadmap generation.

### Product Boundaries

- [ ] **PROD-01**: `polint check` remains the repo-local Rust policy command. Store creation, refresh, corruption, or invalid schema must not silently change diagnostics, output formats, exit semantics, or public rule behavior.
- [ ] **PROD-02**: `polint review` remains the diff-focused agentic review workflow and should be the first workflow to benefit from warm summary reuse and invalidation-frontier recomputation.
- [ ] **PROD-03**: `polint graph` is an exploratory local understanding surface for users and agents. It is not a CI pass/fail interface and must not become a second rule system.
- [ ] **PROD-04**: Recurring graph findings become enforcement by writing repo-local Rust rules consumed by `polint check` and `polint review`, not by making graph queries into hidden policies.
- [ ] **PROD-05**: Public docs, generated skill text, examples, SDK exports, and CLI output must describe Static Analysis 2.0 as durable local analysis infrastructure, not as a remote registry product or bundled production ruleset.

### Store Foundation

- [ ] **STORE-01**: Add a private SQLite/rusqlite semantic-store facade owned by the analysis kernel, with `pub(crate)` boundaries and no escaped `rusqlite` connection, statement, row, or SQL-string types.
- [ ] **STORE-02**: Support migrations, schema versioning through `PRAGMA user_version`, controlled diagnostics for future/invalid schemas, and safe rebuild or skipped-persistence behavior.
- [ ] **STORE-03**: Use explicit connection policy: foreign keys enabled, WAL where appropriate, bounded busy timeout, one writer boundary, and separate read-only query connections.
- [ ] **STORE-04**: Persist store manifest, active generation, pending generation, complete generation, schema version, workspace/config identity, and store stats.
- [ ] **STORE-05**: Commit only complete validated generations. A crash, failed migration, failed payload write, or failed search rebuild must leave either the old complete generation readable or require an explicit rebuild diagnostic.
- [ ] **STORE-06**: Providers and rule execution do not receive SQL connections. They communicate through typed kernel/store methods and existing provider output structures.
- [ ] **STORE-07**: Store failure during `polint check` produces controlled internal diagnostics, rebuilds, or skipped persistence; it must not produce partial policy answers with confident output.

### Metadata, Facts, and Invalidation

- [ ] **META-01**: Mirror existing kernel identity vocabulary in the store: `InputSnapshot`, provider manifests, layer keys, summary keys, query keys, provider output metadata, validation events, and dependency indexes.
- [ ] **META-02**: Persist normalized validated facts and indexes for files, packages/modules, imports/exports, resolutions, symbols, definitions, references, functions, calls, flow, evidence, summaries, unknown regions, and budget events as they become available.
- [ ] **META-03**: Every fact-like row carries stable semantic identity, repository-relative path identity where applicable, fact family, provider/schema identity, precision, confidence/status, provenance, validation state, dependency metadata, and generation.
- [ ] **META-04**: Invalidation dependencies include source files, packages/projects, provider manifests, requested capabilities, language lifecycle inputs, config, schema, summary keys, query options, budget profiles, search manifests, and future model/extension digests where relevant.
- [ ] **META-05**: Deterministic public/query output never depends on SQLite `rowid`, insertion order, unordered Rust maps, parallel provider completion order, or Tantivy internal document IDs.
- [ ] **META-06**: The store does not persist full AST, source, MIR, CFG, or whole raw graph dumps as the product storage model. Persist normalized facts, summaries, indexes, compact evidence, digests, spans, and payload references.
- [ ] **META-07**: `unknown`, `unsupported`, `setup_missing`, `partial`, and `budget_exceeded` states remain durable and queryable. They must never collapse into `not_found` or an empty result.

### Summary Persistence and Registry-Ready Seams

- [ ] **SUM-01**: Persist summary manifests for dependency package summaries and application function/SCC summaries with package/version identity, schema version, toolchain/frontend identity, config digest, provenance, validation metadata, and precision/status.
- [ ] **SUM-02**: Use content-addressed payload IDs for summary payloads and registry-ready package summary seams. Introduce typed digest wrappers so cache invalidation keys and content-addressed payload digests cannot be confused.
- [ ] **SUM-03**: Decide summary payload layout through validation before locking it in: SQLite BLOBs, adjacent content-addressed files, or a hybrid must be benchmarked for DB size, WAL growth, crash behavior, restore behavior, and read latency.
- [ ] **SUM-04**: Implement the invalidation frontier so warm runs recompute changed functions/SCCs plus transitive summary dependents while reusing valid dependency and unaffected application summaries.
- [ ] **SUM-05**: Enable summary reuse only after from-scratch parity, recompute-and-diff checks, manifest validation, and stale-reuse prevention fixtures pass.
- [ ] **SUM-06**: Store summary-derived facts with explicit precision, confidence/status, provenance, digest identity, validation state, and trust placeholders. Heuristic summaries must remain labeled heuristic.
- [ ] **SUM-07**: Build no remote registry, publish protocol, fetch protocol, auth/signing layer, or central corpus in v2.0. The milestone only preserves local manifest/payload seams that can support a future registry.

### Internal Query Engine

- [ ] **QUERY-01**: Add private internal query services over complete store generations for used-by, neighbors, callers, callees, path, taint-style reachability, and search candidate resolution.
- [ ] **QUERY-02**: Standardize one internal query result envelope that can later render CLI JSON with `version`, `schema`, `command`, `query`, `status`, `precision`, `nodes`, `edges`, `paths`, `findings`, `unknowns`, `budgets`, and `summary`.
- [ ] **QUERY-03**: Query status values include at least `complete`, `partial`, `not_found`, `unknown`, `budget_exceeded`, `unsupported`, and `setup_missing`; `not_found` is valid only when the available evidence is complete enough for that claim.
- [ ] **QUERY-04**: Query filters cover path globs, tests included/excluded, minimum precision, provenance, unknown handling, max depth, max paths, and result limits where relevant.
- [ ] **QUERY-05**: Path and taint-style queries are bounded, cycle-aware, deterministic, evidence-backed, and explicit about barriers, summaries, unknown regions, and budget exhaustion.
- [ ] **QUERY-06**: Query results use stable semantic IDs, repository-relative paths, spans, precision, provenance, evidence IDs, and status fields. Raw store row IDs, provider IDs, parser IDs, solver IDs, and SQL names never appear.
- [ ] **QUERY-07**: Search results are candidates over stable store document IDs. Search does not create semantic facts and must not become an input to deterministic `polint check` behavior.
- [ ] **QUERY-08**: Query correctness fixtures land before public CLI promotion and cover cross-file refs, cross-package imports, direct/refined calls, cycles, paths, taint barriers, summary boundaries, setup gaps, unknown-preserving no-results, and budget exhaustion.

### Local CLI Graph Surface

- [ ] **CLI-01**: Promote selected `polint graph` commands only after the underlying internal query fixtures, no-leak gates, determinism gates, docs, and benchmark gates pass.
- [ ] **CLI-02**: Initial graph commands should cover used-by, neighbors, callers, callees, path, taint-style reachability, and lexical search as phase gates allow.
- [ ] **CLI-03**: Graph commands provide agent-friendly JSON as the design center and human output as a convenience renderer over the same private query envelope.
- [ ] **CLI-04**: The public graph CLI exposes bounded purpose-built commands and structured filters, not SQL, table inspection, Cypher, Datalog, QL, SPARQL, or a generic graph shell.
- [ ] **CLI-05**: CLI docs explain limits, precision, unknowns, budgets, summary-backed evidence, and the exploration-to-policy workflow honestly.
- [ ] **CLI-06**: Promoted CLI JSON schemas have snapshots and compatibility notes. Internal store schema changes must not force public JSON schema changes.

### Search Boundary

- [ ] **SEARCH-01**: Define a `SearchCorpus` over stable semantic-store document IDs before adding a search engine dependency.
- [ ] **SEARCH-02**: Add Tantivy in the lexical-search phase for symbols, evidence text, diagnostic text, summaries, and selected snippets after store IDs and query envelopes are stable.
- [ ] **SEARCH-03**: Tantivy internal document IDs, segment state, and index layout remain private. Results map back to stable semantic document IDs and evidence/spans from the store.
- [ ] **SEARCH-04**: Search indexes are derived artifacts tied to store manifest/content digests and complete generations. Rebuild and swap must be crash-safe.
- [ ] **SEARCH-05**: Stable vector search is deferred. Any experimental vector work must stay off by default and require explicit model, chunker, dimension, metric, normalization, provenance, and content-digest metadata.

### Validation, Recovery, and Scale

- [ ] **VAL-01**: Cold build, warm build, restored-store build, partial invalidation, process restart, randomized provider order, and different Rayon worker counts produce byte-identical normalized policy and query JSON where semantics are unchanged.
- [ ] **VAL-02**: Migration tests cover empty DB, previous schema, idempotent migration, future-schema refusal, invalid-schema rebuild path, and controlled diagnostics.
- [ ] **VAL-03**: Crash/recovery tests kill the process during SQLite ingest transaction, summary payload write, migration, WAL checkpoint, and search rebuild. Recovery must expose only a complete generation or a rebuild-needed diagnostic.
- [ ] **VAL-04**: Stale-reuse mutation fixtures cover source edits, package/lifecycle config changes, provider manifest changes, requested-capability changes, schema changes, summary dependency changes, query option changes, and budget-profile changes.
- [ ] **VAL-05**: Unknown, unsupported, setup-missing, partial, and budget-exceeded behavior is covered by fixtures and remains visible in graph/query/review output.
- [ ] **VAL-06**: Public-boundary leak gates prove SQL, table names, raw row IDs, provider generation IDs, parser IDs, solver IDs, raw graph internals, and store payload formats are absent from SDK prelude, CLI JSON, README, docs/facts, examples, and generated skill text.
- [ ] **VAL-07**: Scale benchmarks cover ingest/query p50 and p95, DB and WAL size, RSS, pruning/vacuum cost, recursive CTE versus Rust traversal, and payload BLOB versus adjacent file behavior at 100k, 500k, and 1M+ row scales where practical.
- [ ] **VAL-08**: Cache/status/clean/prune behavior accounts for semantic-store generations, payloads, search indexes, stale rows, WAL/checkpoint policy, and orphaned payload cleanup.
- [ ] **VAL-09**: External temp-repo tests continue to prove repo-local rules import only `polint::sdk::prelude::*`, register through `polint::runner::run_cli`, consume public typed views, and observe unchanged `polint check --format json` behavior.

## Future Requirements

Deferred to later milestones unless explicitly pulled forward.

### Remote Registry

- **REG-FUT-01**: Add remote package-summary publish/fetch only after local manifest identity, recompute-and-diff, validation metadata, trust hooks, and poisoning/revocation models are proven.
- **REG-FUT-02**: Add signing, provenance verification, trust policy, corpus management, and cache-poisoning defenses before any remote registry affects analysis answers.
- **REG-FUT-03**: Add registry import/export CLI only with explicit user action and clear offline fallback behavior.

### Stable Query and Agent Surfaces

- **QUERY-FUT-01**: Promote a stable graph/query JSON schema after the exploratory CLI has survived at least one milestone of usage and compatibility review.
- **QUERY-FUT-02**: Add MCP/LSP/editor integrations over the query envelope once the local CLI schema is stable.
- **QUERY-FUT-03**: Promote deliberately scoped SDK fact views for graph/security queries only after CLI semantics, evidence, and no-leak gates are stable.

### Security Graph and Higher Precision

- **SEC-FUT-01**: Add deeper security graph analysis for user-controlled data, SQL injection, SSRF, command injection, auth boundaries, and sanitizer/barrier reasoning after path and taint-style query semantics are validated locally.
- **SEC-FUT-02**: Add higher-precision context sensitivity or solver modes behind explicit budgets and benchmark gates.
- **SEC-FUT-03**: Add framework/domain packs only after they can emit validated facts or documented heuristic summaries without hiding unknowns.

### Search and Vector

- **SEARCH-FUT-01**: Promote vector search only after deterministic model/chunker/provenance lockfiles, embedding invalidation, privacy controls, and replayable index builds exist.
- **SEARCH-FUT-02**: Add hybrid lexical/vector search as exploration only; search results must still not be policy truth without graph/provider verification.

## Out of Scope

Explicitly excluded from v2.0 to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Remote package-summary registry | The user explicitly deferred it. v2.0 builds local registry-ready seams only. |
| Public SQL, Cypher, Datalog, QL, SPARQL, or generic graph shell | Exposes unstable internals and creates a large support contract before command-specific semantics are proven. |
| Public raw graph SDK views | Raw graph APIs freeze internal storage and solver choices. Promote only deliberate typed views later. |
| `polint graph` as CI gate | Enforcement belongs in Rust rules through `polint check` and `polint review`. |
| Stable vector search | Requires model/chunker/provenance lockfiles and deterministic invalidation not needed for the first implementation. |
| Live embeddings or model downloads in deterministic commands | Would compromise offline reproducibility and policy determinism. |
| Full AST/source/MIR/CFG dumps as the store | Bloats local state, increases privacy/support risk, and exposes the wrong abstraction. |
| Dual graph store or graph database by default | Risks drift between facts, summaries, and query indexes. SQLite plus bounded Rust traversal is the v2.0 default. |
| Parser/frontend replacement | Continue using Oxc for TS/JS and tree-sitter-go plus existing semantic providers for Go. |
| Daemon/server requirement | v2.0 is an offline embedded local CLI foundation. |

## Traceability

Roadmap mapping will be generated after requirements approval.

| Requirement Area | Phase | Status |
|------------------|-------|--------|
| Product Boundaries | TBD | Draft |
| Store Foundation | TBD | Draft |
| Metadata, Facts, and Invalidation | TBD | Draft |
| Summary Persistence and Registry-Ready Seams | TBD | Draft |
| Internal Query Engine | TBD | Draft |
| Local CLI Graph Surface | TBD | Draft |
| Search Boundary | TBD | Draft |
| Validation, Recovery, and Scale | TBD | Draft |

---
*Requirements drafted: 2026-07-07*
