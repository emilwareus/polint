# Phase 65 Pattern Map

**Mapped:** 2026-07-12
**Purpose:** Reusable implementation patterns for complete-generation metadata persistence without creating a second identity or invalidation system.

## Likely File Map

Names for new private store modules are discretionary; the roles and boundaries are not.

| Change | Likely file | Role and data flow | Closest existing analog |
|---|---|---|---|
| Create | `crates/polint/src/analysis_kernel/store/commit_plan.rs` (or equivalent) | Own the SQL-free, deterministically sorted `StoreCommitPlan`. Project validated `InputSnapshot`, provider manifests/outputs, retained layer/query/summary metadata, `FactMeta`, validation events, dependency edges, and generation stats into owned rows. No SQLite IDs or `FactRef::run_id` participate in semantic identity. | `LayerCacheManifest::new` canonicalizes dependencies/warnings; `DependencyIndex::from_edges` sorts/deduplicates; `FactMetaStore::rows` is deterministic. |
| Create | `crates/polint/src/analysis_kernel/store/generation.rs` (or equivalent) | Execute pending/failed/complete lifecycle writes and typed active-complete reads. A private relational generation ID may join rows; the active pointer plus complete status is the only readable selector. | `connection::try_writer_lease`, `migrations::apply_migrations_in_transaction`, and layer-cache “payload first, manifest last” publication. |
| Modify | `crates/polint/src/analysis_kernel/store/mod.rs` | Keep the zero-sized facade and typed outcomes. Add plan commit/read entry points, module declarations, private failure/rebuild outcomes, and test-only fixture projections. Disabled mode must return before path creation and preferably before expensive plan materialization. | `SemanticStore::maintain`, `StoreStatus`, and `map_connection_error`. |
| Modify | `crates/polint/src/analysis_kernel/store/connection.rs` | Continue to own WAL, foreign keys, timeout, `BEGIN IMMEDIATE`, busy classification, and read-only connections. Add only store-internal transaction/query seams needed by generation code; no connection or `rusqlite` type crosses `analysis_kernel::store`. | `open_writer`, `try_initialize_writer`, `try_writer_lease`, `open_read_only`. |
| Modify | `crates/polint/src/analysis_kernel/store/migrations.rs` | Append a numbered schema migration for generation/manifest/metadata tables and validate its required shape. The v1 bootstrap table deliberately contains exactly one marker row, so the new migration must replace/update marker `1` with the new current version rather than append a second row. | `MIGRATIONS.iter().filter(version > found)`, transaction-scoped migration, strict current/future/invalid handling. |
| Modify | `crates/polint/src/analysis_kernel/store/tests.rs` | Add schema round trips, active/pending/failed selection, injected rollback, old-generation fallback, deterministic row/stat digests, mutation matrix, and no-body persistence tests. Keep raw SQL inspection inside this private module. | Existing `connection_policy`, `writer_contention`, and `recovery` test modules. |
| Modify | `crates/polint/src/analysis_kernel/mod.rs` | Preserve the post-validation insertion point, construct the plan only from finalized metadata, commit it, and retain only private status/stats in `KernelRunReport`. Remove run-local IDs from any digest that becomes durable. | Current `validate_fact_metadata` -> `finish_all_fact_meta_insertions` -> `SemanticStore::maintain` sequence. |
| Modify | `crates/polint/src/analysis_kernel/incremental/run_report.rs` | Carry the private store outcome and any deterministic generation stats needed by tests. Do not turn store metadata into a public/debug schema. | `KernelRunReport::new` and its test-only `store_status()` accessor. |
| Modify | `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` | Preserve first-class input statuses and add any missing canonical input rows, especially explicit requested-capability identity rather than only the opaque plan digest. Keep details normalized, sorted, and deduplicated. | `InputComponent::{present,absent,unsupported,setup_missing}` and sorted `provider_schemas`. |
| Modify if needed | `crates/polint/src/analysis_plan.rs` | Supply a canonical sorted snapshot/accessor for requested capabilities, setup state, and relevant analysis settings. Existing `rules()`, `capabilities()`, `setup_checks()`, and `digest()` are already crate-private sources. | `plan_digest` length-prefixes rules, capabilities, statuses, and setup checks deterministically. |
| Modify | `crates/polint/src/analysis_kernel/incremental/stats.rs` | Retain typed provider/layer run metadata through the existing provider-output path. `ProviderOutputMeta::dependency_inputs` currently represents manifest input labels, not the actual dependency graph. | `ProviderOutputMeta::new` keeps identity fields explicit and sorts dependency digests. |
| Modify | `crates/polint/src/analysis_kernel/incremental/demand.rs` | Retain the complete typed `QueryKey` (including budget digest and digest kinds) for store planning. The debug trace currently erases these into strings and includes nondeterministic duration telemetry. | `DemandQueryResult` already owns the full `QueryKey`; `DemandQueryEngine` uses a `BTreeMap<QueryKey, _>`. |
| Modify | `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` | Expose one canonical sorted edge iterator/vector for persistence, add missing canonical node vocabulary before the store mirrors it, and bump its schema only if the canonical wire shape changes. Never persist both forward and reverse maps as independently writable truth. | `from_edges` sorts/deduplicates once, then reconstructs forward/reverse indexes. |
| Modify as needed | `crates/polint/src/analysis_kernel/incremental/{digest.rs,keys.rs}` | Add stable crate-private label/parse helpers or aggregate digest kinds only where the canonical vocabulary lacks them. Store code must not use `Debug` strings or a new hash function. | `Digest::{from_parts,from_unordered}`, sorted key constructors, `serde(rename_all = "snake_case")`. |
| Modify as needed | `crates/polint/src/analysis_kernel/{provider.rs,metadata.rs,validation.rs}` | Provide canonical stable labels/round trips for provider/fact metadata and a structured validation report/event stream. Keep `FactFamily`/precision/confidence/status authoritative in the kernel, then mirror them. | `ProviderManifest::{primary_schema_label,language_scope_label,cache_policy_label}`, `FactFamily::label`, `validate_fact_metadata`. |
| Modify at metadata handoff sites | `crates/polint/src/{go/adapter.rs,ts/adapter.rs,module_graph/mod.rs,symbol_graph/mod.rs,metrics.rs}` and any provider newly given a canonical `LayerKey` | Bubble the exact key/dependency metadata from hit, miss, disabled-cache, and write-failure paths into existing typed provider results. Do not scan layer-cache files or reconstruct stale manifests after the run. | These files already construct `LayerKey`, `LayerCacheManifest`, and dependency edges before discarding all but output digest/cache stats. |
| Modify | `crates/polint/tests/public_surface_leak.rs` and, if the existing focused assertions need extension, `crates/polint/tests/cli.rs` | Add precise generation/manifest/table/type markers and negative controls while leaving `ALLOWED_PRELUDE` at 115. Generic words such as “generation”, “manifest”, or “store” are too broad to ban. | `STORE_PRIVATE_MARKERS`, per-family scanner controls, internal snapshot/layer-cache marker assertions. |
| Verify unchanged unless a test hook is required | `crates/polint/src/eval/bench/{runner.rs,gate.rs}` | Re-run the real store-enabled Phase 64 boundary with metadata writes included: first-open store bytes, isolated RSS/cold time, and byte-identical diagnostics digest. Do not relax thresholds or floors. | `evaluate_semantic_store_boundary` and ignored `real_store_enabled_measurement_passes_locked_boundary`. |

