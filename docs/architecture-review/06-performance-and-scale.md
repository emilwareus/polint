# 06 — Performance and Scale Readiness

**Method note.** This review was produced by reading source and running greps only — no
`cargo build`, no profiling run. Every number below is tagged **[recorded]** (extracted from a
file in this repo, with a path) or **[estimated]** (derived by me from struct definitions and
loop structure, with the derivation shown). Nothing here is a measurement I took.

**Verdict.** polint has *one* well-executed scale mechanism (capability gating + rule-scoped
discovery, which took the reference monorepo from a 30 GB+ OOM to ~1 GB) and *one* real
regression gate (+20% RSS / +25% cold latency, semantic-store-scoped). Underneath that, the
engine is a single-threaded whole-program pipeline that re-parses the entire TypeScript corpus
up to ~13 times per run, stores three redundant copies of a ~120-byte identity string on every
fact, and looks up files by linear scan inside per-fact loops. It has never been measured on a
repository. The scale suite manifests exist; the repos have never been cloned.

---

## (a) Recorded facts — what this repo actually knows about its own performance

### a.1 The only whole-repo numbers that exist

| Metric | Value | Source | Note |
|---|---|---|---|
| Reference monorepo peak RSS | **~1 GB** | `research/evaluation-harness/suites/devloupe-monorepo-local.toml:8`, `.planning/REQUIREMENTS.md:48` | **[recorded]** Private repo, LOC/file count **not recorded anywhere** |
| Reference monorepo cold wall-clock | **7.4 s** | same | **[recorded]** Capability set of the run is not recorded |
| Reference monorepo warm wall-clock | **4.6 s** | same | **[recorded]** |
| Prior peak RSS before capability gating | **30 GB+ (OOM)** | `.planning/REQUIREMENTS.md:28` | **[recorded]** Attributed to "eager whole-repo pipeline + whole-repo source loading" |
| Regression budget | ≤ +20% peak RSS, ≤ +25% cold wall-clock | `crates/polint/src/eval/baseline.rs:199,205` | **[recorded]** Relative-only; no absolute ceiling |

The 1 GB / 7.4 s figure is a **comment in a TOML file**. It is not reproducible in CI, not
attached to a commit, and the repository it describes is proprietary and not cloneable. It is
the closest thing polint has to a scale measurement.

### a.2 The committed baselines are toy-sized

`research/evaluation-harness/baselines/store-disabled-check.json:6-9` **[recorded]**:
peak RSS 46,645,248 B; peak-RSS delta 40,566,784 B; cold 26 ms; warm 22 ms. `repo_id` is
`polint-tiny-fixture`. `store-disabled-review.json` is the same shape (46.5 MB / 25 ms / 22 ms).
`persisted-graph-accuracy.json:8-21` has recall/precision **null** and expected/observed edges
**0/0** — an unfilled placeholder.

`gate.rs:100-110` **[recorded]** explicitly states the committed baseline's *timings* are too
small to be used as a ratio denominator; only its RSS is used, and only as a fallback.

### a.3 The scale suite exists on paper and has never been run

Four `kind = "performance"` manifests are committed (`grafana/grafana`, `gohugoio/hugo`,
`excalidraw/excalidraw`, `devloupe`). The sweep that consumes them —
`crates/polint/src/eval/bench/sweep.rs` — is `#![cfg(test)]` (line 20) and, per its own doc
comment (lines 14-17) **[recorded]**, "Absent large-repo checkouts are SKIPPED, not failed."

`research/evaluation-harness/repos/` **does not exist in this working tree**. No
`benchmark-curves.json` and no `benchmark-report.md` are committed. **No scale repository has
ever been measured.**

### a.4 The recall-vs-runtime trajectory (the most important recorded signal)

From `performance/2026-06-06-jelly-gap-closure-research.md`, `2026-06-14-f1-recall-research.md`,
`2026-06-15-jelly-fn-decomposition.md`, `2026-06-17-*` — 62 recorded iterations on the Jelly
JS/TS call-graph micro-suite (76 programs; one is an Express app of ~80 source files + ~20
third-party packages, per `2026-06-09-teaching-a-linter-to-read-javascript.md:198-202`).
All **[recorded]**:

| Iteration | TP/FP/FN | P / R / F1 (%) | Suite runtime |
|---|---|---|---|
| baseline (`2026-06-06:L20`) | 8 / 6 / 1471 | 57.14 / 0.54 / **1.07** | **793 ms** |
| it6 (`:L106`) | 134 / 45 / 1345 | 74.86 / 9.06 / 16.16 | 7,808 ms |
| it14 (`:L130`) | 405 / 519 / 1074 | 43.83 / 27.38 / 33.70 | **86,477 ms** |
| it30 (`:L146`) | 675 / 586 / 804 | 53.53 / 45.64 / 49.27 | 83,804 ms |
| it39 (`:L284`) | 803 / 81 / 676 | 90.84 / 54.29 / 67.97 | 96,966 ms |
| it56 (`:L934`) | 965 / 54 / 514 | 94.70 / 65.25 / 77.26 | 105,380 ms |
| it62 (`:L976`) | 1078 / 57 / 401 | 94.98 / 72.89 / **82.48** | *(runtime column dropped)* |
| latest (`2026-06-17-*:L3`) | 1219 / 49 / 260 | 96.14 / 82.42 / **88.75** | *(not recorded)* |

Two things stand out. First, **runtime grew ~110× (793 ms → ~90-105 s) while F1 grew from 1.07%
to 88.75%.** Second, **the runtime column was dropped from the log at iteration 57**. From
iteration 57 onward there is no recorded runtime for any recall improvement. The one budget
that was ever written down is a prose note at
`performance/2026-06-11-js-points-to-heap-plan.md:393` **[recorded]**: "current total ≈ 90–105 s;
budget the new policy ≤ 20 s." It was never enforced by a test.

