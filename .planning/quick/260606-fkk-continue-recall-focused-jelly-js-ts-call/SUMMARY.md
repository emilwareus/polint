# Summary: Continue Recall-Focused Jelly JS/TS Callgraph Improvements

Date: 2026-06-06

## Status

Implemented and measured.

## Baseline

Starting checkpoint:

| Suite | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Jelly JS/TS callgraph micro | 134 | 45 | 1345 | 74.86% | 9.06% | 16.16% | 7808 ms | `9d351e5eb129ce84` |

## Current Finding

The naive native-callback host-edge model was the wrong abstraction: it added no
true positives and increased Jelly false positives from 45 to 95, so it was
reverted. Jelly primarily rewards the later call sites where function values
flow into variables, object properties, destructuring bindings, rest parameters,
and collection elements.

Implemented a bounded same-file TS/JS value-flow model for:

- array/set/map collection elements and `for...of` bindings;
- collection callback parameters for iterator-style methods;
- `new Set(...)`, `new Map(...)`, `Array.from(...)`, and mapper returns;
- explicit object-literal function properties and `Array.from` `thisArg`;
- array destructuring, rest slices, numeric index reads, and indexed calls;
- direct same-file function parameter/rest-argument flow;
- object destructuring and object rest parameter flow.
- Promise executor/resolve/reject and `then`/`catch` value chains.
- explicit Jelly target-file dependency inclusion in the benchmark harness.
- class/static/prototype/self-alias value flow.
- async IIFE, `await`, and async function return value flow.
- module-level `this` assignment and object-literal `this` alias flow.

Final measured checkpoint:

| Suite | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Jelly JS/TS callgraph micro | 462 | 552 | 1017 | 45.56% | 31.24% | 37.06% | 81650 ms | `bd04d1cfb14c1da5` |

Go stayed unchanged at 37 TP / 6 FP / 0 FN, 92.50% F1.

The remaining recall gap is now dominated by CommonJS dependency/module object
semantics, Promise result objects, async generators, receiver side effects,
broader function-object/return flow, and exact Jelly span normalization. The
dependency-inclusive harness fix improved recall but also exposed major FP and
runtime cost in `tests/helloworld/app.json`.

## Verification

Passed:

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint analysis::calls --lib --locked`
- `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`