## Canonical Data Flow

```text
AnalysisPlan + loaded inputs
        -> InputSnapshot
        -> provider execution (ProviderOutputMeta + retained LayerKey/dependencies)
        -> global fact validation (structured events + diagnostics)
        -> FactMetaStore::finish_all_insertions
        -> StoreCommitPlan::from_validated_run (sort/dedup/count/digest/validate)
        -> SemanticStore writer lease
        -> isolated pending generation
        -> one IMMEDIATE transaction writes required rows/events/stats,
           marks complete, and changes the active pointer
        -> read-only lookup joins active pointer to status = complete
        -> private StoreStatus/StoreStats on KernelRunReport
```

The complete-and-activate transaction is the visibility boundary. If it rolls back, readers continue to see the previous active complete generation. A best-effort follow-up may mark the isolated pending row failed; it cannot make that row readable.

## Source Vocabulary to Mirror

| Canonical source | First-class store projection | Important constraint |
|---|---|---|
| `InputSnapshot` | Snapshot schema; file path/language/source digest/size/hint; grouped component name/status/digest; normalized detail child rows; provider-schema snapshot rows | Never persist source text, absolute paths, or infer `Absent`/`Unsupported`/`SetupMissing` from a missing row. |
| `ProviderManifest` | Provider ID/version/kind/language/cache/precision and manifest digest; sorted input/output/schema child rows | Reuse manifest label methods and `ProviderSchemaSnapshot.provider_manifest_digest`; no store-only provider enum. |
| `ProviderOutputMeta` | Provider-generation row with output digest, precision, validation, per-provider stats, and actual retained dependency metadata | Its current `dependency_inputs` are digests of declared input names, not a substitute for concrete invalidation endpoints. |
| `LayerKey` | Scalar provider/schema/parameter/lifecycle/config/toolchain fields plus sorted input/upstream/extension digest child rows | Key constructors already sort variable lists. Do not serialize the whole key into one opaque identity blob or use SQLite row IDs as key identity. |
| `SummaryKey` / `QueryKey` | Stable scalar columns and sorted dependency/layer digest child rows; query budget and precision columns | Empty production summary rows are valid in this phase; schema and fixtures still prove the vocabulary. Query durations are telemetry, never identity. |
| `FactMetaStore` | `(generation, fact_family, stable_key)` semantic row with producer/layer, precision, confidence, validation, and existing payload digest | `FactRef::run_id` is an in-run lookup handle only. Sort and deduplicate by stable semantic fields before persistence. |
| `DependencyIndex` | One sorted/deduplicated edge relation with endpoint kind/key, dependency kind, and required shape; indexes from both endpoints | Rebuild forward/reverse traversal from one edge table. Never write serialized forward and reverse copies independently. |
| Validation result | Structured event kind/status/count/digest plus any deterministic provider/schema reference | Rendered diagnostics are not the validation event schema. Store activation requires explicit successful events. |
| Generation stats | Counts and canonical digests for providers, layers, metadata, dependencies, and validation events; size counters | Wall-clock timestamps/durations may be telemetry columns but must be excluded from normalized digests and selection. |