Iteration cost is recorded as **~4 min/iter** (`2026-06-15-jelly-fn-decomposition.md:34`) and
**~2 min/increment** (`2026-06-17-array-model-refactor-plan.md:126`) **[recorded]** — i.e. the
inner development loop on a ~100-file corpus is already minutes long.

### a.5 Micro-benchmark memory

`performance/2026-06-06-static-analysis-performance.md:35` **[recorded]**: peak memory
**63,488,480 B (~60.5 MiB)** for the combined Go x/tools + Jelly benchmark test, wall time
57.62 s including release compile, test body 1.93 s. Host: macOS 26.5 arm64, polint 0.1.14.

### a.6 The measurement plumbing that *does* exist

- Real OS peak RSS via `getrusage(RUSAGE_SELF).ru_maxrss` / `K32GetProcessMemoryInfo`:
  `crates/polint/src/eval/bench/measure.rs:26-45,58-103`. These are the workspace's only two
  `unsafe` blocks. This is well done.
- Per-point isolation in a child process so peak RSS is order-independent
  (`eval/bench/sweep.rs:8-10`). Also well done.
- `CurvePoint` telemetry keyed by `repo_file_count`, `repo_source_bytes`, `diff_files`,
  `diff_hunk_lines`, carrying cold/warm wall-clock, peak RSS, cache size, and
  budget-exhaustion counters (`eval/bench/curve.rs:1-18`). This is the right schema.

The substrate is good. Nothing feeds it real data.

---

## (b) The performance architecture as built

### b.1 Parallelism: four call sites in 253,559 lines

`rg -n 'par_iter|par_bridge|rayon|into_par_iter' crates/polint/src` returns **8 lines across
4 files** (4 imports, 4 uses):

| Site | What is parallel | Deterministic? |
|---|---|---|
| `fs/mod.rs:133` | `read_to_string` for all discovered paths | Yes — `IndexedParallelIterator::collect` preserves order; `discover_files_scoped` pre-sorts at `fs/mod.rs:75`; asserted by `fs/mod.rs:362` |
| `ts/adapter.rs:306` | Per-file oxc parse + syntax-fact extraction | Yes — `results.sort_by(relative_path)` at `ts/adapter.rs:312` |
| `go/adapter.rs:267` | Per-file tree-sitter parse + syntax-fact extraction | Yes — same sort at `go/adapter.rs:277` |
| `core/mod.rs:7738` | Rule execution (`rules.par_iter()`) | Yes — indexed collect then flatten; asserted by `core/mod.rs:10252` |

Determinism is genuinely handled. Everything else in the engine is serial.

**Nothing between "parse" and "run rules" is parallel.** No fixpoint, no call-graph solve, no
points-to solve, no MIR lowering, no CFG construction, no dataflow, no summaries, no
whole-program value-flow. That is 112,545 lines of `crates/polint/src/analysis/` running on
one core.

**Locks in hot paths:** effectively none, which is the flip side of having no parallelism.
The only lock is `sdk/scope.rs:52` — a process-global `OnceLock<RwLock<HashMap<String,
Option<GlobMatcher>>>>` memoizing compiled globs. Its own doc comment says it is called "once
per fact row (every file, function, and literal)". Under `run_rules`'s `par_iter` this
`RwLock` is taken on *every* scope check across all rule threads. Reads dominate so contention
is mild, but `glob_matches` (`sdk/scope.rs:73`) also does `format!("./{value}")` — a heap
allocation per call — on the fallback branch of a path its own comment calls hot.

**No thread-pool configuration exists.** `rg 'ThreadPoolBuilder|RAYON_NUM_THREADS|num_threads|
available_parallelism|--jobs'` returns zero hits. Rayon's global default pool (= core count) is
used, with no `--jobs` flag and no way to bound polint inside a CI container.

### b.2 Memory layout

**No string interner. At all.** `rg -n 'intern|Interner|lasso|string_cache|ustr|SmolStr|
CompactString'` over `crates/polint/src` returns only the English word "internal". None of
`lasso`, `string-interner`, `string_cache`, `ustr`, `smol_str`, or `compact_str` appear in
`crates/polint/Cargo.toml` or `Cargo.lock`.

Counts across `crates/polint/src` **[recorded, by grep]**:

| Pattern | Count |
|---|---|
| `<name>: String,` struct fields | **1,040** |
| `<name>: Option<String>,` fields | 317 |
| `<name>: Vec<String>,` fields | 207 |
| `stable_key: String,` fields specifically | **207** |
| `BTreeMap<String, ...>` usages | 316 |
| `HashMap<String, ...>` usages | 2 (`sdk/scope.rs`, `analysis_kernel/metadata.rs`) |
| `HashSet<String>` / `BTreeSet<String>` usages | 142 |
| `Arc<...>` / `Rc<...>` usages | 8 files |
| `.clone()` (non-test) | 2,849 |
| `.to_string()` (non-test) | 6,651 |

Arc usage is correct where it exists: `SourceFile.source: Arc<str>` (`core/mod.rs:263`) and
`sources_by_relative_path` (`core/mod.rs:3833`) shares via `Arc::clone`, so source text is
stored exactly once. `analysis/identity/dedup.rs` uses `Arc<str>` in its dedup keys. This is a
small island of good practice in an otherwise `String`-everywhere codebase.

**The `stable_key` triple-store is the dominant avoidable cost.** A stable key is a
human-readable length-prefixed concatenation, not a hash — `stable_key_from_parts`
(`analysis_kernel/metadata.rs:465-480`) produces e.g.
`8:Function|4:name=7:handler|4:path=11:src/main.go`, and call-site keys embed
`file_key(db, body.file)` (a full repo-relative path) plus span coordinates
(`analysis/calls/extract.rs:438-450`). Each such key is stored **three times**:

