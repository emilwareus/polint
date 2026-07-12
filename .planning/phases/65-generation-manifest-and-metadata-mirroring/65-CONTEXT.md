# Phase 65: Generation Manifest and Metadata Mirroring - Context

**Gathered:** 2026-07-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 65 turns the private SQLite foundation into a durable metadata control plane. It persists store/run manifests, input snapshots, provider manifests and provider-generation records, layer/key metadata, fact metadata sidecars, dependency edges, validation events, and deterministic store statistics. It establishes pending/complete/active generation discipline and the complete invalidation vocabulary required by later warm reuse. It does not persist semantic fact payloads or graph adjacency, read persisted facts into rules, add summary payload reuse, expose query services, enable the store publicly, or add CLI/config/SDK contracts; those remain in Phases 66–71.

</domain>

<decisions>
## Implementation Decisions

### Generation Lifecycle and Reader Visibility
- **D-01:** Treat one full validated kernel run as the first atomic store generation. Each provider output receives a child provider-generation record, but provider generations are not activated independently until equivalent per-provider validation exists.
- **D-02:** Build a deterministically sorted `StoreCommitPlan` after the existing global fact validators run. A generation becomes readable only after its required metadata and validation events are written and the complete marker plus active selection change atomically.
- **D-03:** Readers must select the active generation and independently require complete status. They must never infer readability from the newest generation number, timestamp, row insertion order, or the presence of some provider rows.
- **D-04:** Keep interrupted pending and failed generation metadata isolated from the active generation so recovery can diagnose it and later pruning can remove it. Pending/failed generations are never semantic truth; the previous complete generation remains readable.
- **D-05:** Internal generation IDs and SQLite row IDs are relational handles only. Semantic equality, invalidation, deterministic ordering, and future result identities continue to use existing stable keys and typed digests.

### Kernel Identity Mirroring
- **D-06:** Mirror canonical fields from `InputSnapshot`, `ProviderManifest`, `ProviderOutputMeta`, `LayerKey`, `SummaryKey`, `QueryKey`, `DependencyIndex`, and `FactMeta` into typed first-class columns. Small extensible detail collections may use deterministic encoding, but core join and invalidation inputs must not be hidden in opaque JSON.
- **D-07:** Persist fact metadata using `FactFamily` plus `stable_key`, producer/layer identity, precision, confidence, validation status, and payload digest. `FactRef::run_id` remains a transient in-run join handle and must not become durable semantic identity.
- **D-08:** Persist metadata sidecars in this phase without persisting semantic fact payloads, source/AST/MIR/CFG bodies, or graph rows. Phase 66 remains responsible for normalized validated fact and graph-index ingest.
- **D-09:** When a required identity or dependency input is missing from the current in-memory model, extend the crate-private canonical kernel vocabulary first, add deterministic construction/serialization tests, and only then mirror it. Do not introduce store-only enums, magic strings, or a parallel digest scheme.
- **D-10:** Keep the current digest purposes distinct. Phase 65 reuses existing invalidation digests; it does not introduce content-addressed payload digests or silently replace cache-key hashing. Typed content/payload digests remain Phase 67 work.

### Invalidation Dependency Coverage
- **D-11:** Persist one canonical sorted and deduplicated dependency-edge set derived from `incremental::DependencyIndex`, with indexes supporting traversal from either endpoint. Reconstruct forward and reverse views from those rows rather than maintain two independently writable copies.
- **D-12:** The canonical dependency vocabulary must explicitly cover source files, packages/projects, provider manifests and schemas, requested capabilities, language lifecycle inputs and tool invocations, config, layer and summary dependencies, query options, budget profiles, and extension/model digests.
- **D-13:** Preserve `present`, `absent`, `unsupported`, and `setup_missing` input states through existing `InputSnapshot`/status vocabulary. Missing rows must not stand in for an explicit absent or unavailable input.
- **D-14:** A rule-pack change invalidates analysis layers only when it changes requested capabilities, analysis settings, model inputs, or extension facts. Rule execution and diagnostics retain their separate dependency boundary.
- **D-15:** Require a table-driven mutation matrix for every META-04 input class. Each class needs both a must-invalidate case and a must-preserve-hit control, plus stable dependency rows under shuffled construction/provider order. This phase proves the frontier vocabulary; it does not claim warm summary reuse.

