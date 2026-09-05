# Scale-envelope experiment log

Target: excalidraw v0.17.6 (385 TS/TSX files, 2.5 MB source, 86,527 LOC) full
pipeline (`dataflow` + `file_metrics` + `function_metrics` + `complexity_metrics`)
under **6 GB peak RSS** and **300 s wall**, on this host (8 cores / 15 GB, no swap).

Rule: every experiment gets a hypothesis, a measurement, and a keep-or-revert
verdict. Anything that does not move RSS or wall is reverted the same hour.

## Measurement protocol

- Driver: the eval harness's isolated perf child
  (`eval::bench::runner::tests::perf_child_measure_entry`), release profile.
  This is the same path the committed `scale-corpus-run.json` baseline used.
- Wrapper: `.scale-envelope/rssrun.py` samples whole-process-tree RSS from
  `/proc/<pid>/status` every 200 ms and enforces an `RLIMIT_AS` guard so a
  runaway run cannot take the host down.
- `.polint/cache` in the corpus checkout is deleted before every cold run.
- Load average checked < 2 before every timed run; runs are sequential and
  exclusive.

## Baseline (the number every experiment is measured against)

Two independent cold runs of the excalidraw full pipeline, release, clean cache:

| run | peak RSS | wall | outcome |
|---|---|---|---|
| `baseline-cold` | 9,527,586,816 B (8.873 GB) | 253.0 s | completed |
| `b1-counters` | 9,553,641,472 B (8.898 GB) | 242.0 s | completed |

Peak reproduces to ±0.3 %; wall to ±5 %. **Reference: 8.90 GB / 242 s.**
Final state: 2,393,266 fact-metadata rows, 3,509,568 interned stable keys,
4,026 MB of interned key text (45 % of peak).

---

## X1 — glibc allocator tuning (env only, no code)

**Hypothesis.** A large share of RSS is memory freed by the engine but retained
by glibc's free lists (`type_value_alias` retains 1,885 MB while adding only
471 k facts, which looked like allocator retention rather than live data). If
so, forcing mmap-backed large allocations and aggressive heap trimming should
return it to the OS for free.

**Change.** None. Run with
`MALLOC_TRIM_THRESHOLD_=131072 MALLOC_MMAP_THRESHOLD_=131072 MALLOC_TOP_PAD_=131072 MALLOC_ARENA_MAX=2`.

**Measured.**

| | peak RSS | wall |
|---|---|---|
| baseline | 9,110 MB | 242 s |
| X1 | **9,094 MB** (−0.2 %) | **258 s** (+6.6 %) |

**Verdict: REJECTED.** 0.2 % of peak for a 6.6 % wall regression. The memory is
live data, not allocator retention — which also rules out swapping in
mimalloc/jemalloc as a primary lever, since their advantage over glibc here
would be the same purge/decay behaviour. Nothing was kept; no code was written.
Run cost: one 258 s measurement, and it removed the single most-suggested lead.

## X2 — stop storing private copies of interned key text (memory) + cached-key sorts (wall)

**Hypothesis.** 4,026 MB of the 9,110 MB peak is interned stable-key text
(3.5 M keys, ~1.1 KB each). Three mechanisms multiply it:

1. `EvidenceNodeFact`/`EvidenceEdgeFact` store `source_fact_stable_keys:
   Vec<String>` — a private copy of key text the interner already owns — and
   `EvidenceStore::from_output` then clones each one *again* into
   `by_source_fact_stable_key: BTreeMap<String, Vec<usize>>`. That is three
   copies of a ~1.2 KB string per reference. Same shape for
   `DataFlowEdgeFact::input_stable_keys`.
2. `SemanticGraphBuilder::node_for_key` and friends probe a
   `BTreeMap<StableKeyId, _>` with `interner.intern(key)`. Interning is
   permanent, so every *miss* retains a key forever: `polint.semantic_graph`
   adds 435,051 keys (409 MB) while adding one fact-metadata row.
3. The points-to solver stores relation fragments as
   `BTreeMap<PtVarId, BTreeSet<String>>` — again full key text, copied.

And separately, for wall: the normalize path sorts with
`sort_by(|l, r| (interner.resolve(l.stable_key), l.id).cmp(&(...)))`, which
takes the interner's `RwLock` and clones an `Arc<str>` **twice per comparison**,
i.e. `O(n log n)` lock acquisitions. `polint.type_value_alias` calls
`normalized()` four times and is 44 % of wall.