1. On the fact struct itself — `stable_key: String` (207 declaration sites; e.g.
   `analysis/data_flow/facts.rs:26`, `analysis/semantic_graph/facts.rs:133,148`).
2. In `FactMeta.stable_key: String` (`analysis_kernel/metadata.rs:230`), stored in
   `FactMetaStore.rows` (`metadata.rs:356`).
3. As the **key** of `FactMetaStore.stable_key_owners: BTreeMap<FactFamily, HashMap<String,
   StableKeyOwner>>` (`metadata.rs:357`), inserted by value at `metadata.rs:388`.

`payload_digest: String` is stored twice (once in `FactMeta`, once in `StableKeyOwner`,
`metadata.rs:281`). None of these is `Arc<str>`, none is interned, none is a fixed-width hash —
even though `stable_hash_bytes` (`incremental/keys.rs:1270`) already produces a 16-hex-char FNV
digest and `symbol_graph/stable_id.rs:202` already derives a `u64` id from a stable key.

**ASTs are correctly dropped.** Every `Allocator` is function-local; `analyze_ts_source_file`
(`ts/adapter.rs:484`) drops the arena on return, and `parse_go_file` (`go/adapter.rs:446`) drops
the tree-sitter `Tree`. The Go parser is reused via a thread-local `RefCell<Option<Parser>>`
(`go/adapter.rs:25`). This is deliberate and right — but it is paid for in (b.3).

### b.3 The re-parse tax — the largest CPU bottleneck

Because no AST or arena is retained, **every downstream provider re-parses the file from
`SourceFile.source`**. Full-corpus TS re-parse sites in a `dataflow`-capability run:

| # | Site | Loop scope | Parallel? |
|---|---|---|---|
| 1 | `ts/adapter.rs:484` | all TS files | **yes** (rayon, disk-cached) |
| 2 | `symbol_graph/ts.rs:161` (loop at `:106`) | all TS files | no |
| 3 | `analysis/semantic_graph/build.rs:234` via `provider.rs:86` | all TS files | no |
| 4 | `analysis/semantic_graph/build.rs:283` via `solver/ts_tokens/inputs.rs:71` | all TS files | no |
| 5 | `analysis/semantic_graph/build.rs:283` via `solver/ts_object_model/inputs.rs:96` | all TS files | no (gated off by default, `solver/budget.rs:189`) |
| 6 | `analysis/mir/lower_ts.rs:79` | all TS files | no |
| 7 | `analysis/calls/ts_value_flows.rs:323` | all TS files **× up to 4 rounds** (`MAX_MODULE_SUMMARY_ROUNDS`, `ts_value_flows.rs:31`) | no |
| 8 | `analysis/calls/ts_value_flows.rs:395` | all TS files | no |
| 9 | `analysis/calls/ts_value_flows.rs:63` | all TS files | no |
| 10 | `analysis/calls/js_points_to/provider.rs:60` | all TS files | no |
| 11 | `analysis/semantic_graph/build.rs:951` | all TS files, when inventory absent | no |

That is **1 parallel + up to 12 serial full-corpus oxc parses per run** **[estimated: derived
by reading the call sites; not measured]**. Go pays a smaller version: `go/adapter.rs:451`
(parallel) plus `analysis/mir/lower_go.rs:61` (serial), and `lower_go.rs` constructs a
*brand-new* `tree_sitter::Parser` per file rather than reusing the thread-local one.

`compute_module_export_summaries` (`ts_value_flows.rs:315-368`) is the worst single offender: a
serial whole-program fixpoint that re-parses **every TS file on every round**, up to 4 rounds,
just to compare `next == summaries`. Its termination check is a `BTreeMap` structural equality
over the whole program, and its convergence bound is a fixed round count rather than a
worklist.

This is a coherent design decision taken to an incoherent extreme: dropping ASTs to bound
memory is correct, but the mitigation (parse once, extract everything you will ever need into
facts) was never applied. Instead the engine drops the AST and then parses again eleven times.

### b.4 O(F²) and O(facts × F) file lookups

`FileId` is dense and sequential (`fs/mod.rs:321-338` asserts ids `0,1,2` in discovery order),
so `db.files()[id.0 as usize]` is O(1). Twelve sites do a linear scan instead:

| Site | Outer loop | Complexity |
|---|---|---|
| `ts/adapter.rs:338-340` | every file in the restored cache payload | **O(F²)** on every run, cold *and* warm |
| `go/adapter.rs:299-301` | same | **O(F²)** |
| `analysis/calls/direct.rs:295` | every call site | O(call_sites × F) |
| `analysis/entrypoints/recognizers_ts.rs:1505` | every call site | O(call_sites × F) |
| `analysis/entrypoints/recognizers_ts.rs:744,1573` | per file-id / per site | O(n × F) |
| `analysis/entrypoints/recognizers_go.rs:852,917` | every call site | O(call_sites × F) |
| `analysis/cfg/lower_ts.rs:352` | every MIR operation (`source_text`) | **O(mir_ops × F)** |
| `analysis/cfg/lower_go.rs:325` | same | **O(mir_ops × F)** |
| `analysis/calls/extract.rs:481` | per call-site file | O(n × F) |
| `module_graph/go.rs:748`, `symbol_graph/go.rs:2323`, `analysis/extensions/validate.rs:461` | various | O(n × F) |

`cfg/lower_ts.rs:346-349` compounds it: `operation_evidence` calls `source_text` (the linear
scan) and then `.to_string()`s the resulting slice — **one heap allocation of the source text
per MIR operation**, in addition to the scan.

At 4,000 files these are invisible. At 100,000 files, `ts/adapter.rs:338` alone is
10¹⁰ string comparisons **[estimated]**.

### b.5 Source retention

`crates/polint/src/fs/mod.rs` (398 lines, read in full):