### Validation, Recovery, and Store Telemetry
- **D-16:** Keep current global fact validation authoritative. Store-plan validation then checks generation/workspace identity, provider/schema references, unique stable identities, dependency endpoints, required validation events, deterministic row counts/digests, and generation completeness before activation.
- **D-17:** On write or validation failure, roll back generation-scoped writes, leave the previous complete active generation selected, and return a typed private store outcome. Record failure events against isolated pending/failed generation metadata when the store remains safe enough to do so.
- **D-18:** Require explicit rebuild only when schema or manifest integrity makes complete-generation selection untrustworthy. A merely incomplete or failed newer generation is ignored for reads and does not justify destroying a valid older generation.
- **D-19:** Persist deterministic per-generation counts and canonical digests for providers, layers, metadata, dependencies, and validation events, plus size accounting needed by later cache status/pruning work. Wall-clock timestamps and durations may be private telemetry but cannot affect semantic identity, normalized digests, generation selection, or deterministic output.
- **D-20:** Preserve Phase 64's private, harness-controlled activation and disabled zero-I/O public path. Phase 65 adds no public store flag, `.polint.toml` option, SDK export, table vocabulary, or store-derived policy diagnostic. `polint check` and `polint review` diagnostics and exit behavior remain byte-identical across store outcomes.

### the agent's Discretion
- Exact private Rust type names and the module split among generation, schema, manifest, dependency, validation, and test helpers.
- Whether pending/failed generation bookkeeping uses a dedicated status row or manifest pointers, provided reader selection and crash/failure invariants above are mechanically enforced.
- Exact SQL column types, indexes, foreign-key shapes, and deterministic encoding for small extensible lists, provided core identity fields remain first-class and round-trip without semantic loss.
- The injected-failure seam and exact private `StoreStatus` additions used by tests, provided no public behavior changes.
- The precise deterministic store-stat counters beyond the required provider/layer/metadata/dependency/validation counts and canonical digests.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope and Product Contracts
- `.planning/PROJECT.md` — v2.0 product goal, outcome gates, local-store boundary, and preserved `check`/`review` product model.
- `.planning/REQUIREMENTS.md` — authoritative STORE-04, STORE-05, META-01, and META-04 requirements and later-phase boundaries.
- `.planning/ROADMAP.md` — Phase 65 goal, dependency, success criteria, and sequencing into facts, summaries, queries, and recovery.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-CONTEXT.md` — inherited activation, ownership, connection, failure, no-leak, and performance decisions.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-04-SUMMARY.md` — completed public-boundary and regression-budget gate that Phase 65 must preserve.
- `docs/API-VISIBILITY-PLAN.md` — supported public surface and visibility discipline; store internals remain implementation detail.

### Locked v2.0 Architecture and Risks
- `.planning/research/SUMMARY.md` — locked complete-generation, identity-reuse, under-invalidation, and no-public-drift direction.
- `.planning/research/STACK.md` — SQLite schema/identity guidance, rejected parallel-store designs, and digest-vocabulary constraints.
- `.planning/research/ARCHITECTURE.md` — `StoreCommitPlan`, full-run generation, provider-generation children, module integration, and validation gates.
- `.planning/research/PITFALLS.md` — mixed-generation, under-invalidation, writer/read visibility, and crash-recovery prevention requirements.