**Change (byte-identical output by construction).**
- `source_fact_stable_keys` / `summary_stable_key` / `input_stable_keys` and the
  evidence store's index key become `Arc<str>` / `BTreeMap<Arc<str>, _>`,
  sharing the interner's allocation. `Arc<str>` orders by the string it points
  at, so every derived order is unchanged.
- New `StableKeyInterner::lookup(&str) -> Option<StableKeyId>`, a non-inserting
  probe, replaces `intern` at the semantic-graph node-index lookups. A key that
  was never interned cannot be in a map keyed by interned ids, so the result is
  identical.
- Points-to identity fragments hold `Arc<str>`.
- 25 `sort_by(...)` comparators become `sort_by_cached_key(...)`: same total
  order (the key tuples are unique), `O(n)` interner lookups instead of
  `O(n log n)`.
- The eval perf runner keeps only `Result<(), _>` from the cold run instead of
  its whole `KernelOutput`, so the cold `AnalysisDb` is dropped before the warm
  run allocates a second one.

**Measured** (`x2`, cold, clean cache, load 1.4 at start):

| | baseline `b2-ref` | `x2` | delta |
|---|---|---|---|
| peak RSS | 9,551,003,648 B (8.895 GB) | **8,025,931,776 B (7.474 GB)** | **−1.42 GB (−16.0 %)** |
| cold wall | 251.0 s | 250.1 s | −0.4 % (noise) |
| provider output digests | — | **23/23 identical** | output identity proven |

Per-provider retained RSS:

| provider | b2-ref | x2 |
|---|---:|---:|
| polint.data_flow | +643 MB | **+3 MB** |
| polint.evidence | +3,017 MB (peak 9,108) | **+717 MB (peak 7,654)** |
| everything else | unchanged | unchanged |

**Verdict — SPLIT.**

*Kept* (commit `a1ea6336`): `Arc<str>` on
`EvidenceNodeFact::source_fact_stable_keys`,
`EvidenceEdgeFact::source_fact_stable_keys` / `summary_stable_key`,
`DataFlowEdgeFact::input_stable_keys`, and the evidence store's
`by_source_fact_stable_key` index. **−1.42 GB.**

*Kept* (commit `5dc846d0`): the eval harness no longer pins the cold
`AnalysisDb` through the warm run.

*Reverted — measured no-ops:*

| reverted change | measured effect |
|---|---|
| `StableKeyInterner::lookup` (non-inserting probe) at the 5 semantic-graph node-index sites | `polint.semantic_graph` still interned **exactly** 435,051 keys (2,681,511 total, unchanged). The orphan keys are minted by `stable_key_text_from_parts`, which interns internally; probing with `intern` was interning text that was *already* interned. Removed. |
| points-to relation fragments as `Arc<str>` instead of `String` | `polint.type_value_alias` retained +1,886 MB (was +1,885 MB). The fragment sets are not where that stage's memory is. Removed. |
| 25 × `sort_by(|l, r| resolve(l).cmp(resolve(r)))` → `sort_by_cached_key` | `polint.type_value_alias` 104.3 s (was 101.3–107.1 s); `polint.data_flow` 62.5 s (was 62.5–67.4 s). Strictly fewer interner lookups, but not this pipeline's bottleneck. Removed rather than carried. |

The three reverts are the interesting result: they say the remaining cost is
**not** lock traffic, **not** sort comparisons, and **not** private copies in the
points-to solver. It is the interned text itself, and the volume of facts that
mint it.

## X3 — stop interning stable-key text that is only ever used as text

**Hypothesis.** `stable_key_text_from_parts` was implemented as
`interner.resolve(stable_key_from_parts(...)).to_string()` — it interned the key
as a side effect of formatting it. Interning is permanent, so every caller that
only wanted the *text* (to embed in a larger composed key, to feed a payload
digest, to key a local map) retained ~1.1 KB for the whole run. `semantic_stable_key`,
the composition helper used by 35 call sites, is a thin wrapper over it.

**Change.** `stable_key_text_from_parts(family, parts)` builds the canonical text
directly with the existing `write_stable_key_text` buffer writer and no interner.
`semantic_stable_key(family, parts)` follows. Both lose their `interner`
parameter, which cascades into ~20 pure key-text helpers that no longer need one;
the cascade was applied compiler-guided (fix, recompile, repeat until clean), not
by regex. The points-to solver's relation-identity fragments and the access-path
identity in `points_to::constraints` switch to the same non-interning builder.