## Concrete Existing Patterns

### Transaction and facade boundary

The connection layer already serializes initialization and writes with an immediate transaction:

```rust
let transaction = writer
    .connection
    .transaction_with_behavior(TransactionBehavior::Immediate)?;
apply_migrations_in_transaction(&transaction)?;
transaction.commit()?;
```

Generation commit should reuse that connection policy. A sibling store module may receive a store-private transaction seam, but providers, rules, `runner`, CLI, SDK, and `KernelRunReport` must never receive a connection.

The disabled fast path is already structural:

```rust
if !config.is_enabled() {
    return StoreStatus::Disabled;
}
```

Keep this before directory creation/opening and before avoidable row cloning or digest aggregation.

### Migration discipline

`apply_migrations_in_transaction` performs four useful steps: read `PRAGMA user_version`, reject future versions before mutation, run only higher numbered migrations, and validate the current invariant before commit. Phase 65 should extend the invariant to required generation metadata tables/columns/indexes. Tests should retain all current cases and add v1 -> current preservation plus a failing migration rollback fixture.

### Deterministic construction

The reusable normalization pattern is consistent across the kernel:

```rust
edges.sort();
edges.dedup();
```

`InputSnapshot` sorts files/providers/components, `LayerKey` sorts every variable digest list, `DependencyIndex` uses `BTreeMap` and sorted edges, and `FactMetaStore::rows()` iterates by `FactFamily` then run-local ID. The store plan must re-key fact metadata by stable semantic identity rather than copy that final run-ID ordering.

