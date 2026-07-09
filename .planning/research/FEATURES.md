# Feature Research — Static Analysis 2.0

**Domain:** Durable local semantic store and agent-facing graph/query layer for polint.
**Researched:** 2026-07-07
**Confidence:** HIGH for feature categories and product boundaries; MEDIUM for exact complexity until real store/query benchmarks land.

## Feature Categories

Static Analysis 2.0 should not change what polint is. `polint check` remains the repo-local Rust policy engine. `polint review` remains the agentic, diff-time review workflow. The new v2.0 capabilities are the durable semantic layer underneath those products and an exploratory `polint graph` surface for users and agents that need to understand code before turning an insight into a policy.

The first implementation should focus on five feature groups:

| Category | Product Behavior | Complexity | Existing Engine Dependencies |
|----------|------------------|------------|------------------------------|
| Local semantic store | Persist validated facts, graph indexes, summaries, provenance, status, and invalidation metadata in an embedded SQLite/rusqlite store behind a private facade. | High | Provider manifests, `InputSnapshot`, layer/query/summary keys, provenance metadata, validation status, semantic graph, evidence facts. |
| Summary persistence | Store registry-ready summary manifests and content-addressed payloads for dependency and application summaries without building a remote registry. | High | Summary kernel, SCC summary cache, direct/refined call facts, data-flow facts, package/module identities, cache digest discipline. |
| Incremental reuse | Make warm `check`/`review` work proportional to changed functions/SCCs and their summary dependents, not the full repo. | High | Layer cache, dependency indexes, function/MIR digests, summary keys, demand queries, deterministic provider DAG. |
| Local graph queries | Add bounded, purpose-built `polint graph` commands for used-by, neighbors, callers/callees, paths, taint-style reachability, and search. | Medium-High | Shared semantic graph, stable symbol/function/callsite IDs, evidence bundles, unknown/budget reporting, path search. |
| Search boundary | Add Tantivy lexical search over stable semantic-store document IDs; leave vector search unstable/deferred behind explicit model and chunker provenance. | Medium | Stable semantic IDs, summary/evidence text, source metadata, store document table, query result envelope. |

## Table Stakes

These are required for v2.0 to feel coherent to users and useful to agents. Missing any of these either breaks trust in `check`/`review`, makes `graph` too unstable to use, or undermines the local-store scaling goal.

| Feature | Expected Behavior | Complexity | Dependencies / Notes |
|---------|-------------------|------------|----------------------|
| Private SQLite/rusqlite store facade | Store setup, migrations, schema versioning, generation tracking, and recovery are handled internally. Users never depend on table names or SQL. | High | Use bundled SQLite via `rusqlite`; reuse existing cache/schema digest discipline. |
| Durable fact persistence | Validated source, package, module, symbol, reference, import, export, function, call, flow, evidence, and summary identities persist with precision, status, provenance, provider, and validation metadata. | High | Depends on v1.2/v1.3 semantic graph and metadata sidecars. Persist only normalized facts and indexes, not raw ASTs as public artifacts. |
| Summary manifests and payloads | Dependency summaries are keyed by package/version/schema/config; application summaries are keyed at function/SCC granularity with Merkle-style dependent summary digests. | High | Depends on summary kernel, SCC scheduling, package identities, and content hashes. Remote registry remains out of scope. |
| Invalidation frontier | A changed file/function invalidates its summary and transitive summary dependents; unaffected dependency/package summaries remain reusable. | High | Depends on `InputSnapshot`, summary keys, dependency indexes, and deterministic query/layer keys. This is the main `polint review` latency lever. |
| `check` preservation | `polint check` remains the CI/policy command. Store use must be transparent, deterministic, and validated by cold/warm parity tests. | Medium | Existing CLI JSON/SARIF-like behavior and exit semantics must not drift because a local store exists. |
| `review` preservation | `polint review` uses the store for diff-focused evidence and summaries, but still reports unknowns, setup gaps, and budget exhaustion honestly. | Medium-High | Depends on diff gating, evidence bundles, demand queries, summary invalidation, and unknown taxonomy. |
| Exploratory `polint graph` | `polint graph` answers local understanding questions and returns evidence. It is not a pass/fail interface and should not become the CI gate. | Medium-High | Commands: `used-by`, `neighbors`, `callers`, `callees`, `path`, `taint`, `search`. Promote recurring policy needs into Rust rules. |
| Stable JSON envelope | Graph output uses one agent-friendly envelope with `version`, `schema`, `command`, `query`, `status`, `precision`, `nodes`, `edges`, `paths`, `findings`, `unknowns`, and `summary`. | Medium | Status values should include `complete`, `partial`, `not_found`, `unknown`, `budget_exceeded`, `unsupported`, and `setup_missing`. |
| Structured filters | Graph/search commands support normal filters: path glob, tests on/off, minimum precision, provenance, unknown handling, max depth, max paths, and limit. | Medium | Depends on persisted metadata and bounded query execution. Filters must be documented as query filters, not precision guarantees. |
| Bounded path and taint queries | Path and taint-style reachability queries are budgeted, evidence-backed, and explicit about unknown or budget-exceeded results. | High | Depends on existing path search, evidence bundles, data-flow facts, summaries, and budget telemetry. |
| Tantivy lexical search | Search covers symbols, summary text, evidence/diagnostic text, rule explanations, and selected snippets through stable semantic-store document IDs. | Medium | Search is an index over the semantic corpus. It should not be a live nondeterministic input to `check`. |
| Validation gates | Cold/warm parity, crash/recovery, stale reuse prevention, migration behavior, query correctness fixtures, unknown/budget preservation, and benchmark coverage are required. | High | Must reuse existing evaluation harness and benchmark gates. Store correctness is more important than a large query surface. |
| Public-boundary proof | Raw SQL, SQLite schema, graph internals, parser IDs, provider IDs, absolute paths, source bodies, and solver IDs stay private until intentionally promoted. | Medium | Mirrors existing public API discipline. The public surface is SDK views and CLI envelopes, not the store. |