- `discover_files_scoped` (`:31-77`) walks with `ignore::WalkBuilder`, filters by language and
  include/exclude/rule-scope globs, sorts. Filtering happens **before** reading — this is the
  good part, and it is what the rule-scope gate exploits.
- `load_analysis_files_with_timings_scoped` (`:121-155`) reads **every discovered file whole
  into a `String` via `fs::read_to_string`, in parallel, collecting all of them into one `Vec`
  before any of them is processed** (`:132-139`). Then a serial loop hands each to
  `db.add_file` (`:144-146`).
- `AnalysisDb::add_file` (`core/mod.rs:986-996`) computes `fingerprint(&[&source])` — a full
  pass over the bytes — then does `Arc::from(source)`, which **memcpys the entire file** into a
  fresh `Arc<str>` allocation and frees the `String`.
- **There is no mmap, no streaming, no lazy read, no eviction, and no `drop` of source after
  fact extraction.** `SourceFile.source` lives for the whole process because rules
  (`sdk/facts.rs`), diagnostics rendering (`diagnostics/mod.rs:1043`), and the ~12 re-parse
  sites all read from it.

**Bytes in RAM per MB of source [estimated]:**

| Component | Cost |
|---|---|
| `Arc<str>` source text | 1.00 MB (exact) |
| Transient `String` during load | up to +1.00 MB at peak (the `Vec<(DiscoveredFile, String)>` is fully materialized before conversion begins; it drains as `add_file` consumes it, so steady-state peak ≈ 1.0-1.2 MB, plus allocator fragmentation from N alloc/copy/free cycles) |
| `SourceFile` record (PathBuf + `relative_path` String + 16-char `content_hash` String + Arc + enum) | ≈ 250 B/file → at ~250 B/file and ~4 KB/file, ≈ 0.06 MB |
| Extra memcpy work | 1.00 MB copied + 1.00 MB FNV-hashed per MB, once |

So source retention itself is ~1.05× the corpus and is **not** the scale problem. The facts are.

### b.6 Budgets and bailouts

Budgets are the strongest part of the design. `crates/polint/src/analysis/solver/budget.rs`
defines `SolverBudget` with 23 named `BudgetReason` variants (`:253-277`) and a three-state
`BudgetStatus { WithinBudget, BudgetExceeded, NotRun }` (`:221-230`). Defaults **[recorded]**:

| Knob | Default | Site |
|---|---|---|
| `solver.max_steps` | 10,000 | `budget.rs:174` |
| `solver.max_outer_iterations` | 64 | `budget.rs:178` |
| `points_to.max_objects_per_var` / `max_dynamic_vars` | 64 / 512 | `budget.rs:38-39` |
| `go.address_taken_threshold` / `max_candidates_per_callsite` / `max_rta_rounds` / `max_worklist_steps` | 256 / 128 / 32 / 10,000 | `budget.rs:81-86` |
| `js.max_tokens_per_var` / `max_candidates_per_callsite` / `max_token_worklist_steps` | 128 / 256 / 10,000 | `budget.rs:109-111` |
| `object.*` (7 knobs, model disabled by default) | 128/128/128/8/8/64/10,000 | `budget.rs:140-146`, `:189` |
| `PathBudget` (slicing): `max_paths` / `max_nodes` / `max_edges` / `max_depth` | 5 / 64 / 96 / 32 | `analysis/slicing/paths.rs:38-42` |
| Demand query: `MAX_ITERATIONS` / `MAX_NODES` / `MAX_DEPTH` | 100 / 10,000 / 64 | `analysis/demand/query.rs:70-72` |
| `MAX_MODULE_SUMMARY_ROUNDS` | 4 | `analysis/calls/ts_value_flows.rs:31` |
| `MAX_HARVEST_DEPTH` / `ARRAY_INDEX_LIMIT` | 256 / 10 | `analysis/calls/js_points_to/harvest.rs:28,32` |

**Degradation is reported, not silent.** `analysis/unknown_taxonomy/collect.rs:252-274` turns a
run-level `BudgetExceeded` into an `UnknownRow` with `status: "budget_exceeded"`,
`suggested_artifact: "budget_or_model"`, and the specific `BudgetReason` labels;
`collect.rs:447-465` does the same per data-flow budget with `limit` and `observed` values.
`analysis/summaries/closure.rs:309,373` reports SCC non-convergence explicitly. `curve.rs:16-18`
carries budget-exhaustion counters as first-class benchmark fields.

**The gaps:** (1) budgets bound *precision work*, not *wall-clock or memory* — there is no time
budget, no memory budget, and no way for a user to say "give me the best answer you can in 5
seconds"; (2) `max_outer_iterations = 64` and `max_steps = 10,000` are **per-run global
counters**, so on a large repo the budget is exhausted by ordinary size and every downstream
finding degrades, with no signal distinguishing "this repo is pathological" from "this repo is
big"; (3) there is no budget at all on the ~12 full-corpus re-parses, the O(F²) scans, or
`AnalysisDb` growth.

### b.7 Always-on whole-program work

`analysis_kernel/mod.rs:942` calls `validation::validate_fact_metadata(&db, ...)`
**unconditionally on every run**, after every provider. It runs 16 sub-validators
(`validation.rs:52-66`) over the entire fact DB, and `IdSets::from_db` (`validation.rs:531-556`)
first materializes `BTreeSet`s of **every** file, function, MIR body, MIR operation, MIR place,
CFG block, call site, symbol, type set, value fact, allocation token, and access path id — then
derives two more sets (`pt_vars`, `object_tokens`) from the places and operations. `validation.rs`
is 5,780 lines. This is a full extra whole-program pass, in production, on every `polint check`,
with no gate.

### b.8 Persistence

