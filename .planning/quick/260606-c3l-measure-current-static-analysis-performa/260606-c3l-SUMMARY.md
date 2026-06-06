---
quick_id: 260606-c3l
slug: measure-current-static-analysis-performa
status: complete
completed: 2026-06-06
commit: pending
---

# Quick Task 260606-c3l Summary

Measured current static-analysis performance on the external graph suites and
repo-local CLI scan, then wrote a dated performance report.

## Files Created

- `performance/2026-06-06-static-analysis-performance.md`

## Files Modified

- `crates/polint/src/eval/external/mod.rs`

## Results

- Go x/tools RTA callgraph release tier: 6.67% precision, 2.70% recall.
- Jelly callgraph release tier: 0.00% precision, 0.00% recall.
- Full-workspace `polint check`: ~35.6s, exit 1 due internal diagnostics.

## Verification

- `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture` - passed.
- `target/release/polint check --format json --no-cache` - measured, exit 1 with internal diagnostics.
- `target/release/polint check --format json` - measured, exit 1 with internal diagnostics.
