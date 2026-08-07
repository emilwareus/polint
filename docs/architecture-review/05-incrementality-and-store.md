# 05 — Incrementality, Caching, Persistence, and the Query Engine

**Scope:** `analysis_kernel/incremental/`, `analysis_kernel/store/`, `src/cache/`, `analysis_kernel/{mod,provider,validation,debug}.rs`, `src/fs/`, `src/analysis/demand/`.
**Verdict up front:** polint has a *content-addressed memoization cache*, not an incremental engine. The vocabulary of an incremental query engine (query keys, demand engine, dependency index, invalidation planner, precision tiers, quarantine) has been built — roughly 11,500 LOC of it — but the load-bearing parts are not wired to anything. `AnalysisKernel::run` is a hardcoded straight-line pipeline of 23 whole-program passes over one 132-field mutable struct. The SQLite store ships in every binary and contains exactly one table with one column and one row.

The determinism work, by contrast, is genuinely good and should be protected.

---

## (a) How incrementality actually works today

### A1. The unit of reuse is a whole layer, keyed by every input at once

There is exactly one real reuse mechanism, and it is a content-addressed lookup, not an incremental computation.

`LayerKey` (`crates/polint/src/analysis_kernel/incremental/keys.rs:67`) identifies a provider's *entire output* for the *entire repo*:

```rust
pub(crate) struct LayerKey {
    layer_kind, provider_id, provider_version, schema_version,
    parameter_digest, lifecycle_digest, config_digest, toolchain_digest,
    input_digests: Arc<Vec<Digest>>,          // one digest per source file, for ALL files
    dependency_layer_digests: Arc<Vec<Digest>>,
    extension_digests: Arc<Vec<Digest>>,
}
```

`input_digests` for the Go syntax layer is literally the digest of every Go file in the repo (`crates/polint/src/go/adapter.rs:195-201`). Change one byte in one file and the whole-layer key changes.

The lookup is a file read: `manifest_path` = `stable_hash(serde_json(key))` (`crates/polint/src/analysis_kernel/incremental/layer_cache.rs:420-425`), pointing at `.polint/cache/layers/manifests/<hash>.json`, which names a content-addressed blob in `.polint/cache/layers/blobs/<payload-digest>.json`.

There is *one* softening mechanism, and it is the only thing in the codebase that deserves the word "invalidation": `invalidation_allows_manifest_reuse` (`layer_cache.rs:657-668`) will accept a manifest whose key is **not** equal to the requested key, if a synthesized `ChangeSet` propagated through the manifest's dependency index yields only `Reuse` actions. So a config-digest delta only invalidates edges of `DependencyKind::Config`, a toolchain delta only `Toolchain`/`ToolInvocation` (`layer_cache.rs:711-779`). That is real, and it is well-built.

But note what it is not: it compares **two cache keys**, not two versions of the repo. It has no idea what the user edited.

### A2. There is no change detection anywhere

`ChangeSet` (`incremental/change_set.rs:33`) has exactly one constructor, `from_rows`. Grep for production callers: the only one is `manifest_change_set(manifest, key)` (`layer_cache.rs:692`), which synthesizes rows by diffing a *cached key* against a *recomputed key*.

Nothing reads mtimes to find edits. Nothing consults git. Nothing tracks a previous run's state. The engine's model is: read everything, hash everything, recompute the key, see if the key matches. The `ChangeKind` taxonomy (`ContentOnly`, `SyntaxShape`, `ImportShape`, `PublicApiShape`, …) — the thing the research doc says is "the difference between useful interactive analysis and whole-repo churn" — is populated only from key-field deltas, and `input_digests` deltas map to `ChangeKind::Unknown` (`layer_cache.rs:741-752`), which fans out to `Recompute` on every dependent (`incremental/invalidation.rs:259-262`).

So in practice: **any source edit recomputes every downstream layer.** The shape-digest hierarchy the research demanded does not exist. There is one digest per file — its content hash — and nothing else.

### A3. The demand-query engine is dead code. Twice.

Two separate demand-query scaffolds exist and neither is wired.

**Scaffold 1 — `crates/polint/src/analysis/demand/` (2,188 LOC).** Declares 7 `QueryKind`s, `QueryBudget`, `QueryContext`, `QueryStatus`, SCC helpers. Every `QueryKind::` construction in the crate is inside a `#[cfg(test)]` block (verified: `context.rs` tests start at :264, `trace.rs` at :213, `query.rs` at :198; all uses are past those lines). `demand_query_key` (`analysis/demand/query.rs:181`) has **zero non-test callers**. The only reference to the module from outside its own directory is a test asserting its names don't leak into the public surface (`analysis_kernel/provider.rs:1818-1821`).

**Scaffold 2 — `analysis_kernel/incremental/demand.rs`.** Carries `#![cfg_attr(not(test), expect(dead_code, reason = "Demand query engine infrastructure is established before Plan 04 wires real demand-driven consumers."))]` at line 1-7. It has exactly one production consumer: SCC closure.

And that consumer does not use it as a cache. `close_summaries_by_scc` (`analysis/summaries/closure.rs:107`) iterates **every** SCC unconditionally:

