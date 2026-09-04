# Scale-envelope attribution — excalidraw v0.17.6, full pipeline

**Measured**, not estimated. Every number below comes from a run on this host
(8 cores / 15 GB RAM / no swap, Linux 6.8), release profile, cold cache.

## Corpus

| | |
|---|---|
| Repo | `excalidraw/excalidraw` @ `0dbd2a39` (tag `v0.17.6`) |
| Files the pipeline loads | **385** |
| Source bytes loaded | **2,566,051** (2.45 MiB) |
| Published LOC (`.ts/.tsx/.js/...`) | 86,527 |
| Capabilities | `dataflow`, `file_metrics`, `function_metrics`, `complexity_metrics` (`AnalysisPlan::full_pipeline_for_test`, `crates/polint/src/analysis_plan.rs:336`) |

## How it was measured

- Driver: `eval::bench::runner::tests::perf_child_measure_entry` in the release
  test binary — the same isolated perf child that produced the committed
  `research/evaluation-harness/baselines/scale-corpus-run.json`.
- Per-stage rows come from a new stage trace emitted at every provider boundary
  in `run_scheduled_providers` (`crates/polint/src/analysis_kernel/mod.rs`),
  enabled with `RUST_LOG=polint::kernel::stage=info`. `rss_mb` is live RSS from
  `/proc/self/statm`; `peak_rss_mb` is `getrusage(RUSAGE_SELF).ru_maxrss`.
- Whole-process peak is independently sampled every 200 ms by
  `.scale-envelope/rssrun.py` (agrees with `ru_maxrss` to within 1 MB).
- `.polint/cache` deleted before the run; load average 1.36 at start.

## Headline: the committed baseline overstates the engine by ~2x

`scale-corpus-run.json` records **`oom` — SIGKILL after ~1026 s at ~12 GB**.
That row is a **cold+warm** measurement. `run_repo_perf_point`
(`crates/polint-eval/src/harness/bench/runner.rs:98-118`) stores the cold run's
whole `KernelOutput` — which owns the entire ~8.3 GB `AnalysisDb` — in
`cold_result` **while the warm run builds a second `AnalysisDb`**. Two complete
fact databases are live simultaneously at the warm run's peak.

A single full-pipeline run on the same host and corpus:

| | Baseline artifact (cold+warm) | Measured (one cold run) |
|---|---|---|
| Peak RSS | ~12 GB → **SIGKILL** | **8.87 GB** (9,527,586,816 B) |
| Wall | ~1026 s (killed) | **253 s** |
| Status | oom | completed |

So the engine's true single-run envelope is **8.87 GB / 253 s**. Both halves of
the problem are real — 8.87 GB is still 48 % over the 6 GB ceiling — but the
harness's double retention is a genuine and separately fixable multiplier.

## Stage attribution (one cold run, 251.5 s of provider time)

`d_rss_MB` is RSS **retained** across the stage (live RSS after − before);
`peak_MB` is the process high-water mark at the end of the stage, so a stage
whose `peak_MB` jumps above its own `rss_MB` had a transient spike.

| provider | ms | wall % | rss after (MB) | **retained ΔMB** | peak (MB) |
|---|---:|---:|---:|---:|---:|
| polint.source | 0 | 0.0 % | 23 | 0 | 22 |
| polint.go.syntax | 0 | 0.0 % | 23 | 0 | 22 |
| polint.ts.syntax | 101 | 0.0 % | 41 | 17 | 40 |
| polint.module_graph | 156 | 0.1 % | 56 | 14 | 55 |
| polint.symbol_graph | 5,130 | 2.0 % | 337 | 281 | 559 |
| polint.module_topology | 126 | 0.1 % | 342 | 4 | 559 |
| polint.semantic_mir | 7,920 | 3.1 % | 616 | 273 | 806 |
| **polint.cfg** | **28,783** | **11.4 %** | 2,118 | **1,502** | 2,117 |
| polint.calls | 4,053 | 1.6 % | 2,214 | 95 | 2,240 |
| polint.go.semantic | 6 | 0.0 % | 2,214 | 0 | 2,240 |
| polint.identity | 364 | 0.1 % | 2,214 | 0 | 2,240 |
| polint.abstract_domains | 3,675 | 1.5 % | 2,598 | 384 | 2,605 |
| polint.direct_summaries | 572 | 0.2 % | 2,599 | 0 | 2,605 |
| polint.entrypoints | 17 | 0.0 % | 2,599 | 0 | 2,605 |
| polint.reachability | 13 | 0.0 % | 2,599 | 0 | 2,605 |
| polint.extensions | 1 | 0.0 % | 2,599 | 0 | 2,605 |
| **polint.type_value_alias** | **105,115** | **41.8 %** | 4,484 | **1,885** | 5,100 |
| polint.semantic_graph | 6,143 | 2.4 % | 4,674 | 189 | 5,100 |
| polint.solver | 1,219 | 0.5 % | 4,678 | 3 | 5,100 |
| polint.refined_calls | 9,915 | 3.9 % | 4,678 | 0 | 5,100 |
| **polint.data_flow** | **63,683** | **25.3 %** | 5,451 | **772** | 5,459 |
| **polint.evidence** | **14,094** | **5.6 %** | 8,320 | **2,868** | **9,085** |
| polint.metrics | 364 | 0.1 % | 8,325 | 4 | 9,085 |
| **TOTAL** | **251,450** | 100 % | | **8,325** | **9,085** |