## Differentiators

These features are not just infrastructure. They are where Static Analysis 2.0 becomes valuable for AI-assisted engineering rather than another cache layer.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Local semantic memory for agents | Agents can ask "what uses this?", "what path connects this source to that sink?", or "what changed if this API moves?" and receive stable IDs plus evidence instead of re-parsing the repo through prompts. | High | This is the long-term product promise. Keep output compact and machine-readable. |
| Honest graph exploration | Precision, provenance, unknowns, setup gaps, and budgets are first-class in every query result. | Medium | Differentiates polint from tools that return a clean-looking but silently incomplete graph. |
| O(change) review foundation | `polint review` can reuse persisted dependency summaries and recompute only changed functions/SCCs plus dependents. | High | This is the practical payoff of summary persistence and the reason incrementality must follow the summary store. |
| Registry-ready without registry | Content-addressed manifests, package/version identity, schema versions, validation metadata, recompute-and-diff hooks, and future trust hooks are present locally before any networked service exists. | Medium-High | Preserves the option for a future package-summary registry without taking on distribution/security work now. |
| Exploratory-to-policy workflow | A user or agent can explore with `polint graph`, then codify a recurring rule as repo-local Rust consumed by `polint check`/`review`. | Medium | This keeps `graph` ergonomic without giving it CI semantics. |
| Semantic lexical search | Tantivy search over symbol/evidence/summary documents is more useful to agents than raw text grep because results point back to stable semantic IDs. | Medium | Start lexical. Vector search only after model/chunker/provenance lockfiles exist. |
| Recompute-and-diff confidence | Stored summaries and payloads can be recomputed and compared against persisted versions, giving users a clear way to detect stale or poisoned data later. | Medium | Important for future registry trust, but useful locally for debugging store correctness. |
| Query envelopes shaped for automation | One stable JSON result shape lets agents consume `used-by`, path, taint, and search without one-off parsers per command. | Medium | Human output can exist, but JSON should be the design center. |

## Anti-Features

