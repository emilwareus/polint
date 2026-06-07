---
quick_id: 260607-bzh
slug: native-computed-js-recall
status: complete
completed: 2026-06-07
---

# Summary

Implemented bounded JS/TS value-flow support for native object/array operations
and constant computed property keys.

## Result

- Jelly JS/TS callgraph micro moved from **749 TP / 609 FP / 730 FN** to
  **814 TP / 609 FP / 665 FN**.
- Precision moved from **55.15%** to **57.20%**.
- Recall moved from **50.64%** to **55.04%**.
- F1 moved from **52.80%** to **56.10%**.
- `tests/approx/natives.json` moved from **1 TP / 2 FP / 32 FN** to
  **29 TP / 3 FP / 4 FN**.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint ts::tests --lib --locked`
- `cargo test -p polint analysis::mir::lower_ts --lib --locked`
- `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`
- `git diff --check`
- `make lint`