Also recorded by the run: `budget.iteration_capped = 8387` — the abstract-domain
solver already latches its iteration ceiling thousands of times at this size,
and `.polint/cache` reaches **202 MB** on disk.

## The dominant memory consumer: interned stable-key TEXT

A second instrumented run (`b1-counters`, 8.898 GB peak / 242 s — the peak
reproduces to within 0.3 %) adds fact and interner counters at every stage
boundary:

| provider | ms | dRSS MB | peak MB | fact-meta rows | interned keys | **key text MB** |
|---|---:|---:|---:|---:|---:|---:|
| polint.source | 0 | 0 | 22 | 385 | 385 | 0 |
| polint.ts.syntax | 99 | 16 | 39 | 19,416 | 19,416 | 2 |
| polint.module_graph | 150 | 14 | 54 | 29,454 | 29,454 | 6 |
| polint.symbol_graph | 5,147 | 281 | 558 | 214,667 | 224,675 | 79 |
| polint.semantic_mir | 8,278 | 274 | 805 | 375,522 | 506,779 | 229 |
| **polint.cfg** | 27,989 | 1,502 | 2,117 | 1,429,469 | 1,438,844 | **1,474** |
| polint.calls | 4,306 | 95 | 2,240 | 1,465,596 | 1,476,463 | 1,540 |
| polint.identity | 343 | 0 | 2,240 | 1,487,842 | 1,498,709 | 1,543 |
| polint.abstract_domains | 3,656 | 384 | 2,605 | 1,625,299 | 1,620,098 | 1,605 |
| polint.direct_summaries | 600 | 0 | 2,605 | 1,648,019 | 1,642,818 | 1,613 |
| **polint.type_value_alias** | 107,142 | 1,885 | 5,100 | 2,120,632 | 2,246,460 | **2,105** |
| polint.semantic_graph | 5,770 | 189 | 5,100 | 2,120,633 | 2,681,511 | 2,514 |
| polint.solver | 1,110 | 4 | 5,100 | 2,127,074 | 2,936,150 | 2,709 |
| polint.refined_calls | 7,591 | 0 | 5,100 | 2,147,480 | 2,956,556 | 2,756 |
| **polint.data_flow** | 53,995 | 782 | 5,460 | 2,384,149 | 3,193,228 | **3,240** |
| **polint.evidence** | 13,892 | 1,667 | **9,110** | 2,384,149 | 3,500,451 | **4,024** |
| polint.metrics | 296 | 0 | 9,110 | 2,393,266 | 3,509,568 | 4,026 |

**3,509,568 interned stable keys holding 4,026 MB of text — 45 % of the 9,110 MB
peak, 57 % of final live RSS. Average key length: 1,148 bytes.**

Keys are that large because identity is a *recursively composed, human-readable,
length-prefixed* string (`analysis_api/metadata.rs:491` `write_stable_key_text`).
A CFG dominator key embeds the function key **and both block keys**, and each
block key embeds the MIR body key, which embeds the function key, which embeds
the repo-relative path. Key text therefore grows multiplicatively with
composition depth:

| stage | new keys | new key text | **bytes/key** |
|---|---:|---:|---:|
| ts.syntax | 19,031 | 2 MB | ~110 |
| symbol_graph | 205,259 | 73 MB | ~360 |
| semantic_mir | 282,104 | 150 MB | ~530 |
| **cfg** | **932,065** | **1,245 MB** | **~1,400** |
| type_value_alias | 602,392 | 492 MB | ~820 |
| semantic_graph | 435,051 | 409 MB | ~940 |
| solver | 254,639 | 195 MB | ~765 |
| data_flow | 237,072 | 484 MB | ~2,040 |
| **evidence** | **307,223** | **784 MB** | **~2,550** |

Two structural observations fall straight out of this table:

1. `polint.semantic_graph` interns **435,051 keys while adding 1 fact-meta row**,
   and `polint.solver` interns 254,639 while adding 6,441 — roughly **600 MB of
   key text that no retained fact references**. Interned text is never released,
   so transient composite keys are a permanent leak by construction.
2. `polint.cfg` alone mints 932 k keys because `derive_dominators` /
   `derive_postdominators` emit one fact per *(dominated, dominator)* pair —
   `O(blocks²)` per function — and each pair's key carries two full block keys.

Interner *overhead* (Arc header, allocator rounding, the `keys` vector and the
`ids` map) is only ~72 B/key ≈ 250 MB. The 4.0 GB is the text itself. So the
levers are **key count** and **key length**, not interner layout.

## Top 3 memory owners

### 1. `polint.evidence` — +2,868 MB retained in 14.1 s (34 % of final RSS)

`crates/polint/src/analysis_neutral/evidence/facts.rs:13` —
`EvidenceNodeFact` carries