`Digest::from_parts` length-prefixes values and `Digest::from_unordered` sorts typed digests. Use those canonical helpers for normalized plan/stat digests. Do not call `cache::stable_hash`, hash SQL text, hash insertion order, or introduce a store-local digest function.

### Explicit absence and unavailable states

`InputComponent` already distinguishes:

```rust
Present | Absent | Unsupported | SetupMissing
```

Each state has an explicit typed digest and normalized detail. Store rows should round-trip all four values. The same rule applies to fact precision/status and provider validation: absence of a row cannot mean a known unavailable state.

### Dependency invalidation and preserved hits

`InvalidationPlan::from_change_set` starts with deterministic `Reuse` actions, walks `DependencyIndex.reverse`, and replaces only affected nodes. Its rule test is the exact D-14 analog: a rule change recomputes a layer only when that rule digest is in the layer key; an unrelated syntax layer remains `Reuse`.

The Phase 65 matrix should reuse this machinery after store round-trip. For every required input class, mutate the dependency that is actually referenced (must invalidate) and a sibling/unreferenced value (must preserve hit). This proves the persisted graph rather than a parallel store-only invalidator.

### Publication ordering

The layer cache writes the payload before publishing its manifest. SQLite strengthens the same idea: write all generation metadata and required validation events before the complete marker and active pointer, with complete+active in the same transaction. Readability must be an explicit join on the active pointer and `status = complete`, never `MAX(generation_id)`, timestamp, row insertion order, or partial row presence.

### Kernel integration

The current authoritative order is:

```rust
let validation_diagnostics = validate_fact_metadata(&db, manifests);
diagnostics.extend(validation_diagnostics);
db.finish_all_fact_meta_insertions();
let store_status = SemanticStore::maintain(&store_config);
```

Replace only the final maintenance action with validated plan construction/commit. Store validation may reject the plan, but it must not alter provider execution, fact validation, policy diagnostics, capability support, or exit behavior.

## Gaps the Planner Must Close Explicitly

1. **No run-level layer/dependency accumulator exists.** Cache-capable providers build exact `LayerKey`/`LayerCacheManifest` values, then their result structs retain only diagnostics, cache stats, and output digest. Capture typed metadata on hit, miss, disabled-cache, and failed-write paths; scanning `.polint/cache/layers` would mix stale or unrelated runs.
2. **The query trace is lossy for identity.** `DemandQueryTraceEntry` stores digest values as strings, omits the budget digest, and includes compute duration. Retain `DemandQueryResult.query_key` or an equivalent deterministic typed row for commit planning.
3. **Requested capabilities are only implicit in the input snapshot.** `InputSnapshot.rules` contains the rule digest and aggregate plan digest; `AnalysisPlan` separately exposes sorted planned rules/capabilities/setup checks. Add canonical explicit capability rows before mirroring them.
4. **Validation has diagnostics, not durable events.** `validate_fact_metadata` returns `Vec<Diagnostic>`. Introduce a structured kernel validation report/event vocabulary first; do not parse diagnostic messages inside the store.
5. **Provider output digests currently include `FactRef::run_id`.** `provider_output_summary_parts` emits `run_id=...`. Any digest persisted as semantic provider identity must instead use fact family, stable key, payload digest, producer/layer, precision/confidence, and validation fields in canonical order.
6. **Several canonical enums lack stable SQL codecs.** `DigestKind::as_str` is private to `digest.rs`; `LayerKind`, precision types, input status, and fact metadata types do not all expose stable label/parse pairs. Add codecs beside the canonical types rather than matching `Debug` strings in store code.
7. **`DependencyIndex` exposes two maps but no canonical all-edge view.** Add one sorted/deduplicated edge projection and persist it once. Do not concatenate forward and reverse values, which would duplicate every edge.
8. **`CacheNode::Input(String)` already has provider-authored canonical prefixes.** Mirror existing values without store parsing where possible. If packages/projects/capabilities/budgets/models need stronger typing, extend `CacheNode`/`DependencyKind` first and update exhaustive invalidation/quarantine matches plus the dependency-index schema.
9. **Schema v1 enforces one migration marker.** A naive v2 `INSERT` leaves two rows and is rejected by `validate_current_schema`; the migration must advance the marker atomically.

