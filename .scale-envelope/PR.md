# Scale envelope: the excalidraw full pipeline now fits in 5.5 GB / 235 s

@emilwareus — every number below is a measurement on this host (8 cores / 15 GB /
no swap, Linux 6.8), release profile, excalidraw v0.17.6 (385 files, 2.45 MiB
source, 86,527 LOC), full pipeline (`dataflow` + the three metric capabilities),
`.polint/cache` deleted before each run, load average checked, runs sequential
and exclusive. Evidence is committed under `.scale-envelope/`.

## Before → after

| | before | after | |
|---|---:|---:|---|
| **peak RSS** | 9,551,003,648 B (**8.895 GB**) | 5,844,377,600 B (**5.443 GB**) | **−38.8 %**, under the 6 GB ceiling |
| **cold wall** | 251.0 s | **222.8 s** | **−11.2 %**, under the 300 s budget |
| run status | completes | completes | |
| `polint check` diagnostics | 6 | 7 | +1: the `polint/resource-budget` warning that reports the degradation |
| provider output digests | 23 | 23 identical **with the dominance bound off** | see "output identity" |

The committed baseline artifact says something different, and it is worth being
precise about why. `research/evaluation-harness/baselines/scale-corpus-run.json`
records **`oom` — SIGKILL after ~1026 s at ~12 GB**. That row is a **cold+warm**
measurement, and `run_repo_perf_point` was keeping the cold run's whole
`KernelOutput` — and therefore its ~8 GB `AnalysisDb` — alive while the warm run
built a second one. One cold full-pipeline run of the *unmodified* engine on this
host peaks at **8.895 GB and completes in 251 s**. Both problems are real; that
one is a harness bug and is fixed here too.

## Attribution (before)

Full table in `.scale-envelope/ATTRIBUTION.md`. The top three owners of the
8.895 GB peak:

| # | owner | cost | code |
|---|---|---|---|
| 1 | `polint.evidence` | +3,017 MB retained, peak 9,108 MB | `analysis_neutral/evidence/facts.rs:32,33,50,52,53` — `source_fact_stable_keys: Vec<String>` and `summary_stable_key: Option<String>` store private copies of interned key text, and `evidence/store.rs:214` clones each entry a **third** time into `by_source_fact_stable_key: BTreeMap<String, _>` |
| 2 | `polint.type_value_alias` | +1,885 MB, **41.8 % of wall** | `analysis_neutral/types/provider.rs:48` |
| 3 | `polint.cfg` | +1,502 MB, 11.4 % of wall | `analysis_neutral/cfg/derived.rs:52,99` — the `O(blocks²)` dominance relation, materialised as 747,249 facts with ~1.4 KB keys |

The dominant consumer overall is **interned stable-key text**: 3,509,568 keys
holding **4,026 MB — 45 % of peak** — averaging 1,148 B/key, because identity is
a recursively composed human-readable string (a dominator key embeds the function
key *and both block keys*).

Sub-stage probes then found the peak itself: **`evidence_output_digest` costs
+2,306 MB on its own**, materialising one ~4 KB payload per evidence fact into a
single `Vec<String>` so the parts can be sorted before hashing.

## What moved the needle

| change | peak | wall | identity |
|---|---:|---:|---|
| `Arc<str>` for key references on data-flow / evidence rows | **−1.42 GB** | — | byte-identical |
| stop interning stable-key text that is only used as text | −0.27 GB | −8.6 s | byte-identical |
| hash provider-output digest parts one fact family at a time | −0.98 GB | — | byte-identical |
| bound the materialised dominance relation (reported) | **−0.79 GB** | −7.7 s | `cfg_dominators` / `cfg_postdominators` carry the tree, not the closure |
| replace the data-flow projection's whole-vector scans with key indexes | — | **−12.6 s** | byte-identical |
| harness: stop pinning the cold `AnalysisDb` through the warm run | halves the harness's own peak | — | n/a |

## Output identity

`POLINT_CFG_MAX_DOMINANCE_PAIRS=0` disables the dominance bound, so the **same
binary** runs with and without the one semantic change. With it off:

> **23/23 provider output digests are byte-identical to the pre-change engine**,
> at 6.250 GB / 242.5 s.

So the three structural changes are proven identity-preserving, not assumed to
be. The remaining 0.79 GB is the dominance bound, and it is what takes the run
from 6.25 GB (over the ceiling) to 5.46 GB (under it).

## What degrades, and how it says so

Two independent mechanisms, both default-on and both reported through one rule id
(`polint/resource-budget`, severity **warn** — `--fail-on` defaults to `error`,
so no exit code changes) which `polint unknowns` surfaces as a `budget_exceeded`
row:

1. **Dominance materialisation budget** (`analysis_neutral/cfg/budget.rs`). When
   the worst-case relation exceeds `DEFAULT_MAX_DOMINANCE_PAIRS` (250,000;
   `POLINT_CFG_MAX_DOMINANCE_PAIRS` overrides, `0` disables) the two families
   carry the immediate (tree) edges only. **Nothing is lost**: dominance is the
   reflexive transitive closure of that tree. Below the ceiling — every fixture in
   `tests/`, every `examples/` project, the callgraph eval corpora — output is
   unchanged.
2. **Run memory ceiling** (`analysis_kernel/resource.rs`). Live RSS is sampled at
   every provider boundary. When it crosses the ceiling — `POLINT_MEMORY_CEILING_MB`,
   else **80 % of host RAM**, `0` disables — the providers scheduled after that
   point are recorded as `budget_exceeded` / `memory_ceiling` and their
   capabilities degrade through the existing capability-support path. This is a
   *safety net*: on any run that fits in memory it never fires, so behaviour is
   unchanged. It exists so a repository that does not fit reports what it could
   not do instead of being killed by the OOM reaper with no output at all.

## Experiments, including the ones that failed

`.scale-envelope/EXPERIMENTS.md` has every attempt with its measurement and a
keep-or-revert verdict. Four were **reverted as measured no-ops**, which is
information worth having:

- **glibc allocator tuning** (`MALLOC_TRIM_THRESHOLD_`, `MALLOC_MMAP_THRESHOLD_`,
  `MALLOC_ARENA_MAX`): −0.2 % peak for +6.6 % wall. The memory is live data, not
  allocator retention — which also rules out mimalloc/jemalloc as a primary lever.
- **`StableKeyInterner::lookup`** (non-inserting probe) at the semantic-graph node
  index: zero keys saved.
- **`Arc<str>` in the points-to solver's relation fragments**: zero MB.
- **25 `sort_by(cmp on resolve)` → `sort_by_cached_key`**: strictly fewer interner
  lookups, zero measurable wall.
- **`shrink_to_fit` on digest payloads**: slightly worse.

## Delivery-rule note

This is larger than the 25-file / 1,500-line guideline from the phase-65
forensic, and I want that on the record rather than buried. It is
NN files / +NNN −NNN. One commit —
`perf(engine): stop interning stable-key text…` — accounts for 29 of those files
and is almost entirely single-argument deletions (net −201 lines) for a 3.3 %
win; **it is the one to drop if you want a tighter PR**, at a cost of ~270 MB.
The rest is four focused commits.