```rust
for scc in &schedule.sccs {
    ...
    let (scc_summaries, scc_events, iterations, budget_exceeded) = process_recursive_scc(db, scc, config);
    ...
    record_scc_demand_query(demand_engine, scc, &scc_digest, iterations, was_backdated);   // :145
}
```

`record_scc_demand_query` runs *after* the work. `DemandQueryEngine::lookup` has **zero production callers** anywhere in the crate. The engine is a write-only trace log.

The design bug that proves this was never intended to short-circuit: the query key's `budget_digest` is derived from `iterations` (`closure.rs:719-724`), which is an *output* of the computation. You cannot look up a key you can only construct after doing the work. The key also has `layer_digests: Vec::new()` (`closure.rs:714`) — it doesn't record its own inputs, so it could never be validated across runs either.

### A4. Granularity summary

| Layer | Nominal granularity | Actual reuse granularity |
|---|---|---|
| `polint.source` | per-file | none (always re-read) |
| `polint.go.syntax` / `ts.syntax` | per-file JSON cache **and** whole-layer cache | per-file, but poisoned by rule digest (§C2) |
| 18 `WholeRepoDerived` providers | whole program | whole program; `db.replace_semantic_mir(output)` etc. wholesale replaces the fact family |
| `polint.direct_summaries` | per-callable `SummaryKey` exists (`keys.rs:92`) | never persisted; recomputed every run |
| `polint.metrics` | whole repo | whole repo |

The only genuinely function-granular key type, `SummaryKey`, is constructed nowhere in production. `DiagnosticKey` likewise.

### A5. Scheduling: hardcoded, not a graph

`AnalysisKernel::run` (`analysis_kernel/mod.rs:92-968`) is an ~880-line straight-line function. 23 providers called in a fixed statement order, each taking `&mut db`, each threading the previous provider's `Digest` in by hand as a positional argument.

`ProviderManifest` declares `inputs: &'static [&'static str]` and `outputs` (`provider.rs:1-11`) — a complete dependency DAG, in data. **Nothing topologically sorts it.** Its only consumers are cache-digest construction (`incremental/run_report.rs:118-125`, `incremental/input_snapshot.rs:294-330`) and a test comparing the manifest order to a third hardcoded literal list (`provider.rs:935-964`). The manifest DAG is decorative.

Demand pruning exists but is coarse. Five boolean gates (`mod.rs:97-114`), of which **three are the same predicate** — `SEMANTIC_PIPELINE_TRIGGER_CAPABILITIES`, `CFG_CALL_PIPELINE_TRIGGER_CAPABILITIES`, and `FULL_REFINEMENT_PIPELINE_TRIGGER_CAPABILITIES` are all byte-identical `&["calls", "control_flow", "dataflow"]` (`mod.rs:43, 47, 55`). So there are three real tiers, not five. Consequence: a rule requesting `control_flow` — a *same-function* property — pulls in points-to, the constraint solver, interprocedural summaries, entrypoints, and reachability.

Providers run strictly sequentially; `&mut AnalysisDb` forbids otherwise. Rayon appears in 4 places only: file reads (`fs/mod.rs:133`), Go parse (`go/adapter.rs:267`), TS parse (`ts/adapter.rs:306`), and rule evaluation *after* the kernel (`core/mod.rs:7736`). The 18 derived providers — including the most expensive ones — are single-threaded.

### A6. Determinism: this part is done right

Credit where due. This is the strongest area of the subsystem.

- **Zero `HashMap`/`HashSet` in the entire cache, digest, and incremental path.** The whole crate has 4 `HashMap` uses (`analysis/identity/categorize.rs:232`, `analysis_kernel/metadata.rs:357`, `sdk/scope.rs:52`, `sdk/facts.rs:1938`), all point-lookup only, none iterated into a digest or into output. `metadata.rs:357`'s inner `HashMap` is cleared at `finish_all_insertions` (`metadata.rs:426`).
- **No `DefaultHasher`/`RandomState` anywhere** — no ASLR/seed dependence.
- Digest construction is length-prefixed on both label and value with a `0xfe` separator (`incremental/digest.rs:138-147`), so `"ab"+"c" ≠ "a"+"bc"`, and the `DigestKind` tag is mixed first so domains cannot collide.
- `Digest::from_unordered` sorts before hashing (`digest.rs:52`); `LayerKey::new` sorts all three digest vectors (`keys.rs:138-140`); manifest dependencies and warnings are sorted+deduped (`layer_cache.rs:58-60`).
- Config hashing deliberately avoids serde-JSON "to avoid hidden serde-json key ordering quirks" (`cache/keys.rs:14-17`).
- **No mtime, `SystemTime`, absolute path, `home_dir`, or env value reaches any key.** `FileSnapshot.mtime_hint_present` (`input_snapshot.rs:38`) is a bool existence probe, never a timestamp, and is read nowhere outside a test.
- Config/rule options *are* folded into digests, as AGENTS.md:162-164 requires.

