# Scale envelope: the excalidraw full pipeline now fits in 5.4 GB / 223 s

@emilwareus — every number below is a measurement on this host (8 cores / 15 GB /
no swap, Linux 6.8), release profile, excalidraw v0.17.6 (385 files, 2.45 MiB
source, 86,527 LOC), full pipeline (`dataflow` + the three metric capabilities),
`.polint/cache` deleted before each run, load average checked, runs sequential
and exclusive. The raw stage traces and 200 ms RSS timelines are committed under
`.scale-envelope/runs/`. The "after" column is the `final` run: a clean rebuild
of the exact commit at the tip of this branch. Two runs of that code measured
**5.443 GB / 222.8 s** and **5.466 GB / 234.8 s**; the table quotes the worse of
the two.

## Before → after

| | before | after | |
|---|---:|---:|---|
| **peak RSS** | 9,551,003,648 B (**8.895 GB**) | 5,869,457,408 B (**5.466 GB**) | **−38.5 %** — under the 6 GB ceiling |
| **cold wall** | 251.0 s | **234.8 s** | **−6.5 %** — under the 300 s budget |
| interned stable keys | 3,509,568 | 2,554,247 | −955,321 |
| interned key text | 4,026 MB | 3,096 MB | −930 MB |
| fact-metadata rows | 2,393,266 | 1,735,237 | −658,029 |
| `polint check` diagnostics | 6 | 7 | +1: the warning that reports the degradation |

The committed artifact says something different, and it is worth being precise
about why. `research/evaluation-harness/baselines/scale-corpus-run.json` records
**`oom` — SIGKILL after ~1026 s at ~12 GB**. That row is a **cold+warm**
measurement, and `run_repo_perf_point` was keeping the cold run's whole
`KernelOutput` — and therefore its ~8 GB `AnalysisDb` — alive while the warm run
built a second one. One cold full-pipeline run of the *unmodified* engine on this
host peaks at **8.895 GB and completes in 251 s**. Both problems are real; that
one is a harness bug and is fixed here too.

## Attribution (before)

Full table and method in `.scale-envelope/ATTRIBUTION.md`. Top three owners of
the 8.895 GB peak:

| # | owner | cost | code |
|---|---|---|---|
| 1 | `polint.evidence` | +3,017 MB retained, peak 9,108 MB | `analysis_neutral/evidence/facts.rs:32,33,50,52,53` — `source_fact_stable_keys: Vec<String>` stores a private copy of interned key text, and `evidence/store.rs:214` clones each entry a **third** time into `by_source_fact_stable_key: BTreeMap<String, _>` |
| 2 | `polint.type_value_alias` | +1,885 MB, **41.8 % of wall** | `analysis_neutral/types/provider.rs:48` |
| 3 | `polint.cfg` | +1,502 MB, 11.4 % of wall | `analysis_neutral/cfg/derived.rs:52,99` — the `O(blocks²)` dominance relation, 747,249 facts with ~1.4 KB keys |

The dominant consumer overall is **interned stable-key text**: 3,509,568 keys
holding **4,026 MB — 45 % of peak** — averaging 1,148 B/key, because identity is
a recursively composed human-readable string (a dominator key embeds the function
key *and both block keys*).

Sub-stage probes then found the peak itself: **`evidence_output_digest` costs
+2,306 MB on its own**, materialising one ~4 KB payload per evidence fact into a
single `Vec<String>` so the parts can be sorted before hashing.

## What moved the needle

| commit | change | peak | wall |
|---|---|---:|---:|
| `a1ea6336` | `Arc<str>` for key references on data-flow / evidence rows | **−1.42 GB** | — |
| `ca44d78e` | stop interning stable-key text that is only used as text | −0.27 GB | −8.6 s |
| `0005aac9` | hash provider-output digest parts one fact family at a time | **−0.83 GB** | — |
| `65473dbc` | bound the materialised dominance relation (reported) | **−0.81 GB** | −7.7 s |
| `c482edc7` | index the data-flow projection instead of rescanning | — | **−12.6 s** |
| `5dc846d0` | harness: stop pinning the cold `AnalysisDb` through the warm run | halves the harness's own peak | — |

## Output identity

`POLINT_CFG_MAX_DOMINANCE_PAIRS=0` disables the one semantic change, so the
**same binary** runs with and without it. With it off:

> **23/23 provider output digests byte-identical to the pre-change engine**,
> at 6.250 GB / 242.5 s.

The three structural changes are therefore *proven* identity-preserving, not
assumed to be. The remaining 0.81 GB is the dominance bound, and it is what takes
the run from 6.25 GB (over the ceiling) to 5.44 GB (under it).

## What degrades, and how it says so

Two mechanisms, both default-on, both reported through one rule id
(`polint/resource-budget`, severity **warn**; `--fail-on` defaults to `error`, so
no exit code changes) that `polint unknowns` surfaces as a `budget_exceeded` row.

**1. Dominance materialisation budget** (`analysis_neutral/cfg/budget.rs`). When
the worst-case relation exceeds `DEFAULT_MAX_DOMINANCE_PAIRS` (250,000;
`POLINT_CFG_MAX_DOMINANCE_PAIRS` overrides, `0` disables), `cfg_dominators` and
`cfg_postdominators` carry the immediate (tree) edges only. **Nothing is lost** —
dominance is the reflexive transitive closure of that tree. Below the ceiling
(every fixture in `tests/`, every `examples/` project) output is unchanged.

```
$ POLINT_CFG_MAX_DOMINANCE_PAIRS=1 polint inspect unknowns
  category  budget_exceeded
  reason    control-flow dominance materialisation bounded: worst-case 1340 pairs
            against a 1 pair ceiling. `cfg_dominators` and `cfg_postdominators`
            carry the immediate (tree) edges only; the full relation is their
            reflexive transitive closure.
```

