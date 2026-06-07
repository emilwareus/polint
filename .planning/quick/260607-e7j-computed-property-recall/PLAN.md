---
quick_id: 260607-e7j
slug: computed-property-recall
status: completed
created: 2026-06-07
description: Improve Jelly computed-property recall with bounded key evaluation and accessor flow
---

# Quick Task 260607-e7j: Computed Property Recall

## Objective

Recover the remaining high-value `tests/approx/computedProperties.json` Jelly
edges without broadening to CommonJS/module semantics.

## Scope

- Add bounded computed property key evaluation for string concatenation,
  literal booleans, simple conditionals, and string array index reads.
- Apply computed keys to object literals and class members.
- Separate getter/setter targets from normal property call targets enough to
  recover getter return values and avoid treating accessors as direct callables.
- Add real-pipeline tests derived from Jelly's `computedProperties.js`.

## Verification

- PASS `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- PASS `cargo test -p polint ts::tests --lib --locked`
- PASS `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- PASS release external graph benchmark:
  `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`
- UPDATED `performance/2026-06-06-jelly-gap-closure-research.md`
- PASS `make lint`

## Result

Jelly overall moved from **814 TP / 609 FP / 665 FN** to **840 TP / 604 FP /
639 FN**. Precision moved from **57.20%** to **58.17%**, recall from **55.04%**
to **56.80%**, and F1 from **56.10%** to **57.48%**.

The target fixture `tests/approx/computedProperties.json` moved from **8 TP / 4
FP / 18 FN** to **24 TP / 3 FP / 2 FN**.