Two residual determinism notes, neither a bug today:
1. `manifest_path` hashes `serde_json::to_string(key)`, so cache identity depends on serde's field emission order for `LayerKey` — i.e. struct declaration order in `keys.rs:67-79`. Reordering fields silently rekeys the entire layer cache with no schema bump.
2. **Every hash in the system is FNV-1a 64-bit**, implemented three times independently (`cache/mod.rs:773`, `incremental/digest.rs:134-174`, `diagnostics/mod.rs:2269`). There is no blake3 or sha2 in the workspace despite `.planning/REQUIREMENTS.md:38` locking "Add `blake3` for content-addressed summary and payload identities." See §C5.

### A7. Observability: there is none in production

`analysis_kernel/debug.rs` is 3,960 lines and is declared `#[cfg(test)] mod debug;` (`analysis_kernel/mod.rs:8`), with `#![cfg(test)]` at `debug.rs:1`. It does not compile into a release binary. It produces a rich `MetadataDebugReport` (files, imports, symbols, references, semantic, MIR, CFG, calls, domains, summaries, dataflow, evidence, entrypoints, refined calls, extensions, SCC schedule, demand queries) — for snapshot tests only.

What production actually has: `tracing::info!` phase markers (`mod.rs:127, 200, 218, …`) and `KernelRunReport`, whose accessors are `#[cfg(test)]`-gated (`incremental/run_report.rs:62-65`) and whose field carries `expect(dead_code, reason = "The crate-private run report is consumed by internal tests and eval fixtures before a public surface exists.")` (`mod.rs:77-83`). `CacheStats` is collected per provider and never surfaced to a user.

So: no way for a user or an agent to ask "why did my cache miss?" The research doc's promotion gate — "benchmark output reports hit/miss/recompute/quarantine counts" — is unmet outside tests.

---

## (b) The memory / scale failure mode

### B1. Correction to the premise

The brief says polint "holds ASTs for all files simultaneously." **That is false.** Both parsers are strictly per-file and scoped: tree-sitter trees are function-local in `parse_go_file` (`go/adapter.rs:446-471`); all 17 oxc `Allocator::default()` sites are function-local, with an explicit invariant comment at `analysis/calls/js_points_to/provider.rs:60-70` ("`allocator` drops here; the harvester retains no AST references"). Greps for `Arc<Tree>` / `Vec<Tree>` / `tree_sitter::Tree` in field position return nothing, and there are regression tests defending it. Peak AST memory is bounded by (threads × one arena), which is correct.

The 30GB OOM is a **documented past incident that was already mitigated**. `.planning/REQUIREMENTS.md:28`:

> "Store ingest must not resurrect the eager whole-repo pipeline or whole-repo source loading that previously caused 30GB+ OOM (fixed via capability gating and rule-scoped discovery; current baseline ~1GB peak on the reference monorepo)."

The real failure mode is different, and worse in the long run: the mitigation is a *bypass*, not a fix, and it is one config change away from disengaging.

### B2. All source is read into one `Vec` before anything is processed

`crates/polint/src/fs/mod.rs:132-139`:

```rust
let loaded = discovered
    .into_par_iter()
    .map(|file| {
        let source = fs::read_to_string(&file.path)
            .with_context(|| format!("failed to read {}", file.path.display()))?;
        Ok((file, source))
    })
    .collect::<Result<Vec<_>>>()?;
```

`collect::<Result<Vec<_>>>()` is a hard barrier. Every file's contents exist as an owned `String` in one `Vec` before a single byte reaches the DB. Then `fs/mod.rs:143-146` moves them into `AnalysisDb`, where `add_file` does `Arc::from(source)` (`core/mod.rs:986`) — a copy, so there is transient 2× per file during ingest.

**There is no size cap on this read.** The crate *has* a bounded-read helper — `repo_fs.rs:179-192` returns `RepoFileReadError::TooLarge` — and uses it for config (1 MiB), analysis cache (16 MiB), layer payloads (64 MiB), lockfiles (16 MiB), baselines, and extension sources. `fs/mod.rs:135` calls bare `fs::read_to_string`. One 2 GB generated bundle that survives the glob filter is read whole into RAM. The only defenses are advisory, path-based, user-overridable globs (`config/mod.rs:424-430`).

Rayon has no bounded concurrency: there is no `ThreadPoolBuilder`, `num_threads`, or `available_parallelism` anywhere in the crate, so parallel read and parallel parse widths are unbounded with respect to memory.

### B3. `AnalysisDb` is a 132-field whole-program fact table that is never shrunk

`core/mod.rs:658-825`. 132 fields, essentially all `Vec<...Fact>` or `BTreeMap` indexes, plus 9 `Option<...Store>` whole-program stores. `files: Vec<SourceFile>` (`core/mod.rs:659`) holds `source: Arc<str>` for every file (`core/mod.rs:262`).

The corpus is live from load until process exit. `files` is never cleared — the only `.clear()` calls in `core/mod.rs` are on `refined_call_edges` (:1350), `summary_facts`/`summary_events` (:1569-1582), and symbol/scope index maps (:3418-3423, :3478-3484). The DB is returned out of the kernel as `KernelOutput.db`, used for rules (`runner/mod.rs:425`), ignore processing (`runner/mod.rs:234`), and rendering (`runner/mod.rs:239`).