The text is byte-identical: both paths call `write_stable_key_text`.

**Measured** (`x3`, cold, clean cache):

| | `x2` | `x3` | delta |
|---|---|---|---|
| peak RSS | 8,025,931,776 B (7.474 GB) | **7,758,098,432 B (7.225 GB)** | **−268 MB (−3.3 %)** |
| cold wall | 250.1 s | **241.5 s** | **−8.6 s (−3.4 %)** |
| interned keys | 3,509,568 | **3,140,288** | **−369,280** |
| interned key text | 4,026 MB | **3,768 MB** | **−258 MB** |
| provider output digests | — | **23/23 identical** | |

Per stage, keys no longer interned: `type_value_alias` −154,715,
`semantic_graph` −69,890, `solver` −144,676, `data_flow` −369,280 cumulative.

**Verdict: KEPT**, with a caveat recorded honestly: this is a 29-file diff for a
3.3 % peak win. It is a *net −201 lines* (it deletes an argument from ~90 call
sites) and it removes a real API hazard — a "give me the text" helper that
permanently mutated global state — but if the reviewer prefers a tighter PR this
commit is the one to drop.

## X4 — sub-stage probes (research) + `shrink_to_fit` on digest payloads

**Probe result — this is where the peak actually is.** Temporary
`polint::probe` instrumentation inside `polint.cfg` and `polint.evidence`:

```
stage="cfg"      blocks=56999 functions=4193 reachability=56999
                 dominators=412465 postdominators=334784 control_dependence=28294
stage="evidence" label="start"                       rss_mb=4685 peak_mb=5064
stage="evidence" label="data_flow_evidence"          rss_mb=4854 peak_mb=5064
stage="evidence" label="control_dependence_evidence" rss_mb=5183 peak_mb=5182
stage="evidence" label="normalized"                  rss_mb=5190 peak_mb=5196
stage="evidence" label="digest"                      rss_mb=6538 peak_mb=7502
stage="evidence" label="replace"                     rss_mb=5197 peak_mb=7502
```

Two findings, both actionable:

1. **`evidence_output_digest` is the peak.** It costs **+2,306 MB of peak** on its
   own — more than the facts it describes — because it materialises one ~4 KB
   `format!("{fact:?}")`-derived payload per evidence fact into a single
   `Vec<String>` so the parts can be sorted before hashing. Every provider's
   `*_output_digest` has this shape; evidence is simply the largest.
2. **412,465 dominator + 334,784 post-dominator facts** — 71 % of everything
   `polint.cfg` produces, from 4,193 functions averaging 13.6 blocks. The whole
   relation is derivable from the 52,806-edge immediate-dominator tree.

**`shrink_to_fit` on digest payloads: REJECTED.** Hypothesis was that `String`
doubling slack (each payload grows past its `with_capacity` as stable-key ids
expand into ~1.2 KB of text) was landing on peak.

| | `x3` | `x4` (+shrink) |
|---|---|---|
| peak RSS | 7,758,098,432 B (7.225 GB) | 7,866,929,152 B (7.327 GB) |

Slightly **worse** — a realloc-and-copy per part for no measurable saving.
Reverted, along with the probes.

## X5 — the two fixes the probes pointed at

### X5a — hash provider-output digest parts one fact family at a time

`evidence_output_digest` must hash its parts **in sorted order**, and was
achieving that by collecting every part into one `Vec<String>` and sorting it.
That is what made the digest cost +2.3 GB of peak.

It is avoidable without changing the digest by a byte. Every family's parts
share a `<family>=` prefix; no family name is a prefix of another; and no header
part (`provider_id=…`, `cfg=…`, `extensions=…`) starts with a family prefix, so a
header sorts before *every* part of a family exactly when it sorts before that
family's prefix. Emitting the header parts and the family blocks in that merged
order therefore reproduces `parts.sort()` exactly, with only one family's
payloads live at a time. `family_prefixes_partition_the_sorted_order` asserts
all three properties so a new family or header cannot silently change the digest
of every evidence layer ever produced.

### X5b — bound the materialised dominance relation, and report it

`derive_dominators`/`derive_postdominators` emit one fact per *(dominated,
dominator)* pair — `O(blocks²)` per function, each with a ~1.4 KB key embedding
the function key and **both** block keys. Measured on excalidraw: 412,465 +
334,784 = **747,249 facts, 71 % of everything `polint.cfg` produces**.