The SQLite store (`analysis_kernel/store/`) has exactly one table,
`_polint_schema_migrations` (`store/migrations.rs:13`). It persists no facts. `SemanticStore::maintain`
runs last (`analysis_kernel/mod.rs:964-968`) and its own comment says persistence "must not
change provider execution." The layer cache (`incremental/layer_cache.rs`) is used by 6 modules
but caches whole-layer JSON payloads keyed by all inputs at once (see doc 05), capped at
64 MB/payload (`layer_cache.rs:27`). There is **no spill-to-disk path for `AnalysisDb`**.

### b.9 Per-file vs whole-program boundary

`AnalysisKernel::run` gates work in five slices
(`analysis_kernel/mod.rs:96-113`, trigger lists at `:31-60`). A syntactic rule set skips
everything below "syntax" *and* narrows file discovery to the union of enabled rules' `files`
globs (`:118-124`) — this is the mechanism that fixed the 30 GB OOM.

| Stage | Provider | Unit | Parallel today | Cacheable per-file? | Gate |
|---|---|---|---|---|---|
| Discovery | `fs::discover_files_scoped` | repo | no (`ignore` walk is serial here) | n/a | always |
| Source read | `fs/mod.rs:133` | **per file** | **yes** | trivially | always |
| Go syntax | `go/adapter.rs:267` | **per file** (isolated `local_db`) | **yes** | yes (layer cache, but whole-corpus key) | always |
| TS syntax | `ts/adapter.rs:306` | **per file** | **yes** | yes (same) | always |
| Module graph | `module_graph::derive_requested_*` | **whole program** | no | no | `resolved_imports`/`module_graph`/… |
| Symbol graph | `symbol_graph/ts.rs:106` | per file, run serially | **no** | **yes — should be parallel** | `symbols`/`references` |
| Module topology | `analysis::topology` | whole program | no | no | `run_cfg_call_pipeline` |
| Semantic MIR | `analysis/mir/lower_{ts,go}.rs` | per file, run serially | **no** | **yes — should be parallel** | `run_semantic_pipeline` |
| CFG + dominators | `analysis/cfg/` | **per function** | **no** | **yes — should be parallel** | `run_cfg_call_pipeline` |
| Direct calls | `analysis/calls/extract.rs` | per call site, but scans all files | no | partly | `run_cfg_call_pipeline` |
| TS value flows | `analysis/calls/ts_value_flows.rs` | **whole program, ≤4-round fixpoint** | no | no | `calls` |
| JS points-to heap | `analysis/calls/js_points_to/` | **whole program** | no | no | `calls` |
| Go semantic (sidecar) | `go/semantic/client.rs:119` | whole program, external `go` process | subprocess only | no | `run_full_refinement_pipeline` |
| Identity + dedup | `analysis/identity/dedup.rs:116` | whole program (BTreeMap collapse + sort) | no | no | full refinement |
| Abstract domains | `analysis/domains/` | whole program | no | no | full refinement |
| Summaries + SCC closure | `analysis/summaries/closure.rs:261` | **whole program fixpoint** | no | no | full refinement |
| Entrypoints + reachability | `analysis/entrypoints/`, `reachability` | whole program | no | no | full refinement |
| Type/value/alias + solver | `analysis/solver/` | **whole program worklist fixpoint** | no | no | full refinement |
| Semantic graph | `analysis/semantic_graph/` | whole program | no | no | full refinement |
| Refined calls | `analysis/refined_calls/` | whole program | no | no | `run_cfg_call_pipeline` |
| Data flow + evidence | `analysis/data_flow/`, `evidence/` | whole program | no | no | `dataflow` |
| Metrics | `metrics.rs` | **per file / per function** | **no** | **yes — should be parallel** | always |
| Fact validation | `analysis_kernel/validation.rs:41` | whole program | no | no | **ungated — always runs** |
| Rule execution | `core/mod.rs:7738` | per rule | **yes** | no | always |

**Four stages are structurally per-file or per-function and run serially anyway**: symbol
graph, MIR lowering, CFG construction, metrics. Together they are a large fraction of a full
run and are embarrassingly parallel — the same `results.sort_by(relative_path)` pattern the two
syntax adapters already use would preserve determinism.

### b.10 Rule-host latency

`polint check` in a repo with `.polint/rules` does not analyze anything itself. It shells out:
`cli/mod.rs:4008` builds `cargo run --quiet [--release] --manifest-path .polint/rules/Cargo.toml
-- check --format json …` and parses the child's stdout. The generated
`.polint/rules/Cargo.toml` (`cli/mod.rs:886-900`) depends on the full `polint` crate.

Cost inputs **[recorded]**:
- `Cargo.lock` has **273 packages**.
- The dep graph includes `libsqlite3-sys 0.38.1` and `tree-sitter 0.26.8` / `tree-sitter-go
  0.25.0` (bundled C, compiled by `cc 1.2.61`) plus `oxc_parser`/`oxc_ast`/`oxc_semantic` 0.129.0.
- Default profile is **release** (`cli/mod.rs:4168-4171`: absent env var → `Release`), so first
  run is a full optimized build of all 273 crates including two C toolchain builds.
- Output goes to `CARGO_TARGET_DIR = cache_layout.rules_target_dir()` (`cli/mod.rs:4041`), so it
  is cached across runs, but it is a *separate* target dir from the user's own build.
- The workspace `Cargo.toml` declares **no `[profile.*]` section**, so no LTO, no tuned
  codegen-units, no `opt-level` override for the dependency graph.
- `POLINT_RULES_PROFILE=""` (empty string) maps to `Dev` (`cli/mod.rs:4174`) — an easy way to
  accidentally run the entire analysis engine unoptimized.