These should be rejected during roadmap creation even if they appear to speed up delivery.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Making `polint graph` a CI gate | It blurs exploratory understanding with enforceable policy and creates a second rule system. | Keep CI semantics in `polint check` and `polint review`; promote durable policies into Rust rules. |
| Public SQL, Cypher, QL, SPARQL, or a generic graph shell | It exposes unstable internals and creates a large support contract before facts and query patterns settle. | Ship purpose-built graph commands with stable envelopes and bounded filters. |
| Remote package-summary registry in v2.0 | Registry operations, trust, signing, poisoning, and corpus management distract from proving local value. | Build registry-ready manifests and payload seams locally; defer networked registry. |
| Stable vector search in the first implementation | Embeddings need model ID, digest, chunker version, dimensions, metric, normalization, provenance, and deterministic lockfiles. | Ship Tantivy lexical search first; keep vector search experimental and explicitly isolated. |
| Embeddings or search indexes affecting `polint check` | Nondeterministic or model-dependent inputs would compromise policy reproducibility. | Treat search as exploration over persisted semantic documents, not a rule input unless later promoted through explicit validated facts. |
| Exposing raw graph/provider/parser/solver IDs | These IDs are implementation details and can leak unstable architecture into agents and rule packs. | Expose stable semantic IDs, relative paths, spans, precision, status, provenance, and evidence links. |
| Dual-store graph architecture by default | A separate graph DB risks drift between facts, summaries, and query indexes. | Use SQLite adjacency/evidence tables plus bounded Rust/SQL query code as the canonical v2.0 backend. |
| Persisting full AST/source dumps as the product store | Large payloads increase disk/memory pressure and create privacy/support concerns without being the public contract. | Persist normalized facts, summaries, indexes, and content-addressed payloads where needed. |
| Silent stale reuse | Reusing stale summaries is worse than a cold run because it produces confident wrong answers. | Store generation/schema/config/provider digests and fail closed with rebuilds or explicit diagnostics. |
| Broad public SDK promotion for raw graph internals | Premature public APIs freeze the wrong abstractions. | Keep new store/query internals crate-private; promote typed SDK views only after validation gates. |
| Whole-repo rebuild as the normal warm path | It wastes the summary-store investment and keeps `review` too slow for agents. | Recompute by invalidation frontier: changed functions/SCCs plus summary dependents. |
| Replacing editor LSP/navigation | polint's graph can answer semantic analysis questions, but should not become an editor protocol implementation. | Return local CLI evidence and stable IDs; let editors integrate later if demand proves it. |

## User/Agent Workflows

### 1. Existing Policy Workflow

User runs `polint check` as before. The store may be created or refreshed behind the scenes, but diagnostics, output formats, exit codes, and public rule behavior remain stable. If the store is missing or invalid, polint rebuilds required layers or reports a controlled internal/capability diagnostic rather than silently producing partial policy results.

### 2. Agentic PR Review

An agent runs `polint review` on a diff. polint maps changed files to changed functions/SCCs, invalidates affected summaries, reuses unchanged dependency and application summaries, runs demand queries for changed code, and emits findings with evidence, unknowns, precision labels, and budget status. This workflow is the first place users should feel warm-run wins.

### 3. Local Graph Exploration

User or agent asks:

```bash
polint graph used-by --at src/api.ts:42:15 --format json
polint graph callers --symbol function:...
polint graph path --from function:... --to function:... --edge calls --max-depth 8
```

The command returns a bounded evidence envelope. `status: partial` or `budget_exceeded` is a valid answer; it must be visible, not hidden behind an empty result.

### 4. Taint-Style Investigation

Agent asks whether user-controlled input can reach a sink:

```bash
polint graph taint --source "http.request.*" --sink "sql.query" --barrier "sanitize.*" --format json
```

The result should include paths, barriers considered, unknowns, and budget state. This is exploratory. If the team wants enforcement, they write a Rust rule over promoted typed views.

### 5. Semantic Search

Agent searches:

```bash
polint graph search "sanitize email" --kind symbol,evidence,summary --format json
```

Tantivy returns semantic-store document IDs that resolve to symbols, summaries, evidence, or selected snippets. Results should be useful for navigation and investigation, not a source of policy truth.

### 6. CI Cache Restore

CI restores `.polint` cache/store artifacts. polint validates schema generation, provider digests, config digests, and summary manifests before reuse. Invalid or stale entries are rebuilt or quarantined with explicit diagnostics. Cold/warm parity tests must prove restored store artifacts do not change policy output.

### 7. Exploration Becomes Policy

A recurring `polint graph` query reveals a local architectural rule. The user implements a repo-local Rust rule using `polint::sdk::prelude::*`, validates it through `polint check --format json`, and lets `polint review` use it in PR workflows. This is the intended route from understanding to enforcement.

## Requirement Seeds

These seeds are phrased for roadmap/requirements creation. They intentionally focus on user-visible behavior and the existing engine dependencies that make each feature possible.