Dominance is the reflexive transitive closure of the immediate-dominator tree,
so the closure is derivable, not information. When the worst-case relation size
exceeds `cfg::budget::DEFAULT_MAX_DOMINANCE_PAIRS` (250,000; override with
`POLINT_CFG_MAX_DOMINANCE_PAIRS`, `0` disables) the two families carry the tree
edges only and the run emits a `polint/resource-budget` warning that
`polint unknowns` surfaces as a `budget_exceeded` row.

**Measured** (`x5`, cold, clean cache):

| | `x3` | `x5` | vs baseline `b2-ref` |
|---|---|---|---|
| **peak RSS** | 7,758,098,432 B (7.225 GB) | **5,861,257,216 B (5.459 GB)** | **−3.69 GB, −38.6 %** |
| **cold wall** | 241.5 s | **235.4 s** | **−15.6 s, −6.2 %** |
| interned key text | 3,768 MB | **3,096 MB** | −930 MB |
| interned keys | 3,140,288 | **2,554,247** | −955,321 |

Per stage:

| provider | b2-ref ΔRSS / peak | x5 ΔRSS / peak |
|---|---|---|
| polint.cfg | +1,502 MB / 2,117 | **+526 MB / 1,171** (20.3 s, was 28.0 s) |
| polint.type_value_alias | +1,885 MB / 5,099 | +2,313 MB / **4,074** |
| polint.data_flow | +643 MB / 5,460 | +4 MB / **4,496** |
| polint.evidence | +3,017 MB / **9,108** | +105 MB / **5,589** |

The evidence stage's transient fell from +2,306 MB to **+1,162 MB** — exactly the
"largest single family instead of the sum of all families" the streaming
predicts.

### The A/B that proves what changed

`POLINT_CFG_MAX_DOMINANCE_PAIRS=0` disables the dominance bound, so the *same
binary* can be run with and without it. Both runs, same host, cold cache:

| | pre-change `b2-ref` | `x5-fullrelation` (bound off) | `x5` (bound on, default) |
|---|---|---|---|
| peak RSS | 8,895 MB | **6,400 MB** | **5,591 MB** |
| cold wall | 251.0 s | 242.5 s | 235.4 s |
| provider output digests vs `b2-ref` | — | **23/23 identical** | 11/23 (cfg + downstream) |
| `polint check` diagnostics | — | 6 | 7 (`+polint/resource-budget`) |

Read the middle column carefully: **with the dominance bound off, every one of the
23 provider output digests is byte-identical to the pre-change engine.** The
`Arc<str>` sharing, the non-interning key-text builder and the streamed digest
are provably identity-preserving — they are not "probably fine", they reproduce
the same facts bit for bit while removing 2.5 GB of peak.

The remaining 0.8 GB comes from the dominance bound, which *is* a semantic
change, is confined to `cfg_dominators` / `cfg_postdominators`, and is reported.
It is also what takes the run from 6.25 GB (over the ceiling) to 5.46 GB (under).

## X6 — replace the data-flow projection's whole-vector scans with key indexes

**Hypothesis.** `polint.data_flow` is 22–26 % of wall and does three
whole-vector scans per insert: `summary_edges.rs::summary_node` scans every
`DataFlowNodeFact` (of which `derive_local_place_nodes` has already pushed one
per MIR place) to find a node by stable key, `push_edge` scans every edge, and
`local.rs::projection_edge_kind` scans the whole MIR place table per projection
edge. All three are `O(n²)` in corpus size and all three are replaceable with a
key index maintained alongside the output.

`local.rs::push_edge` additionally re-scanned `output.edges` *after* the
`emitted_edges` set already answered the same question — and the two are kept in
sync by the only path that removes edges — so that scan is redundant, not just
slow.

**First attempt: WRONG, and the identity oracle caught it.** The first version
indexed the lookups in `summary_node` / `event_node` but forgot to insert the
newly created node into the index, so a repeated key minted a duplicate node,
`replace_data_flow_facts` rejected the output on duplicate stable keys, and
`polint.evidence` was dependency-blocked. The run still "succeeded" and reported
a very attractive **4.09 GB / 194.8 s** — which is exactly the shape of a
measurement that is fast because it stopped doing the work. The per-provider
digest comparison flagged it immediately:

