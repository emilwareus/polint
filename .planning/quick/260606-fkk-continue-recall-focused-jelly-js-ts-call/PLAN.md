# Quick Task: Continue Recall-Focused Jelly JS/TS Callgraph Improvements

Date: 2026-06-06

## Goal

Continue from the 16.16% Jelly JS/TS callgraph F1 checkpoint and target recall
specifically, while keeping new edges routed through normal polint analysis
facts instead of benchmark-only scoring paths.

## Starting Point

| Suite | TP | FP | FN | Precision | Recall | F1 | Hash |
|---|---:|---:|---:|---:|---:|---:|---|
| Jelly JS/TS callgraph micro | 134 | 45 | 1345 | 74.86% | 9.06% | 16.16% | `9d351e5eb129ce84` |

## Analysis Focus

Highest remaining recall blockers:

- `tests/helloworld/app.json`: dependency/module execution and unavailable local
  source files in the current checkout.
- Native callback families: iterators, promises, arrays, `Array.from`, `flatMap`,
  `call`/`apply`/`bind`.
- Class/prototype/object-property flow.
- Function value flow through variables, returns, properties, and parameters.
- Exact Jelly call-site span normalization around parenthesized expressions.

## Plan

1. Classify remaining false negatives by case and capability family.
2. Prefer recall changes that emit normal call-target/refined-call facts.
3. Start with low-blast-radius native callback models for inline callback
   arguments because they can recover true edges without full object flow.
4. Add targeted Rust tests before each semantic expansion.
5. Measure the release external graph benchmark after every iteration.
6. Append progress to
   `performance/2026-06-06-jelly-gap-closure-research.md`.

## Verification

- `cargo check -p polint --locked`
- `cargo test -p polint lower_ts --lib --locked`
- `cargo test -p polint analysis::calls --lib --locked`
- `cargo test -p polint eval::external::jelly_callgraph --lib --locked`
- `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`