## Deterministic and Recovery Test Matrix

### Store lifecycle

- Commit generation A; typed reader returns A only when the manifest points to A and A is complete.
- Create pending B; reader still returns A.
- Inject failure after each logical write group for B; its generation-scoped rows roll back, A stays readable, and any follow-up failed event remains isolated.
- Commit B successfully; required rows/events, complete marker, and active pointer appear atomically.
- Pointing active selection at pending/failed/missing or structurally invalid metadata never yields mixed rows; return the typed rebuild/no-readable outcome required by the integrity condition.
- Busy writer, future schema, invalid schema, corrupt file, and unsafe path retain the Phase 64 controlled outcomes and preserve prior bytes/state.

### Canonical identity and ordering

- Reverse/shuffle provider manifests/outputs, layer metadata, dependency edges, input details, and fact insertion/run IDs; assert identical canonical plan rows, aggregate digests, and stats.
- Reopen and round-trip every canonical status/enum/digest kind through typed readers.
- Assert explicit `ORDER BY` on test projections and that no assertion depends on `rowid`, insertion order, timestamps, or autoincrement values.
- Assert stored metadata contains no source text, absolute/temp paths, AST/MIR/CFG bodies, or semantic fact payload columns.

### META-04 mutation table

At minimum include paired invalidate/preserve cases for source file, package/project, provider manifest/version/schema, requested capability/analysis setting, Go and TS/JS lifecycle/tool invocation, config, upstream layer, summary dependency, query parameters/options, budget profile, extension code/declared input, and model digest. Include explicit present/absent/unsupported/setup-missing transitions and stable edge rows under shuffled construction order.

## Phase 64 Regression Gates to Preserve

- `store::tests`: connection policy, writer contention, schema recovery, unsafe-path protection, and disabled zero-I/O behavior.
- `analysis_kernel::tests::semantic_store`: maintenance/commit remains after finalized fact metadata.
- `analysis_kernel::tests::semantic_store_check_parity::all_store_modes_preserve_byte_identical_json_and_exit_semantics`: enabled, corrupt, future, invalid, and busy outcomes remain byte-identical to disabled public JSON and exit status.
- `crates/polint/tests/public_surface_leak.rs`: outside-consumer prelude probe, exact 115-name allowlist, public docs/output/generated-skill scans, and marker negative controls.
- Existing `crates/polint/tests/cli.rs` internal `InputSnapshot`/key/layer-cache vocabulary assertions.
- `eval::bench::runner::tests::semantic_store::isolated_modes_report_real_store_bytes_and_equal_diagnostics_digest`.
- Dedicated serialized ignored gate `eval::bench::gate::tests::semantic_store_boundary::real_store_enabled_measurement_passes_locked_boundary`; keep the locked +20% RSS, +25% cold-time, 16 MiB RSS floor, 50 ms cold floor, and diagnostics parity.
- `make lint` and `cargo test --workspace --all-features --locked`; no new public flag/config/SDK export, no `ALLOWED_PRELUDE` change, and no generated-skill vocabulary drift.