23 providers each take `&mut db` and each perform a complete pass over the whole corpus (`mod.rs:191, 209, 233, 263, 294, 323, 355, 382, …, 923`). **No phase streams.** Each is read-whole-db → compute → write-all-rows-back. Nothing is dropped between phases; the DB only grows. Derived providers replace whole fact families wholesale (e.g. `db.replace_semantic_mir(output)`).

Additional transient doubling: `parse_go_syntax_layer_payload` / `parse_ts_syntax_layer_payload` build a `SyntaxLayerPayload` holding facts for **all** files, then `restore_syntax_layer_payload` copies them into the DB (`go/adapter.rs:163`, `ts/adapter.rs:329`) — syntax facts exist twice at peak. (`restore_syntax_layer_payload` also does `db.files().iter().find(...)` per file — O(n²) — `go/adapter.rs:481`.)

**Zero eviction infrastructure exists.** No `lru`, `moka`, `dashmap`, or `Semaphore` — not even as dependencies. Greps for `evict`, `memory_limit`, `max_memory`, `spill`, `max_concurrency` return no in-memory-management hits; `evict` hits are all disk cache-file unlinking. The `budget` hits are all *algorithmic* search budgets (`PointsToBudget { max_steps: 2_000_000 }` at `analysis/calls/js_points_to/solver.rs:219`, `DEFAULT_MAX_NODES: 10_000` at `analysis/demand/query.rs:71`) — real protection against solver blow-up, no protection against corpus size.

### B4. Unconditional per-fact metadata: the likely dominant residual cost

`core/mod.rs:4257`:

```rust
fn record_fact_meta(&mut self, family: FactFamily, run_id: u64, meta: FactMeta) {
    let reference = FactRef::new(family, run_id);
    let _insert = self.fact_meta.insert(reference, meta);
```

Called on **every** fact push — `push_source_file` (:1027), `push_package` (:1037), `push_function` (:1046), and so on for all families. Not `cfg`-gated, not opt-in.

`FactMeta` (`analysis_kernel/metadata.rs:228-237`) carries `stable_key: String` and `payload_digest: String`. `FactMetaStore::insert` (`metadata.rs:362-375`) *also* writes into `stable_key_owners: BTreeMap<FactFamily, HashMap<String, StableKeyOwner>>` (`metadata.rs:357`), keyed by a **second copy** of `stable_key`, with `StableKeyOwner` holding a **second copy** of `payload_digest`.

That is ~4 retained heap `String` allocations per fact, unbounded, for the whole run. On a repo producing millions of symbol/reference/call-site/dataflow facts this plausibly exceeds the source corpus footprint. *Flagged as a strong hypothesis, not a measured number* — it should be profiled before it is optimized.

### B5. The mitigation is fragile

Two gates got polint from 30GB to ~1GB:

1. **Capability gating** (`mod.rs:97-114`) — skip whole provider slices.
2. **Rule-scoped discovery** (`mod.rs:122-126`) — narrow the file walk to the union of enabled rules' `files` globs.

Both disengage easily. `rule_scope_globset` (`mod.rs:1048-1061`) returns `None` — full workspace discovery — if `rules.is_empty()` **or if any single enabled rule has an empty `files` list**:

```rust
for rule in rules {
    if rule.files.is_empty() {
        return None;
    }
```

And scoping is disabled outright whenever any cross-file capability is requested (`mod.rs:120-122`). So one unscoped rule, or one rule requesting `symbols`, reverts the whole run to whole-repo loading. The OOM path is *reachable*, not *removed*.

### B6. Un-cacheable fixed costs on every run

Even on a 100% cache hit, two full-corpus passes always run:

- **`validate_fact_metadata`** (`analysis_kernel/mod.rs:941-943` → `validation.rs:41`) runs unconditionally in production — no feature flag, no `cfg(debug_assertions)`, no config gate. `IdSets::from_db` (`validation.rs:3224-3269`) materializes ~24 `BTreeSet`s over every fact family up front; `TypeValueAliasIdSets::from_db` (`validation.rs:530`) is a second such pass; then 18 sub-validators each linear-scan with `BTreeSet` lookups. Failures surface as user-visible `polint/internal` errors (`validation.rs:5268`). This is an assertion layer promoted to error output, and it is the largest un-cacheable cost in a warm run.
- **`evict_stale_manifests_for_key`** (`layer_cache.rs:536-565`) does a full `manifests/` directory scan, deserializing every manifest, **on every cache miss** (`layer_cache.rs:246`). Cold-path cost that degrades as the cache grows.

### B7. Cache growth is unbounded

Layer-cache blobs are **never garbage collected**. Manifests are evicted individually (`layer_cache.rs:257, 261, 266, 301, 562`) without touching their blobs. Because blobs are content-addressed and manifests are `LayerKey`-addressed, every changed input mints a new blob and abandons the old one. There is no TTL, no `max_entries`, no size cap, no background GC. `.polint/cache/layers/blobs/` grows monotonically until a human runs `polint cache prune --max-size-mb` (`cli/mod.rs:2606` hard-errors if neither bound is given). That prune is mtime+size based across whole categories and oblivious to manifest↔blob referential integrity.

---

## (c) Research recommendations vs. what was built

The research is unusually good — specific, falsifiable, with explicit anti-goals. The implementation tracks it structurally and diverges on almost every load-bearing detail.

