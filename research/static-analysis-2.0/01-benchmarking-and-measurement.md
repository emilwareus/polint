# 01 — Benchmarking & Measurement

## Problem

Every accuracy number we have comes from 76 Jelly micro fixtures (JS/TS) and
5 Go x/tools fixtures. There is no real-application benchmark, no
F1-vs-repo-size curve, no memory/latency regression gate, and no telemetry on
budget exhaustion — so the two stated problems ("not accurate enough and not
fast enough on large apps") are invisible in our own harness. Worse, the
solver's honesty budgets (`max_tokens_per_cell: 64`, `max_steps: 2M` in
`crates/polint/src/analysis/calls/js_points_to/solver.rs:219-228`) degrade
recall silently on exactly the repos that matter.

## Current state in polint

- Adapter-based external harness: `crates/polint/src/eval/external/`
  (`jelly_callgraph.rs`, `go_x_tools_callgraph.rs`, stubs for `gosec.rs`,
  `secbench_js.rs`), TOML manifests in `research/evaluation-harness/suites/`,
  pinned repos, `POLINT_WRITE_GRAPH_BENCH=1` writes baselines to
  `.context/graph-benchmarks/`.
- Metrics: TP/FP/FN, P/R/F1, runtime; `eval/performance.rs` exists but no
  peak-RSS or budget-event reporting per case.
- The solver already emits `budget_reasons: BTreeSet<String>`
  (`solver.rs:240`) — it is just never surfaced in benchmark reports.
- Methodology strength worth keeping: pinned oracles, revert-on-regression
  discipline, determinism gates, "public claims cite measured reports only".

## What the research says

- **Real-app JS/TS oracle**: the Jelly PLDI 2024 evaluation ("Reducing Static
  Analysis Unsoundness with Approximate Interpretation",
  https://dl.acm.org/doi/10.1145/3656424, artifact
  https://zenodo.org/records/10930752) uses **NodeProf dynamic call-graph
  traces on 141 real Node.js projects** (36 with full dynamic CGs). This is
  the de-facto recall oracle for JS; static-only Jelly scores ~75.9% recall
  there, 88.1% with the dynamic pre-pass. Our micro-suite 89% F1 is *not*
  comparable to these numbers.
- **Cross-tool calibration**: "Static JavaScript Call Graphs: a Comparative
  Study" (https://arxiv.org/abs/2405.07206) — ACG F≈95 / Closure 85 / TAJS 82
  on SunSpider-class programs; only ACG and Closure survive multi-file
  inputs. Useful for competitor columns in reports.
- **Ground-truth caveat** (applies to us and to all ML-for-CG work, see
  07): dynamic traces under test execution **under-approximate** — an edge
  not exercised is not a false positive. MSR 2024
  (https://arxiv.org/abs/2402.07294) shows benchmark choice flips
  conclusions. We already hit this with the x/tools partial `WANT` oracle
  (FPs that are real edges) and unexercised true edges in Jelly.
- **Root-cause accounting**: "Automatic Root Cause Quantification for Missing
  Edges in JavaScript Call Graphs" (ECOOP 2022,
  https://arxiv.org/abs/2205.06780) — dynamic property access is the #1
  recall killer, ahead of eval/reflection. Our FN decomposition should keep
  using this taxonomy so numbers are comparable to literature.
- **Industry practice**: Google Tricorder (ICSE 2015,
  https://research.google/pubs/pub43322/) gates analyzers on measured
  usefulness and latency budgets per diff — the model for our regression
  gates.

## Direction for polint

1. New adapter `real_app_callgraph` beside `jelly_callgraph.rs`; corpus:
   5–10 pinned apps from the Jelly PLDI'24 artifact (NodeProf oracles
   included) spanning sizes (10k → 1M+ LOC with deps), plus 2–3 real Go
   repos scored against `golang.org/x/tools` callgraph (RTA) output.
2. Per-case telemetry: peak RSS, wall-clock, **budget-exhaustion events**
   (plumb `budget_reasons` into `eval/metrics.rs` reports), unknown-fact
   counts.
3. Standing curves: F1-vs-size and RSS-vs-size snapshots; gate all 2.0 work
   on "curve does not get worse".
4. Keep the micro suites as unit-level regression nets, but stop treating
   micro-F1 as the optimization target.
5. Score with *two* oracle modes where possible: dynamic-trace (recall
   floor, precision suspect) and curated required-edges (precision floor) —
   report both, never blend.

## References

Jelly PLDI'24 + artifact (above) · NodeProf: https://github.com/Haiyang-Sun/nodeprof.js
· comparative study arXiv:2405.07206 · ECOOP'22 root causes arXiv:2205.06780
· MSR'24 arXiv:2402.07294 · Tricorder ICSE'15 · x/tools callgraph:
https://pkg.go.dev/golang.org/x/tools/cmd/callgraph
