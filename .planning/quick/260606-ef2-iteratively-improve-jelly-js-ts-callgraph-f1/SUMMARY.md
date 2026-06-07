# Summary: Iteratively Improve Jelly JS/TS Callgraph F1

Date: 2026-06-06

## Result

Jelly JS/TS callgraph F1 improved from **1.07%** to **16.16%**.

| Suite | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Jelly JS/TS callgraph micro | 134 | 45 | 1345 | 74.86% | 9.06% | 16.16% | 7808 ms | `9d351e5eb129ce84` |
| Go x/tools RTA | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% | unchanged | `f9c8f398e133e64b` |

## Implemented

- Added a synthetic private TS/JS module function owner and lowered top-level
  program statements into a module MIR body.
- Filtered the synthetic module owner from metrics and metrics cache inputs.
- Added stable anonymous callable names for expression-position function and
  arrow callees, including IIFEs.
- Classified anonymous callable evidence as a lexical callee for direct call
  extraction.
- Switched variable-initialized function identities to the function/arrow
  expression span instead of the full variable declarator span.
- Lowered TS/JS `new` expressions as constructor call operations so existing
  call extraction and direct resolution can produce constructor graph edges.
- Included existing source files listed by Jelly cases, not only entry files.
- Deduplicated normalized Jelly observed graph edges as set semantics.
- Updated `performance/2026-06-06-jelly-gap-closure-research.md` with the
  measured iteration log and remaining blockers.

## Verification

Passed:

- `cargo test -p polint lower_ts --lib --locked`
- `cargo check -p polint --locked`
- `cargo test -p polint analysis::calls::extract --lib --locked`
- `cargo test -p polint eval::external::jelly_callgraph --lib --locked`
- `cargo test -p polint metrics --lib --locked`
- `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`

## Remaining Gap

The next score jump needs real callable value flow, exact parenthesized call span
normalization, and object/module/native semantics. The current bridge recovers
module-level direct calls, IIFEs, expression-span targets, and constructors, but
does not yet model function values moving through variables, object properties,
classes/prototypes, CommonJS/ESM exports, or ECMAScript callback APIs.