```rust
pub compact_label: Option<String>,
pub source_fact_stable_keys: Vec<String>,
```

and every construction site fills it by **re-materializing interned key text as
owned `String`s**:

- `crates/polint/src/analysis_neutral/evidence/provider.rs:243-246`
  `source_fact_stable_keys: vec![ interner.resolve(dependence.stable_key).to_string(),
  interner.resolve(controlling_edge.stable_key).to_string() ]`
- same pattern at the data-flow-derived node/edge builders
  (`provider.rs:121`, `:158`).

Stable keys here are 150–300 byte length-prefixed identity strings. The
interner (`crates/polint/src/internal_core/stable_key.rs:33`) already holds each
one exactly once as an `Arc<str>`; the evidence layer stores a **second, private
copy per reference**, plus a `Vec` header per fact. Evidence is a 1:1 projection
of the data-flow graph, yet it retains 3.7× what data flow itself retains
(2,868 MB vs 772 MB) — the ratio is the string duplication.

`derive_control_dependence_evidence` (`provider.rs:195-260`) additionally does
**five** whole-vector linear scans per control-dependence fact
(`db.cfg_edges().iter().find`, `db.cfg_blocks().iter().find`, and
`db.cfg_functions().iter().find` **three times**), which is why the stage costs
14 s despite doing no analysis.

### 2. `polint.type_value_alias` — +1,885 MB retained, 105.1 s (41.8 % of wall)

`crates/polint/src/analysis_neutral/types/provider.rs:48-115`. One stage does
Go + TS type/value/access-path derivation, extension merges, points-to
constraint derivation, the points-to fixpoint, and alias answers — and calls
`output.normalized(interner)` **four separate times** (`:80`, `:85`, `:114`,
plus the merge outputs). Each `normalized` re-sorts every fact family with

```rust
rows.sort_by_cached_key(|row| interner.resolve(row.stable_key))
```

`StableKeyInterner::resolve` (`internal_core/stable_key.rs:90`) takes the
process-global `RwLock` and clones an `Arc<str>` per row, so each normalize is
`O(n)` lock acquisitions + atomic ref-count pairs on top of an `O(n log n)`
**string** comparison sort. There are **56 `sort_by_cached_key` sites** in the
crate on this exact pattern. This is simultaneously the single largest wall
consumer and the second largest memory consumer.

### 3. `polint.cfg` — +1,502 MB retained, 28.8 s (11.4 % of wall)

`crates/polint/src/analysis_neutral/cfg/derived.rs:52` (`derive_dominators`) and
`:99` (`derive_postdominators`) materialize the **full transitive dominance
relation as facts** — one `DominatorFact` per *(dominated, dominator)* pair,
i.e. `O(blocks²)` per function, and again for post-dominance. Every such fact
gets a freshly built stable key composed of the function key **plus both block
keys** (`derived.rs:81-88`), roughly 300 bytes of unique interned text per
*pair*. `derived.rs:490-496` then makes it worse:

```rust
interner.intern(semantic_stable_key(interner, family, parts).into_string())
```

— `semantic_stable_key` already interns and resolves the key, `.into_string()`
copies it out, and `intern` hashes and looks it up a second time. Two hash
lookups and one wasted full-string allocation per fact.

`derive_cfg_with_cache_stats` (`cfg/provider.rs:35-38`) also calls
`normalized()` twice around `append_derived_rows`, re-sorting families that
`derive_dominators` already sorted.

## Runners-up

| Owner | Cost | Code path |
|---|---|---|
| `polint.data_flow` | +772 MB, **63.7 s** | `data_flow/summary_edges.rs:292` `output.nodes.iter().find(...)` and `:247` `output.edges.iter().any(...)` — linear scans over the whole node/edge vectors on **every** insert, i.e. `O(n²)`; `data_flow/local.rs:347` scans all MIR places per projection; `local.rs:338` `output.edges.retain(...)` inside a per-operation loop. Every edge also carries `evidence: Vec<String>` built with `format!` (`summary_edges.rs:73-78`). |
| `polint.abstract_domains` | +384 MB, 3.7 s | 8,387 iteration-capped observations already recorded |
| `polint.symbol_graph` | +281 MB, 5.1 s | `ts/symbol_graph.rs:110-118` — serial per-file loop, one oxc parse per file, on one core |
| `polint.semantic_mir` | +273 MB, 7.9 s | `ts/mir/lower.rs:54` — serial per-file loop; `:58` `lowering.places.clone()`; `:59-62` builds a `BTreeMap<String, PlaceId>` holding an owned copy of every place key |

## Parallelism

Confirmed unchanged from doc 06: **4 rayon sites**, none in `analysis_neutral`
(`fs/mod.rs:146`, `ts/adapter.rs:354`, `go/adapter.rs:348`, `core/rule.rs:363`).
Everything from `polint.module_graph` onward — **251 s of the 251.5 s total** —
runs on one core. `symbol_graph`, `semantic_mir`, the CFG derivations and
`metrics` are structurally per-file or per-function and are serial today.