**Assessment for editor/agent loops: this is disqualifying as designed.** Cold cost is a
release build of 273 crates with two C compilations — minutes, not seconds. Warm cost is a
`cargo` invocation (lockfile read, fingerprint check of 273 units, link check) plus process
spawn plus JSON serialize/parse of every diagnostic — typically 200-800 ms of pure overhead
before any analysis starts **[estimated; not measured in this repo]**. Any touch of the rules
crate, any `polint` version bump, and any toolchain change re-triggers a full rebuild.
`RUSTUP_TOOLCHAIN` is forwarded (`cli/mod.rs:4053-4056`), so a toolchain mismatch silently
doubles the cost. There is no persistent-server mode, no incremental re-check, no way to keep a
warm process between edits.

---

## (c) Top bottlenecks, ranked

| # | Bottleneck | Evidence | Cost class |
|---|---|---|---|
| 1 | Up to 12 serial full-corpus oxc re-parses per run | table in b.3; worst: `analysis/calls/ts_value_flows.rs:323` × 4 rounds | O(rounds × corpus_bytes), single-threaded |
| 2 | Triple-stored `stable_key` + double-stored `payload_digest` per fact | `metadata.rs:230,281,356-357,388`; 229 `stable_key: String` fields | ~470 B/fact of pure redundancy **[estimated, derivation in (d)]** |
| 3 | Whole-program analysis is 100% single-threaded | 0 of 4 rayon sites are in `crates/polint/src/analysis/` (112,545 LOC) | leaves (cores−1)/cores on the table |
| 4 | `O(F²)` file lookup in the syntax-layer cache restore | `ts/adapter.rs:338-340`, `go/adapter.rs:299-301` | quadratic in file count, on *every* run |
| 5 | `O(mir_ops × F)` scan + `String` alloc per MIR operation | `cfg/lower_ts.rs:346-352`, `cfg/lower_go.rs:325` | quadratic + allocation storm |
| 6 | Ungated whole-DB validation on every run | `analysis_kernel/mod.rs:942`; `validation.rs:41-66,531-556` | full extra pass + 14 `BTreeSet`s of every id |
| 7 | Serial per-file stages that should be parallel | symbol graph `symbol_graph/ts.rs:106`; MIR `lower_ts.rs`/`lower_go.rs`; CFG; `metrics.rs` | free 4-8× on those stages |
| 8 | Fixed-round fixpoint with whole-program structural equality | `ts_value_flows.rs:315-368` (`next == summaries` on a whole-program `BTreeMap`) | O(rounds × program) compare, no worklist |
| 9 | `Arc::from(String)` memcpy + FNV pass per file | `core/mod.rs:988,993` | 2× corpus-byte pass at load |
| 10 | Rule-host `cargo run` on a 273-package graph | `cli/mod.rs:4008`, `Cargo.lock` | 200-800 ms warm floor, minutes cold |
| 11 | Global glob cache `RwLock` on the per-fact-row path | `sdk/scope.rs:52-65`, `:73-78` (plus `format!` alloc) | contention under `run_rules` par_iter |
| 12 | 2,849 non-test `.clone()` / 6,651 `.to_string()` | grep | allocator pressure throughout |

---

## (d) What breaks at 1M and 10M LOC

### d.1 The memory model

Per fact row admitted to `AnalysisDb` with metadata **[estimated — derived from struct
definitions, not measured]**:

| Component | Bytes | Derivation |
|---|---|---|
| Fact's own `stable_key: String` | ~152 | 24 B header + ~120 B content (family + label=value parts incl. a repo-relative path and span), allocator-rounded to 128 |
| `FactMeta` row inline | ~88 | 2× String header (48) + 2× `&'static str` (32) + 3 enums padded (8) |
| `FactMeta.stable_key` heap | ~128 | second copy of the same key |
| `FactMeta.payload_digest` heap | ~32 | 16 hex chars, rounded |
| `stable_key_owners` HashMap key | ~152 | **third** copy of the same key |
| `StableKeyOwner` + hashbrown slot | ~72 | `FactRef` (16) + `payload_digest` String (24 + 32) at 0.875 load factor |
| Fact's own payload fields | ~80 | ids, spans, enums; varies 40-200 |
| **Total** | **~700 B/fact** | of which **~465 B (66%) is redundant key/digest copies** |

Fact density per LOC in a `dataflow`-capability run, summed over the 99 `Vec<Fact>` fields of
`AnalysisDb` (`core/mod.rs:658-825`) plus the 18 `Option<*Store>` sub-stores **[estimated]**:
MIR places+operations ~1.5/LOC; CFG nodes+blocks+edges+reachability+dominators+postdominators+
control-dependence ~2.5/LOC; data-flow nodes+edges ~2/LOC; semantic nodes+edges+constraints
~1/LOC; symbols+definitions+references ~0.5/LOC; calls ~0.2/LOC; types/values/points-to/
access-paths ~1/LOC. **≈ 8 facts/LOC.**

→ **≈ 5.6 KB retained per LOC**, plus ~35 B/LOC of source text (negligible by comparison).

**Consistency check against the one recorded anchor:** 1 GB peak RSS ÷ 5.6 KB/LOC ≈ **180 K LOC**.
The devloupe monorepo's size is not recorded anywhere in this repo, so this is not a validation
— but it is at least the right order of magnitude for something called a monorepo, and it is
consistent with the recorded 30 GB+ pre-gating OOM (≈ 6× more, matching unscoped whole-repo
discovery pulling in vendored/generated trees).

### d.2 Projected failure points

| Corpus | Source bytes | Facts **[est.]** | Fact memory **[est.]** | Outcome |
|---|---|---|---|---|
| 200 K LOC | ~7 MB | ~1.6 M | ~1.1 GB | Recorded-adjacent. Works. Cold ~7 s. |
| **1 M LOC** | ~35 MB | ~8 M | **~5.6 GB** | Survives on a 16 GB dev machine; **OOMs a standard 7 GB GitHub-hosted runner**. Wall-clock ~35-60 s **[est., scaling the recorded 7.4 s superlinearly for the ~12 serial re-parses and the O(F²) scans]** |
| **10 M LOC** | ~350 MB | ~80 M | **~56 GB** | **Hard OOM everywhere.** Even with infinite RAM: `ts/adapter.rs:338` is ~1.6×10¹¹ string compares at ~40 K files; the 12 serial re-parses are ~4 GB of oxc parsing on one core |