| ID | Requirement Seed | Category | Complexity | Existing Engine Dependency |
|----|------------------|----------|------------|----------------------------|
| V2-FEAT-01 | Provide a private SQLite/rusqlite semantic-store facade with migrations, schema versioning, generation tracking, and crash-safe open/recovery behavior. | Table stakes | High | Cache directory discipline, schema digests, provider manifests. |
| V2-FEAT-02 | Persist normalized validated facts and graph adjacency/evidence indexes with stable semantic IDs, relative paths, precision, status, provenance, provider, and validation metadata. | Table stakes | High | Semantic graph, metadata sidecars, evidence facts, stable IDs. |
| V2-FEAT-03 | Persist summary manifests and content-addressed payload seams for dependency package summaries and application function/SCC summaries. | Table stakes | High | Summary kernel, SCC cache, package/module graph, summary keys. |
| V2-FEAT-04 | Implement invalidation dependencies so warm runs recompute changed functions/SCCs and transitive summary dependents while reusing valid dependency summaries. | Table stakes | High | `InputSnapshot`, layer keys, summary keys, dependency indexes, demand queries. |
| V2-FEAT-05 | Preserve `polint check` output and exit semantics while allowing the store to accelerate or back typed fact views only when cold/warm parity is proven. | Table stakes | Medium | Existing CLI, runner, diagnostics, cache, SDK views. |
| V2-FEAT-06 | Preserve `polint review` as the diff-focused agent workflow and use the store for bounded evidence, unknown, and budget-aware recomputation. | Table stakes | Medium-High | Diff gating, demand queries, path/evidence bundles, summary invalidation. |
| V2-FEAT-07 | Add exploratory `polint graph` commands for used-by, neighbors, callers, callees, path, taint, and search with no CI pass/fail semantics. | Table stakes | Medium-High | Shared graph, path search, data-flow facts, evidence bundles. |
| V2-FEAT-08 | Standardize graph JSON output around one envelope with status, precision, nodes, edges, paths, findings, unknowns, and summary counts. | Table stakes | Medium | Existing JSON rendering discipline, stable IDs, metadata sidecars. |
| V2-FEAT-09 | Support graph filters for path globs, tests, minimum precision, provenance, unknown inclusion, depth, path count, and result limit. | Table stakes | Medium | Persisted metadata, bounded query execution, graph indexes. |
| V2-FEAT-10 | Build a Tantivy lexical-search boundary over stable semantic-store documents for symbols, evidence, diagnostics, summaries, and selected snippets. | Differentiator | Medium | Store document IDs, symbol/evidence/summary text, query envelopes. |
| V2-FEAT-11 | Add validation gates for cold/warm parity, restored-store parity, crash/recovery, migration, stale reuse, query correctness, unknown preservation, and budget preservation. | Table stakes | High | Evaluation harness, benchmark gates, deterministic output fixtures. |
| V2-FEAT-12 | Prove public-boundary discipline: no raw SQL, store schema, graph internals, parser IDs, provider IDs, solver IDs, absolute paths, or source bodies leak into stable CLI/SDK output. | Table stakes | Medium | Public API visibility plan, CLI snapshot tests, SDK no-leak tests. |
| V2-FEAT-13 | Keep registry-ready seams by recording package/version identity, schema versions, content digests, provenance, validation metadata, recompute-and-diff hooks, and future trust-hook placeholders. | Differentiator | Medium-High | Summary manifests, package graph, validation metadata. |
| V2-FEAT-14 | Keep vector search deferred behind an unstable boundary requiring model ID, model digest, chunker version, dimensions, metric, normalization, provenance, and content digest before promotion. | Anti-feature guard | Medium | Search boundary, document IDs, config/digest infrastructure. |

## MVP Recommendation

Prioritize in this order:

1. Store facade, migrations, generation tracking, and private boundary proof.
2. Durable fact and summary persistence using existing semantic graph, summary keys, provenance, and validation metadata.
3. Invalidation frontier and cold/warm parity gates for `check` and `review`.
4. Minimal exploratory `polint graph used-by`, `neighbors`, `callers`, and `callees` with the stable JSON envelope and filters.
5. Path/taint queries and Tantivy lexical search once the store/query correctness gates are green.

Defer remote registry, stable vector search, public SQL/query language, broad SDK graph promotion, and CI semantics for `polint graph`.

## Sources

- `.planning/PROJECT.md`
- `research/static-analysis-2.0/README.md`
- `research/static-analysis-2.0/03-summary-store.md`
- `research/static-analysis-2.0/04-incrementality.md`
- `research/local-semantic-store/FINAL-REPORT.md`
- `research/local-semantic-store/implementation/QUERY-CLI-SKETCH.md`
- `research/local-semantic-store/decisions/DECISIONS.md`