### Local Semantic Store Contract
- `research/local-semantic-store/decisions/DECISIONS.md` — SQLite choice, registry deferral, and mandatory reuse of kernel manifests, snapshots, keys, metadata, and validators.
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — non-public contract, required metadata, commit protocol, invalidation boundary, and privacy rules.
- `research/local-semantic-store/RECOMMENDED_IMPLEMENTATION.md` — identity model, metadata schema families, lifecycle, sequencing, and future-phase boundaries.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/store/mod.rs`: existing crate-private `SemanticStore`, typed configuration/status outcomes, cache-path ownership checks, and controlled rebuild seam form the facade to extend.
- `crates/polint/src/analysis_kernel/store/connection.rs`: centralized WAL/foreign-key/busy-timeout policy, writer lease, and read-only connection boundary already enforce Phase 64 connection discipline.
- `crates/polint/src/analysis_kernel/store/migrations.rs`: strict `PRAGMA user_version` runner and bootstrap invariant provide the numbered migration path for the metadata schema.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs`: canonical `InputSnapshot` already captures sorted files, config, Go/TS lifecycle, rules, models, extensions, tools, explicit status, and provider schema snapshots.
- `crates/polint/src/analysis_kernel/provider.rs`: `ProviderManifest` is the authoritative provider ID/kind/input/output/language/cache/schema/precision source.
- `crates/polint/src/analysis_kernel/incremental/keys.rs`: typed `LayerKey`, `SummaryKey`, and `QueryKey` expose the exact digest inputs the store must mirror.
- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs`: sorted/deduplicated `CacheNode` and `DependencyEdge` vocabulary already provides canonical forward/reverse invalidation semantics.
- `crates/polint/src/analysis_kernel/metadata.rs`: deterministic `FactMetaStore` iteration, stable-key conflict tracking, and separation of `FactRef::run_id` from stable identity provide the metadata-sidecar source.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs`: `KernelRunReport` already groups input snapshot, provider outputs, cache stats, demand trace, and private store status for commit-plan construction and telemetry.

### Established Patterns
- `AnalysisKernel::run` finishes provider work, validates fact metadata, finalizes metadata insertions, and only then invokes store maintenance. Phase 65 must preserve this ordering while replacing no-op maintenance with validated metadata commit planning.
- Internal collections use `BTreeMap`/`BTreeSet`, explicit sorting/deduplication, typed digests, and schema labels. SQLite insertion order and row IDs cannot become observable ordering or identity.
- Store errors are typed private outcomes; `check` output and exit semantics are deliberately independent of store availability.
- The layer cache remains in place. SQLite mirrors its identity/dependency vocabulary first and does not become a competing cache or reader in this phase.

### Integration Points
- Extend the private store schema through a new numbered migration rather than altering the bootstrap invariant ad hoc.
- Build the `StoreCommitPlan` from `input_snapshot`, sorted provider outputs/manifests, layer/dependency metadata, and finalized `FactMetaStore` after existing validation in `AnalysisKernel::run`.
- Add typed store read helpers that select only an active complete generation; keep raw `rusqlite` connections, SQL strings, table names, and internal IDs inside `analysis_kernel::store`.
- Extend store fixtures and kernel integration tests for pending/failed/complete selection, old-generation fallback, injected commit failures, deterministic round trips, and the META-04 mutation matrix.
- Re-run `crates/polint/tests/public_surface_leak.rs`, Phase 64 parity fixtures, and the Phase 63 regression-budget boundary after metadata persistence lands.

</code_context>

<specifics>
## Specific Ideas

- Treat Phase 65 as the store's metadata control plane: complete-generation selection and invalidation identity must be trustworthy before broad fact rows arrive.
- A single canonical dependency-edge relation with indexes in both directions is preferred over duplicating forward and reverse serialized maps.
- Persist enough isolated pending/failed metadata to make recovery explainable, while making it structurally impossible for readers to see those rows as facts.
- Keep metadata columns explicit and inspectable in private tests; opaque JSON is acceptable only for small extensible details, never for identity or invalidation joins.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. Semantic fact/graph ingest remains Phase 66; summary payloads and warm reuse remain Phase 67; query services and public graph CLI remain Phases 68–69; search remains Phase 70; pruning and full crash/scale hardening remain Phase 71.

</deferred>

---

*Phase: 65-generation-manifest-and-metadata-mirroring*
*Context gathered: 2026-07-12*