Three things break, in this order:

1. **Memory, at ~1.5 M LOC on a 7 GB runner.** `AnalysisDb` is monolithic, never shrunk, and
   there is no spill path. The `Option<*Store>` sub-stores are populated but nothing is ever
   `drop`ped or `mem::take`n mid-pipeline even after its consumers have run.
2. **The quadratic scans, at ~50 K files.** `ts/adapter.rs:338` and `cfg/lower_ts.rs:352` are
   the two that hit first. These are one-line fixes (`FileId` is dense) and it would be
   negligent to ship 1 M LOC support without them.
3. **Wall-clock, throughout.** Even fixing 1 and 2, one core doing 12 corpus parses plus a
   whole-program solver on 35 MB of source is a multi-minute operation. There is no
   incremental path: the layer cache is keyed on the whole corpus at once, so one edited file
   invalidates everything (see doc 05).

The **budget system does not save this**, because the budgets are precision budgets, not
resource budgets: `max_steps = 10,000` on a 1 M LOC repo means the solver latches
`BudgetExceeded` almost immediately and *everything* downstream degrades to
`unknown / budget_or_model`. The engine will report honestly that it gave up — which is better
than lying — but the product answer is "polint cannot analyze this repository."

---

## (e) Target performance architecture

Ordered by (benefit ÷ risk). Items 1-4 are mechanical and should land before any further
recall work.

**1. Intern every identity string. `SymbolId(u32)` + a side table.**
Replace `stable_key: String` on facts with a `StableKeyId(u32)` into one `Vec<Box<str>>` +
`HashMap<&str, u32>`. Replace `payload_digest: String` with `[u8; 8]` (the FNV digest at
`incremental/keys.rs:1270` is already a `u64` — it is being formatted to hex and stored as a
`String`). Drop `stable_key_owners`' duplicate key by keying on the interned id.
*Expected: ~465 of ~700 B/fact eliminated → **~2.5× reduction in fact memory** **[estimated]**.*
This is the single highest-leverage change in the codebase and it is largely a type swap.

**2. Fix the quadratic lookups.** `FileId` is dense; add
`AnalysisDb::file(FileId) -> Option<&SourceFile>` doing `self.files.get(id.0 as usize)` and
replace all 12 `db.files().iter().find(...)` sites. Make `cfg/lower_*.rs::operation_evidence`
return `&str` or an interned id instead of `String`. *Mechanical, no behavior change, testable
by snapshot equality.*

**3. Parse once per file, retain a compact per-file IR.** Today the engine drops the AST (right)
and re-parses eleven times (wrong). Extract everything the downstream providers need in the
*existing parallel* `analyze_ts_source_file` pass and emit a compact per-file IR — the shape
`TsFileAnalysis` (`semantic_graph/build.rs:262-269`) already has. Then make the whole-program
passes consume that IR instead of re-parsing. Eliminates 11 of 12 corpus parses and moves the
surviving one under rayon. *This is the largest CPU win available.*

**4. Parallelize the per-file and per-function stages.** Symbol graph
(`symbol_graph/ts.rs:106`), MIR lowering (`lower_ts.rs`/`lower_go.rs`), CFG + dominators, and
metrics are all embarrassingly parallel. Use the exact pattern the two syntax adapters already
prove deterministic: `par_iter().map(...).collect()` followed by
`sort_by(relative_path)`. Add a `--jobs` / `POLINT_JOBS` knob and a `ThreadPoolBuilder` so
polint can be bounded inside a CI container.

**5. Arena the fact tables.** `AnalysisDb` should own a bump arena per fact family, with facts
as `#[repr(C)]` PODs of ids and spans (no owned `String` anywhere) and all text behind interner
ids. That also makes (7) possible.

**6. Parallel + worklist fixpoints.** Replace the fixed-round whole-program structural-equality
fixpoint (`ts_value_flows.rs:315-368`) with a proper worklist keyed on the module dependency
SCCs the module graph already computes. Within an SCC the work is serial; *across* SCCs it is
parallel. Same for `summaries/closure.rs:261` and `summaries/scc.rs:240`.

**7. Spill-to-disk / demand-driven fact access.** The SQLite store exists and has one table.
Give it the fact tables, and make `AnalysisDb` a query facade over (arena ∪ store) rather than a
monolith. Then a whole-program pass can stream a fact family instead of holding it, and
completed providers can drop their `Option<*Store>` once their consumers have run. This is the
only way past ~2 M LOC.

**8. Resource budgets alongside precision budgets.** Add `max_wall_clock`, `max_peak_rss`, and
`max_facts` to `SolverBudget`, and make `max_steps` / `max_outer_iterations` **per-SCC or
per-function**, not per-run — today a big repo exhausts the global counter through sheer size
and every finding degrades. Surface the resource budget in the `unknown_taxonomy` output the
same way the precision budgets already are (`unknown_taxonomy/collect.rs:252-274` is the right
model).

**9. Gate `validate_fact_metadata`.** It is a whole-DB pass with 14 materialized id sets running
unconditionally in production (`analysis_kernel/mod.rs:942`). Make it debug-assert / opt-in
(`--validate-facts`) and keep it always-on in CI.

**10. Kill the `cargo run` rule host for the interactive loop.** Options, in order of
preference: (a) a persistent `polint serve` daemon that keeps the compiled rule host warm and
speaks LSP/JSON-RPC, so the editor path never spawns `cargo`; (b) pre-compiled, distributable
rule packs (a `cdylib` or WASM component) so the analysis engine is not rebuilt per repo;
(c) at minimum, add `[profile.release] lto = "thin"` / `codegen-units = 1` to the workspace and
split the SDK into a thin `polint-sdk` crate so a rules crate does not pull SQLite and two C
parsers.