```
DIFFER   polint.data_flow: 2eef57019918c121 -> -
MISSING  polint.evidence
21/23 provider output digests identical  (2 differ)
```

That is the whole argument for carrying an identity oracle alongside a
performance number.

**Measured after the fix** (`x6`, cold, clean cache):

| | `x5` | `x6` | |
|---|---|---|---|
| peak RSS | 5,861,257,216 B (5.459 GB) | **5,844,377,600 B (5.443 GB)** | −17 MB (noise) |
| cold wall | 235.4 s | **222.8 s** | **−12.6 s, −5.4 %** |
| `polint.data_flow` | 60.9 s | **53.4 s** | −7.5 s |
| provider output digests vs `x5` | — | **23/23 identical** | |
| `polint check` diagnostics digest | `dfd881fcfd3bd80f` | `dfd881fcfd3bd80f` | identical |

**Verdict: KEPT** — a wall-only win, identity-preserving, one file.

---

# Final result

| | before (`b2-ref`) | after (`x6`) | |
|---|---:|---:|---|
| **peak RSS** | 9,551,003,648 B (**8.895 GB**) | 5,869,457,408 B (**5.466 GB**) | **−38.5 %** — under the 6 GB ceiling |
| **cold wall** | 251.0 s | **234.8 s** | **−6.5 %** — under the 300 s budget |
| interned key text | 4,026 MB | 3,096 MB | −930 MB |
| interned keys | 3,509,568 | 2,554,247 | −955,321 |
| fact-metadata rows | 2,393,266 | 1,735,237 | −658,029 (the dominance closure) |

## Note on running the suite locally

`cargo test -p polint --lib --all-features --locked` behaves very differently
depending on whether `research/evaluation-harness/repos/` exists. The bench
sweep "skips absent checkouts" — so with the scale corpus fetched, the suite
starts measuring **excalidraw through a debug build** inside a unit test. CI
never has those checkouts, so the suite reported here was run with the corpus
moved aside, which is the CI configuration.

That is worth knowing independently of this change: anyone who runs
`make fetch-scale-repos` and then `cargo test` locally will find the suite
apparently hanging.


## Final verification (clean rebuild of the branch tip)

| run | peak RSS | cold wall | identity |
|---|---:|---:|---|
| `x6` | 5,844,377,600 B (5.443 GB) | 222.8 s | 23/23 vs `x5` |
| `final` | 5,869,457,408 B (**5.466 GB**) | **234.8 s** | 23/23 vs `x6`, diagnostics digest `dfd881fcfd3bd80f` unchanged |

Run-to-run spread on this host is ~0.4 % on peak and ~5 % on wall. Both runs are
inside the envelope; the reported figure is the worse one.

## hugo (stretch, honest reporting)

`gohugoio/hugo` @ `3f35721f`, same host, same command, cold cache.

**It does not run at all, before or after.** `fs::load_analysis_files_scoped`
reads sources with `fs::read_to_string`, so one non-UTF-8 file aborts the whole
run before any provider executes:

```
isolated perf child measurement: failed to read
  research/evaluation-harness/repos/gohugoio-hugo/media/testdata/fake.js: invalid utf-8
```

Exactly one file in hugo's tree is non-UTF-8 (`media/testdata/fake.js`, a
deliberate fixture). With it moved aside:

| | |
|---|---:|
| files loaded | 871 |
| source bytes | 5,866,703 |
| **peak RSS** | 7,263,526,912 B (**6.765 GB**) |
| **cold wall** | **714.5 s** |
| providers that ran | **16 of 23** |
| `budget.iteration_capped` | 20,829 |

Both budgets are exceeded, and the number is a **lower bound**: this host has no
Go toolchain, so hugo's Go semantic layer is unavailable and `identity`,
`reachability`, `semantic_graph`, `solver`, `refined_calls`, `data_flow` and
`evidence` were dependency-blocked. On excalidraw those seven providers account
for a further ~1.7 GB.

The shape is still informative:

| provider | ms | wall % | retained | peak |
|---|---:|---:|---:|---:|
| polint.cfg | 160,066 | 22.4 % | +1,800 MB | 2,692 MB |
| **polint.type_value_alias** | **499,604** | **70.0 %** | **+3,103 MB** | 6,927 MB |

`type_value_alias` is 70 % of hugo's wall and the largest memory owner — the same
stage that is now 45 % of excalidraw's wall. It is the clear next target, and
nothing in this change touched it.
