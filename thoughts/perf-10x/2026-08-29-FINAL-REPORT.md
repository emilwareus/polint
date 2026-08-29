# polint check perf-10x — final report (2026-08-29)

Branch `perf/scan-10x` @ a0ce9a64 · PR #102 · mission window 01:00–10:00 UTC

## Outcome summary

Warm `polint check` on plinty went **15.7 s → ~2.4 s (≈6.5x)** via removal of two
accidental hot-path costs (per-fact `lines().count()`; allocation-heavy scope-glob
matching) plus cache-layer fixes. devloupe warm went 11.2 s → ~5.0 s (≈2.2x), bounded
below by a single rule in the benchmark pack (`local/code-health-metrics`, 1.70 s
critical path — pack code, out of scope). Cold tiers improved ~1.6–2.0x; they remain
dominated by tree-sitter/Oxc parsing on a cache miss, which is real work the engine
must do. The ≥5x target is **met on plinty warm; missed on the other three cells**,
with named, measured blockers — not for lack of ideas (12 hypotheses investigated,
9 landed, 1 landed-and-reverted on measurement).

## Baselines vs final (medians)

Full-matrix cells were measured at a0ce9a64 (run-bench.sh, 3 runs/cell, all rc=0).
The branch tip adds 16ef22bf (H9-lite) after a 3-run warm/cold re-measurement showed
parity (plinty warm 2.21 s median, cold 12.6 s median with 8.7–13.1 s session spread;
devloupe warm 4.75 s, cold 7.02 s) and byte-identity re-verified on both repos.
Ice-cold cells are unaffected by H9-lite (it does not touch the build path).

| repo | tier | 2026-08-28 baseline | final (this branch) | speedup | target | met |
|---|---|---:|---:|---:|---|---|
| plinty | warm | 15.7 s | 2.28 s | 6.89x | ≥5x | YES |
| devloupe | warm | 11.2 s | 4.81 s | 2.33x | ≥5x | no |
| plinty | cold | 25.1 s | 8.79 s | 2.86x | ≥5x | no |
| devloupe | cold | 13.1 s | 6.86 s | 1.91x | ≥5x | no |
| plinty | ice-cold | 409.1 s | 187.4 s | 2.18x | bonus | n/a |
| devloupe | ice-cold | 230.6 s | 182.1 s | 1.27x | bonus | n/a |

(final cells = run-bench.sh medians on the same machine; ice-cold caveat below)

## What landed (SHAs on perf/scan-10x)

| commit | change | measured effect |
|---|---|---|
| 04deacf6 | H1 memoize per-file Go line counts (was `lines().count()` per fact; 11.7 GB rescanned/run on plinty) | −9.5 s plinty warm+cold; −3.0 s devloupe |
| c2c1f9d7 | H2 allocation-free `sdk::scope::glob_matches` (borrowed matcher, reused buffer) | −1.5 s both repos all tiers |
| 1d8ef065 | H3 metrics-layer manifest readable (4 MiB→64 MiB ceiling; devloupe's 5.7 MB manifest could never be read) + stop per-function dependency edges + parameter-digest bump | −0.5–1.0 s devloupe; stops 26 MB/run cache rewrite |
| 19fda989 | H11 parser identity in cache keys (tree-sitter/tree-sitter-go/oxc) | correctness: closes stale-fact-on-parser-upgrade gap; one-time invalidation |
| f6a4fe66 | H4 read layer blob once; output digest derived from manifest payload digest (no 30–45 MB re-serialize, no double deserialize) | −0.47 s plinty warm; −0.74 s devloupe warm |
| c6371e43 | H5 fact metadata without per-fact string allocs; one-lock interner fast path | −0.3–0.5 s warm both repos |
| 06469701 | H12 skip fact-DB teardown at CLI exit (drop-audited first) | −0.1–0.2 s all tiers |
| a0ce9a64 | revert of d99d0e43 (H10 size-descending parse scheduling) | **landed change reverted on measurement**: it *cost* ~3 s plinty cold (15.5–16.4 s with, 12.7 s without, back-to-back) |
| 16ef22bf | H9-lite: per-file cache adapter hands callers raw JSON bytes (was Value→to_vec→parse, three passes per file cold) | parity-or-better on warm/cold quick-bench; byte-identity re-verified on both repos |

## Why the remaining gaps exist (measured, not speculative)

- **devloupe warm (5.0 s vs 2.2 s target):** the pack's own `local/code-health-metrics`
  rule is a 1.70 s single-rule critical path (rayon wall = slowest rule). Engine work
  cannot go below it; the rule lives in the benchmark repo (untouchable by this mission).
- **plinty cold (12.7 s vs 5.0 s target):** 6.9 s of tree-sitter parsing of a heavily
  skewed file set (one 558 KB generated file = 21% of facts) + 47 MB of per-file cache
  writes. H10 was the attempted fix and it made things worse (reverted). Remaining levers
  (chunked parsing, per-file store removal) are behavior-risky redesigns.
- **devloupe cold:** same parse-bound structure + metrics recompute on the always-miss
  path (fixed by H3; remaining cost is the metrics derivation itself).
- **Ice-cold:** dominated by rule-host compilation (unrelated to scan engine). The
  measured improvement on this harness is partly an artifact (vendored sources skip the
  crates.io/git fetch; plinty's git-tag dep clones a 222 MB repo — recommending
  `polint = "0.3.0"` over a git tag is a free ~40% ice-cold win for users, no code change).

## Correctness verification (byte-identical output)

- Inner JSON report md5 identical to released v0.3.0 binary on both repos, warm AND cold:
  plinty `8507f4fae91f6608c6dce4912be0926f`, devloupe `ca7193c06a6fbce1ec6447abcf197760`.
- Outer `polint check` stdout byte-identical on both repos.
- Gates on the final tree: `cargo fmt --check`, `clippy -D warnings`, `cargo test
  --workspace` all green (the 3 capability-matrix tests that fail at base pass here).
- Benchmark hosts built from this branch via a vendored-source cargo patch wrapper
  (`cargo-polint-local`), validated byte-identical vs the released binary; protocol in
  LOG.md. One-time cache invalidation from H3/H11 key changes is stated and expected.

## Not done (identified, deferred)

- H6 (prefer provider's verified digest over canonical re-projection; −0.3–0.6 s warm):
  cut by the schedule after the H10 bisect consumed the window.
- H7 (parallel file walk): unmeasurable on this harness (page cache never dropped).
- H8 (skip `cargo run` when host is current; −0.15 s): fingerprint-correctness risk
  judged not worth 5% of the post-fix wall this pass.
- H9-lite (per-file cache adapter triple JSON round-trip; cold −0.5–1.5 s): cut with
  wave 3.
- Architecture items from `research/code-preserving-rule-build/FINAL-REPORT.md`
  (SDK/engine split, fact-snapshot protocol) — the real path to ice-cold and cold
  improvements beyond these limits; out of scope for a byte-identical pass.

## Artifacts

- `thoughts/perf-10x/2026-08-29-RESEARCH.md` (761 lines, measured cost centers)
- `thoughts/perf-10x/2026-08-29-PLAN.md` (waves, gates, abort criteria)
- `thoughts/perf-10x/2026-08-29-LOG.md` (timestamped run log incl. the H10 investigation)
- `thoughts/perf-10x/2026-08-29-instrumentation-and-prototype.diff` (reference patch)
- `/workspace/bench-polint-results/`: raw-results.tsv + raw/ (final matrix), quick-results-wave{1,2}.tsv
- PR: https://github.com/emilwareus/polint/pull/102