---

## (f) The CI measurement discipline that should gate merges

### f.1 What exists today

Exactly one merge-blocking performance gate, `.github/workflows/ci.yml:188-192`:

```yaml
- name: semantic-store regression gate (serialized)
  run: >-
    cargo test -p polint --lib --all-features --locked
    eval::bench::gate::tests::semantic_store_boundary::real_store_enabled_measurement_passes_locked_boundary
    -- --exact --ignored --test-threads=1 --nocapture
```

It enforces peak-RSS ≤ 1.20× and cold wall-clock ≤ 1.25× a **paired same-runner control**
(`eval/bench/gate.rs:166-181`), with +16 MB / +50 ms noise floors (`gate.rs:28,36`), on a
**synthetic 512-file tempdir fixture** (256 Go + 256 TS files of 12 trivial functions each,
`gate.rs:531-573`). The methodology — child-process isolation, paired control, explicit noise
floor, fail-not-silent — is **correct**. The scope is a single feature flag on a toy corpus.

What is absent: `polint-bench` is never invoked by any workflow; there is no absolute latency or
memory SLO; no throughput gate; no regression-vs-`main` comparison; no benchmark artifact
tracking; no `timeout-minutes` on any `ci.yml` job (the one occurrence, `release-dry-run.yml:74`,
is on an apt-get step). No criterion/divan/iai/dhat/hyperfine anywhere in the workspace.

### f.2 What should gate merges

**Tier 0 — every PR, blocking, <2 min.** Absolute latency and memory SLOs on a *committed,
version-controlled* synthetic corpus at three sizes (10 K / 100 K / 1 M LOC, generated
deterministically so it costs nothing in git). Assert absolute ceilings, not just ratios:

| Metric | 10 K LOC | 100 K LOC | 1 M LOC |
|---|---|---|---|
| `polint check` cold, syntactic rule set | ≤ 1 s | ≤ 5 s | ≤ 30 s |
| `polint check` cold, `dataflow` rule set | ≤ 3 s | ≤ 20 s | ≤ 180 s |
| Peak RSS, `dataflow` | ≤ 300 MB | ≤ 1.5 GB | ≤ 6 GB |
| Facts / LOC | reported, budgeted | | |
| Full-corpus parse count | **asserted == 1** | | |

That last row is the one that matters most: a test that counts oxc `Parser::new` invocations per
run and fails if it exceeds `file_count` would have caught the re-parse tax the day it was
introduced, and would prevent it recurring.

**Tier 1 — every PR, blocking, cheap.** Complexity assertions that need no timing and so cannot
flake: assert `O(F)` not `O(F²)` by running the same fixture at F and 4F and asserting the
*operation counter* (not wall-clock) grows ≤ 5×. Apply to the syntax-layer restore, the
file-lookup paths, and the value-flow fixpoint.

**Tier 2 — every PR, blocking.** The recall/precision gate that doc 08 says does not exist,
**with the runtime column restored**. Every entry in the F1 trajectory of (a.4) should have been
a CI row of `(F1, wall_clock, peak_rss)` — the runtime column was dropped at iteration 57 and
no recall change since has had a recorded cost. Make F1 and runtime a *joint* gate: a change may
not improve F1 while regressing p95 wall-clock by more than a declared budget, and vice versa.

**Tier 3 — nightly, non-blocking, tracked.** Actually clone the four committed scale manifests
and run `eval::bench::sweep`. The infrastructure is built (`sweep.rs`, `curve.rs`, `report.rs`);
it needs a workflow, a cache of the clones, and `benchmark-curves.json` committed as an artifact
with trend tracking. Right now `research/evaluation-harness/repos/` does not exist and the sweep
silently skips.

**Tier 4 — every release, blocking.** Editor-loop latency: cold and warm `polint check` in a
repo with a `.polint/rules` crate, measured end-to-end including the `cargo run` spawn. Assert a
warm p95 ceiling (target ≤ 200 ms) — this is the number that decides whether polint is usable in
an agent loop, and it is currently unmeasured.

**Cross-cutting rules.**
- Every perf assertion must run in a child process with `getrusage` isolation — `runner.rs`
  already does this correctly; reuse it.
- Every threshold constant gets the treatment `DEFAULT_MAX_PEAK_RSS_RATIO` already has
  (`baseline.rs:196-205`): a default-value test so silently loosening a budget fails CI.
- `polint-bench` must either run in CI or be deleted. Today it compiles, runs against 17
  toy `examples/` dirs of 1-3 files each, and measures `run_rules` with an **empty rule list**
  (`polint-bench/src/lib.rs:102`) — it is unrun infrastructure that creates a false impression
  of coverage.
- No performance number should live in a markdown file or a TOML comment. The 1 GB / 7.4 s
  anchor — the only whole-repo figure polint has — is a comment in
  `devloupe-monorepo-local.toml:8`. That is where measurements go to die.

---

## Cross-references

- Doc 02 (`02-rust-code-quality.md`) independently found the missing interner (D2), the repeated
  TS parses (D4), the `BTreeSet<String>` Go RTA worklist (D3), the unbounded glob cache (D9),
  and the `glob_matches` allocation (D8). This document quantifies them.
- Doc 05 (`05-incrementality-and-store.md`) covers the whole-layer cache key, the absent change
  detection, and the per-fact metadata cost (B4). The re-parse tax and the parallelism gap here
  are the CPU-side counterpart to its memory-side findings.
- Doc 08 (`08-evaluation-and-correctness.md`) covers the absence of any asserted accuracy
  number. Section (f) above argues accuracy and cost must be gated *jointly*, because the
  recorded trajectory in (a.4) is exactly what happens when only one of them is tracked.
</content>
</invoke>
