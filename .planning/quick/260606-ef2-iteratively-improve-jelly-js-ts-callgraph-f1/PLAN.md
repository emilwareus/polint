# Quick Task: Iteratively Improve Jelly JS/TS Callgraph F1

Date: 2026-06-06

## Goal

Improve the external Jelly JS/TS callgraph benchmark F1 as much as practical in
one focused loop while preserving polint's architecture, private analysis
boundaries, deterministic behavior, and runtime discipline.

## Baseline

Baseline after the Go RTA oracle fix:

| Suite | TP | FP | FN | Precision | Recall | F1 |
|---|---:|---:|---:|---:|---:|---:|
| Jelly JS/TS callgraph micro | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% |

## Plan

1. Measure after each change with the release external graph benchmark.
2. Fix the most fundamental missing JS/TS callgraph owner first: top-level
   module execution.
3. Add a narrow anonymous callable bridge for IIFEs so expression-position
   function/arrow callees have stable identities.
4. Align function identity spans with callable expression spans where the
   frontend had been using wider variable declarator spans.
5. Lower `new` expressions as constructor call operations through the normal MIR
   call path.
6. Keep synthetic module ownership from leaking into normal metrics.
7. Correct benchmark normalization where it is clearly set semantics or input
   selection, without filtering the oracle to make the score look better.
8. Record every measured iteration in
   `performance/2026-06-06-jelly-gap-closure-research.md`.

## Non-goals

- Do not port Jelly wholesale.
- Do not add public SDK facts for internal callgraph plumbing.
- Do not filter expected Jelly oracle edges by local source availability.
- Do not start broad CommonJS, class/prototype, or native callback modeling in
  this quick task.

## Verification

- `cargo test -p polint lower_ts --lib --locked`
- `cargo check -p polint --locked`
- `cargo test -p polint analysis::calls::extract --lib --locked`
- `cargo test -p polint eval::external::jelly_callgraph --lib --locked`
- `cargo test -p polint metrics --lib --locked`
- `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`