### C1. Scorecard

| Research recommendation | Status |
|---|---|
| Bespoke layered engine, not Salsa (`research/incremental-query-engine/FINAL-REPORT.md:5`) | **Followed.** No salsa dep. |
| Phase 0: `Digest`, `DigestKind`, `CacheStats`, `ProviderOutputMeta` | **Built** (`incremental/{digest,stats}.rs`) |
| Phase 1: `InputSnapshot` | **Built** (`incremental/input_snapshot.rs`, 1,913 LOC) |
| Phase 2: typed keys `LayerKey`/`QueryKey`/`SummaryKey`/`DiagnosticKey` | **Built** (`keys.rs`). Only `LayerKey` is used. |
| Phase 3: persistent layer cache | **Built and live** for syntax, module graph, symbol graph, metrics |
| Phase 4: `DependencyIndex` fwd+reverse | **Built** (`dependency_index.rs`); only ever constructed from a single manifest's own edges (`layer_cache.rs:675`), never repo-wide |
| Phase 5: `ChangeSet` with 14 `ChangeKind`s | **Built, but fed only by key-diffing** — no edit detection (§A2) |
| Phase 6: invalidation planner | **Built and live** (`layer_cache.rs:657`) |
| Phase 7: demand query engine | **Scaffolded twice, wired zero times** (§A3) |
| Phase 8: summary SCC cache + backdating | **Backdating only.** Digest cache exists; no per-summary persistence |
| Phase 9: extension quarantine | `quarantine.rs` is `expect(dead_code)`, in-memory, never persisted |
| Phase 10-12: diagnostic cache, watch/daemon, relation engine | Correctly deferred |
| SQLite store via rusqlite | **Skeleton only** (§C3) |
| Multi-level shape digests (text / syntax / import-export / public-signature) | **Not built.** One digest per file: its content hash |
| "Do not cache parser facts with rule hashes long term" | **Violated** (§C2) |
| "Add blake3 for content-addressed identities" (`.planning/REQUIREMENTS.md:38`) | **Not done.** FNV-1a 64-bit everywhere (§C5) |
| Eviction / spilling / memory budget | **Not designed in research either** — genuine shared gap (§C6) |
| Remote cache | Correctly out of scope (§C7) |

### C2. The parser cache is keyed on the rule digest — a named anti-goal

`research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md:369`: *"Do not cache parser facts with rule hashes long term."*
`research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md:509-522` promotion gate: *"rule option edits do not invalidate parser facts."*

Implementation, `cache/mod.rs:42-58`:

```rust
pub(crate) fn for_file(relative_path, content_hash, config_hash, rule_hash, plan_hash, schema) -> Self {
    Self {
        file_hash: stable_hash(&[relative_path, content_hash]),
        config_hash: config_hash.to_string(),
        rule_hash: rule_hash.to_string(),
        plan_hash: plan_hash.to_string(),
        ...
```

Both language adapters use it for parsed facts: `go/adapter.rs:368-374`, `ts/adapter.rs:407-413`.

**Practical impact:** change one rule option, or enable/disable one rule (which moves `plan_hash`), and every file's parse cache misses. Full repo re-parse. This is the single highest-leverage incrementality bug in the codebase and it is a ~20-line fix.

### C3. The SQLite store is inert

Research locked SQLite with a rich schema — `store_manifest`, `schema_migrations`, `provider_generation`, `input_snapshot`, `layer_entry`, `layer_dependency`, `validation_event`, `package`, `source_file`, `module`, `symbol`, `definition`, `reference`, `node`, `edge`, `summary_manifest`, … (`research/local-semantic-store/RECOMMENDED_IMPLEMENTATION.md:99-152`).

What exists (`analysis_kernel/store/migrations.rs:8-17`) is the complete schema:

```rust
pub(super) const CURRENT_SCHEMA_VERSION: i32 = 1;
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: "CREATE TABLE _polint_schema_migrations (\
              version INTEGER PRIMARY KEY CHECK (version > 0)\
          );\
          INSERT INTO _polint_schema_migrations (version) VALUES (1);",
}];
```

One table. One column. One row, containing the integer `1`. No fact, diagnostic, symbol, or file table.

Worse, it **cannot be enabled in a production build**. `Cache::semantic_store_enabled` is hardcoded `false` at construction (`cache/mod.rs:108`) and the only mutator is `#[cfg(test)] with_semantic_store_enabled_for_test` (`cache/mod.rs:144-148`). There is no CLI flag, no config key, no env var. `SemanticStore::maintain` short-circuits at `store/mod.rs:91-93`. And its result is never read: `KernelRunReport::store_status()` is `#[cfg(test)]`-gated (`run_report.rs:62-65`).

So `rusqlite` with a **bundled SQLite C build** compiles into every shipped binary to service dead code.

To be fair, this is where the roadmap says it should be. `.planning/ROADMAP.md:24` marks Phase 64 ("Store Foundation and Boundary Proof… zero-cost disabled path, no-leak gates") complete; Phase 65 (metadata mirroring) and 66 (streaming ingest) are open. The migration/locking/recovery contract that *was* built is solid: WAL enforced (`connection.rs:81-87`), `BEGIN IMMEDIATE` single-writer lease (`connection.rs:105-118`), forward-incompatible refusal on future schema with a byte-identical-DB assertion (`migrations.rs:54-59, 228-252`), strict invariant validation (`migrations.rs:99-131`).