**2. Run memory ceiling** (`analysis_kernel/resource.rs`). Live RSS is sampled at
every provider boundary. When it crosses the ceiling — `POLINT_MEMORY_CEILING_MB`,
else **80 % of host RAM**, `0` disables — the providers scheduled after that point
are recorded as `budget_exceeded` / `memory_ceiling` and their capabilities
degrade through the existing capability-support path. This is a *safety net*: the
ceiling is 12 GB on this 15 GB host and the run peaks at 5.4 GB, so it never
fires. It exists so a repository that does **not** fit reports what it could not
do instead of being SIGKILLed with no output.

```
$ POLINT_MEMORY_CEILING_MB=1 polint inspect unknowns
  category  budget_exceeded
  reason    memory ceiling reached after `polint.source`: 12 MiB resident against
            a 1 MiB ceiling (configured). Providers scheduled after it were
            skipped and their capabilities degraded; set POLINT_MEMORY_CEILING_MB
            to raise or disable the ceiling.
```

## Verification

- `cargo test -p polint --lib --all-features --locked`: **2,436 passed / 22 failed
  / 14 ignored**. The same suite on `main` @ `828905f1` in a scratch worktree:
  **2,425 passed / 22 failed / 14 ignored**, and the two failure sets are
  **identical** (`diff` of the sorted names is empty). All 22 are Go fixtures on a
  host with no `go` toolchain. The branch adds 11 net passing tests.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: clean.
- Per-provider output digests and a `polint check` diagnostics digest are compared
  on every measurement run; the comparison is what caught a genuine bug in the
  data-flow indexing (a missing index insert silently dropped `polint.evidence`
  and made the run look 25 % faster).

## Experiments, including the ones that failed

`.scale-envelope/EXPERIMENTS.md` records every attempt with its measurement and a
keep-or-revert verdict. Five were **reverted as measured no-ops**:

- **glibc allocator tuning** (`MALLOC_TRIM_THRESHOLD_`, `MALLOC_MMAP_THRESHOLD_`,
  `MALLOC_ARENA_MAX`): −0.2 % peak for +6.6 % wall. The memory is live data, not
  allocator retention — which also rules out mimalloc/jemalloc as a primary lever.
- **`StableKeyInterner::lookup`** (non-inserting probe) at the semantic-graph node
  index: zero keys saved.
- **`Arc<str>` in the points-to solver's relation fragments**: zero MB.
- **25 × `sort_by(cmp on resolve)` → `sort_by_cached_key`**: strictly fewer
  interner lookups, zero measurable wall.
- **`shrink_to_fit` on digest payloads**: slightly worse.

## Delivery-rule note — please read before reviewing

This is **50 source files, +1,518 / −957** against the phase-65 guideline of 25
files / 1,500 lines, and I would rather flag that than bury it. Per commit
(source only):

| commit | files | +/− | |
|---|---:|---|---|
| `16e8e330` research: attribution + stage trace | 5 | +111 / −8 | |
| `a1ea6336` `Arc<str>` key references | 14 | +269 / −50 | −1.42 GB |
| `5dc846d0` harness cold-DB retention | 1 | +14 / −9 | |
| `ca44d78e` non-interning key text | **26** | +195 / **−512** | −0.27 GB |
| `0005aac9` streamed evidence digest | 1 | +191 / −60 | −0.83 GB |
| `65473dbc` dominance budget | 4 | +241 / −16 | −0.81 GB |
| `c482edc7` data-flow indexes | 2 | +78 / −38 | −12.6 s |
| `75564e56` memory ceiling | 5 | +429 / −284 | safety net |
| `1bd52c4e` fmt + diagnostics marker | 6 | +24 / −14 | |

`ca44d78e` alone is 26 of the 50 files and is a **net −317 lines** — it deletes
one argument from ~90 call sites — for a 3.3 % win. **If you want a tighter PR,
that is the commit to drop**; it costs ~270 MB and the run still lands at
~5.7 GB, under the ceiling. Everything else is four focused commits.

## Not done, and why

- **hugo** — cannot be measured at all, before or after, and the reason is worth
  knowing: `fs::load_analysis_files_scoped` uses `fs::read_to_string`, so a
  single non-UTF-8 source file aborts the entire run.
  `media/testdata/fake.js` — one file out of hugo's whole tree, deliberately
  invalid — kills it at source load, before any provider runs:
  `failed to read .../media/testdata/fake.js: invalid utf-8`. Doc 05 §B2 flagged
  that this read is unbounded; it is also non-lossy-decoding. A repository does
  not get to be un-analysable because it contains one deliberately corrupt
  fixture. Numbers with that one file moved aside are below; note this host has
  no Go toolchain, so hugo's Go semantic layer is unavailable regardless.
- **`polint.type_value_alias` is now 45 % of wall** (99 s). Its four
  `normalized()` passes and the points-to fixpoint are the next lever; I measured
  the obvious sort fix and it did nothing, so this needs its own attribution pass
  rather than a guess.
- **The `Vec<String>` digest pattern exists in every provider**, not just
  evidence. `data_flow` and `type_value_alias` still pay a few hundred MB of
  transient each. The same family-streaming applies; I stopped at the one that was
  the measured peak.
- **`cargo test -p polint --lib` becomes unusable once
  `research/evaluation-harness/repos/` is populated** — the bench sweep stops
  skipping and starts measuring excalidraw through a *debug* build inside a unit
  test. CI never has those checkouts. Worth a guard, separately.