The honest read: the *hardest* part (durability semantics) is done and the *valuable* part (data) is not, and Phase 65 was already abandoned once for being oversized — `research/local-semantic-store/decisions/DECISIONS.md:102-105`: *"the abandoned Phase 65 implementation combined nineteen plans and expanded beyond 85,000 added lines."*

The privacy discipline, however, is exemplary and worth preserving: a dedicated leak gate with 13 markers (`tests/public_surface_leak.rs:191-205`) plus a meta-test verifying the scanner detects each marker family (`:541-560`), plus a compile-time assertion confining `rusqlite::Transaction` to the store module (`migrations.rs:330-333`).

### C4. `provider_version` is the crate version

`provider.rs:14-16`:

```rust
pub(crate) fn provider_version(&self) -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

Every provider returns the same string. Consequences both ways:
- **Every crate version bump invalidates every provider's layer cache globally.** Combined with `CACHE_VERSION = concat!("polint-cache-v1:", env!("CARGO_PKG_VERSION"))` (`cache/mod.rs:9`), *every polint release cold-starts every user.*
- Conversely, changing a provider's *algorithm* without touching its `schema_versions` label does **not** change the key. Correctness rests entirely on manual `schema_versions` discipline (`provider.rs:140-253`).

`mod.rs:798-806` documents this sharp edge in a comment — an explicit `FIX 4` recording that the `go.semantic` digest had to be manually folded into the solver's key because the docstring's claim that "any upstream change invalidates the solver cache" was simply false until someone noticed.

Related: `created_by_polint` is written into every manifest (`layer_cache.rs:68`) and **never read** — two grep hits, both writes. So layer entries silently survive upgrades that the JSON tier correctly invalidates.

### C5. FNV-1a 64-bit is the only hash in the system

No blake3, no sha2, no sha256 anywhere in `crates/` — despite `.planning/REQUIREMENTS.md:38` locking blake3 as a technology decision, and `research/local-semantic-store/RECOMMENDED_IMPLEMENTATION.md` specifying content-addressed summary payloads.

`payload_digest_for_bytes` (`layer_cache.rs:615-622`) is the layer cache's **sole integrity check** on blob contents, and blob filenames are that same 64-bit value. `content_hash` (`core/mod.rs:988`) is a 64-bit FNV of full file source.

For a local single-user cache this is a defensible performance choice — it is stable, seedless, well-constructed (length-prefixed, kind-tagged), and byte-identical across machines. For anything shared, it is not adequate: FNV-1a collisions are trivially constructible by an adversary, and the consequence in a *security* linter is a wrong cache hit that suppresses a finding. Note also that the layer cache does **not** hard-reject a key mismatch — `invalidation_allows_manifest_reuse` (`layer_cache.rs:657-668`) falls through to the invalidation planner when `manifest.key != *key`, so a filename collision is not fully fenced.

This blocks any move to a shared/CI cache (§C7) and should be fixed before it, not after.

### C6. Memory/eviction: the research is thin too

This is not a case of the implementation ignoring the research. `research/*` contains **no memory ceiling, no eviction algorithm, no spilling design**. LRU is named only as an inherited idea (`research/analysis-kernel/RESEARCH-ANALYSIS.md:83` "LRU limits for large memoized outputs"), streaming appears once and only for digest computation (`:250`), and memory is treated as a *reported metric* (peak RSS in benchmarks) rather than a *budget*. Memory containment is delegated to algorithmic budgets + `BudgetExceeded` status.

`.planning/ROADMAP.md:26` (Phase 66, "streaming bounded-batch ingest") is the first place streaming becomes a deliverable, and it is unstarted.

**Treat this as an open design gap in both the research and the code**, not as an implementation shortfall.

### C7. Remote cache: correctly out of scope, but not currently possible

Research is explicit (`research/local-semantic-store/decisions/DECISIONS.md:26-43`): local value must be proven first; build "registry-ready seams," not a registry. `.planning/REQUIREMENTS.md:41`: "Do not build a remote package-summary registry in v2.0."

The implementation matches, and goes further in the machine-local direction: the cache lives at `<repo>/.polint/cache` (`cache/mod.rs:346-350`), `.gitignore:25` excludes it, and `cli/mod.rs:676` documents "Ensures `.polint/.gitignore` lists `cache/` so analysis cache stays local to each machine."

**Is it architecturally possible?** Yes, and closer than you'd expect. Cache *content* is machine-independent: relative paths only, no mtimes, no absolute paths, no env values in keys. Four things block it:

1. 64-bit FNV payload digest is not an acceptable integrity gate for untrusted content (§C5).
2. `provider_version = CARGO_PKG_VERSION` means a shared cache is single-version-only.
3. No export/import, no namespacing, no cross-machine validation.
4. `POLINT_CACHE_DIR` **silently downgrades path-safety enforcement**: `Cache::default_for_repo` sets `repo_root = Some(...)` only when the env var is unset (`cache/mod.rs:114-124`), which switches all I/O off the hardened `repo_fs` containment path onto raw fs. That is an env-controlled security-posture change and it is exactly the knob a remote-cache integration would set.

---

## (d) Target design

### D1. Salsa vs bespoke — judgement

**Stay bespoke. The research verdict was right, and it is more right now than when it was written.**

Not because Salsa is bad, but because Salsa solves the problem polint doesn't have yet. Salsa gives you demand-driven memoization with red-green verification over *scalar-ish* query results. polint's providers produce large relation sets that are wholesale-replaced into a monolithic `AnalysisDb` (`db.replace_semantic_mir(output)`). Dropping Salsa on top of that today would memoize 23 whole-program queries — exactly the granularity that already exists, at the cost of a hard dependency and a lifetime/storage rewrite.

The research's own revisit criteria (`research/incremental-query-engine/decisions/001-layered-incrementality-not-salsa-first.md:97-113`) are the right test: adopt Salsa when *"the native demand query engine starts replicating most of Salsa badly."* polint is not there — it hasn't started replicating it at all. It has written down the type names.

**Critically: the current design has NOT diverged from a demand-driven model in a way that requires a rewrite.** The key algebra is right. `QueryKey`, `SummaryKey`, `DiagnosticKey`, `LayerKey`, `DependencyEdge`, `CacheNode`, the 5-action invalidation state machine, `PrecisionTier` — these are all convergent with Salsa's model and in some ways richer (Salsa has no notion of precision or validation status in a key, and polint's research is correct that it needs one).

What is missing is not *shape*, it is *plumbing*: a query function that calls `lookup` before it computes. The gap is small and structural, not conceptual. The one thing that must be fixed on the key side is the SCC key's circular `budget_digest` (`closure.rs:719-724`) and its empty `layer_digests` — a key must be constructible from inputs alone.

If Salsa is ever adopted, adopt it for the demand-query subsystem only, behind the `QueryKey` abstraction that already exists. Keep the layer cache bespoke — it handles lifecycle digests, tool invocations, extension quarantine, and validation status, none of which Salsa models.

### D2. Target architecture

**1. Demand-driven query layer over the existing layer cache.**
Give the 7 declared `QueryKind`s real implementations with the standard shape:

```
fn query(ctx, key) -> Result {
    if let Some(hit) = engine.lookup(&key) { return hit }     // <- the missing line
    let value = compute(ctx, key);
    engine.insert(key, value);
}
```

`QueryContext` (`analysis/demand/context.rs`) already records dependency reads — that is the tracing half of red-green, already written. Wire it. Start with `FunctionSummary` and `FunctionCfg`, which are the two that would let `polint review` recompute a frontier.

**2. Shape digests, not one content digest.**
The highest-value single change. Add, per file, alongside `content_hash`: an import-shape digest and a public-signature digest (rust-analyzer's ItemTree split; TypeScript's `.d.ts` boundary). Then a function-body edit stops invalidating dependents' module/symbol/summary layers. This is what makes `ChangeKind::{ImportShape, PublicApiShape}` — already defined and already handled by the planner — actually reachable.

**3. Per-file/per-function layer keys where the provider is per-file.**
Today `LayerKey.input_digests` for the Go syntax layer is every Go file. Split whole-repo layers into per-partition keys (per package for topology, per file for syntax, per callable for summaries) so a one-file edit invalidates one entry.

**4. Persist `SummaryKey`.**
The type exists (`keys.rs:92`), is function-granular, and has a `body_shape_digest` + `dependency_summary_digests` — it is correctly designed. It is never written. This is the keystone for warm `polint review` (Phase 67).

**5. Streaming ingest + a working-set budget.**
- Cap source reads: route `fs/mod.rs:135` through `repo_fs::read_file_to_string_with_limit` with a configurable ceiling and a `polint/capability` diagnostic on skip.
- Bound rayon width by an explicit memory-aware pool rather than default.
- Process in bounded batches: discover → read batch → parse → extract facts → **drop batch sources** → next. Retain `Arc<str>` only for files a rule will actually render.
- Make `record_fact_meta` opt-in (or intern `stable_key`/`payload_digest`). Measure first — §B4 is a hypothesis.

**6. Cache hygiene.**
- Mark-and-sweep GC for `blobs/` against live manifests; run opportunistically, bounded.
- Replace the O(n) `evict_stale_manifests_for_key` directory scan (`layer_cache.rs:536`) with an index.
- Read `created_by_polint` on load, or drop the field.
- Per-provider versions instead of `CARGO_PKG_VERSION`, so a patch release doesn't cold-start every user.

**7. Blake3 for content addressing.** As already locked in `.planning/REQUIREMENTS.md:38`. Keep FNV for in-memory, non-durable fingerprints; use blake3 for anything that names a file or gates a reuse decision.

**8. Production observability.** `debug.rs` is `#[cfg(test)]`. Expose a `--cache-stats` / `--explain-cache` surface built on the `CacheStats` and `KernelRunReport` that are already collected and already thrown away. Without this, nobody can tell whether any of the above worked.

---

## (e) Migration path

Ordered by (value ÷ risk). Every step keeps the current engine working; nothing here is a rewrite.

**Step 0 — Instrument (no behavior change).**
Surface `KernelRunReport` + per-provider `CacheStats` behind a flag. Ungate the parts of `run_report.rs` that are `#[cfg(test)]`. Add a warm-run RSS + hit/miss/recompute report to the Phase 63 benchmark harness. *You cannot land any of the below honestly without this.* Half a day.

**Step 1 — Stop rule digests from poisoning parser caches.**
Remove `rule_hash`/`plan_hash` from `CacheKey::for_file` (`cache/mod.rs:42`); introduce a separate `RuleKey` for anything genuinely rule-dependent. Gate on the research's own criterion: "rule option edits do not invalidate parser facts." Highest value-per-line in this document. ~1 day.

**Step 2 — Cap and bound the source read.**
Route `fs/mod.rs:135` through the existing bounded-read helper; emit a capability diagnostic on skip; bound the rayon pool. Removes the unbounded-file OOM vector without touching the pipeline. ~1 day.

**Step 3 — Per-provider versions + read `created_by_polint`.**
Add a `version: u32` to `ProviderManifest`, replace `env!("CARGO_PKG_VERSION")` in `provider.rs:14-16`. Stops every release from cold-starting every user, and makes upgrade staleness detectable. ~1 day.

**Step 4 — Blake3 for durable digests.**
Swap `payload_digest_for_bytes` and `content_hash` to blake3; keep `stable_hash` (FNV) for in-memory fingerprints. Bump `LAYER_CACHE_MANIFEST_SCHEMA` so old entries are ignored rather than misread. Prerequisite for any shared cache. ~1-2 days.

**Step 5 — Blob GC + manifest index.**
Mark-and-sweep on `blobs/`; replace the per-miss directory scan. Makes long-lived caches viable. ~2-3 days.

**Step 6 — Shape digests.**
Add import-shape and public-signature digests per file; feed them into `ChangeSet` classification so `ImportShape`/`PublicApiShape` become reachable instead of everything degrading to `Unknown`. Ship behind a flag, validate with the research's cold-vs-incremental equivalence oracle (`research/analysis-kernel/algorithms/cache-invalidation.md:99-117`):
`assert normalized_facts(cold) == normalized_facts(incremental)`. **This is the first step that changes analysis results if you get it wrong** — the oracle is mandatory, not optional. ~1-2 weeks.

**Step 7 — Wire one real demand query.**
`FunctionSummary`. Fix the circular `budget_digest` in `closure.rs:719-724` first (derive the budget digest from the *configured* budget, not the observed iteration count) and populate `layer_digests`. Then add the `lookup` call. Persist via `SummaryKey`. Measure frontier recompute on a one-function edit. ~1-2 weeks.

**Step 8 — Split whole-repo layer keys into partitions.**
Per-file syntax keys, per-package topology keys. Depends on Step 6. ~2 weeks.

**Step 9 — Give the SQLite store data.**
Follow the R0-R6 restart plan (`research/local-semantic-store/RESTART-PLAN.md:49-117`) and its delivery budgets (max 3 tasks / 15 files / 2,500 lines / 1 schema family per PR). Mirror **one** provider family end to end (R4) before expanding. Add a real enablement path — a config key or flag — so it is testable outside `#[cfg(test)]`. Do not start this before Step 7 proves the query layer works; otherwise you are persisting a cache nobody reads.

**Step 10 — Bounded-batch streaming ingest** (roadmap Phase 66) and **eviction policy**. Requires Steps 2, 6, 8. This is where "memory proportional to working set, not repo" (`.planning/REQUIREMENTS.md:28`) actually becomes true rather than aspirational.

**Deliberately not on this path:** adopting Salsa; a provider trait + runtime toposort (the hardcoded order is ugly but it is not what is slow — revisit only if provider count grows or parallel provider execution becomes the bottleneck); a remote cache (blocked on Steps 3, 4 and a security review of `POLINT_CACHE_DIR` at `cache/mod.rs:114-124`); and deleting the dead demand scaffolding — Step 7 should consume it, not replace it.

---

## Appendix — dead-code inventory

For scope calibration. LOC includes tests.

| Module | LOC | Status |
|---|---|---|
| `analysis_kernel/incremental/` | 9,349 | Live: `digest`, `layer_cache`, `keys` (LayerKey only), `invalidation`, `dependency_index`, `input_snapshot`, `stats`. Dead: `demand`, `quarantine`, `change_set` (as an edit-change-set), `QueryKey`/`SummaryKey`/`DiagnosticKey` |
| `analysis/demand/` | 2,188 | **Entirely dead** — every use is `#[cfg(test)]` |
| `analysis_kernel/store/` | 1,303 | Wired but permanently disabled; schema is one bookkeeping table |
| `analysis_kernel/debug.rs` | 3,960 | `#![cfg(test)]` — absent from release builds |

Eight `expect(dead_code)` allowances in `incremental/` alone, each with a "before Plan NN wires…" reason string. The scaffolding was built ahead of its consumers and the consumers did not arrive. That is a recoverable position — the types are largely right — but it should be recognised as ~7,000 LOC of carrying cost that currently returns nothing.
