# Quick Task 260830-nzd: Polint Algorithmic Scan Optimization - Summary

**Date:** 2026-08-31
**Status:** Implemented and benchmarked
**Pull request:** https://github.com/emilwareus/polint/pull/104

## Completed

- Replaced whole-source span conversion scans with retained line-start indexes.
- Reused native Go/TS syntax output identities instead of rebuilding fallback
  digests when persistence is disabled.
- Built one run-scoped canonical metrics context and reused provider-owned
  output identities at the scheduler boundary.
- Added immutable dense per-file fact-view indexes, borrowed range/sparse
  iterators, and direct insertion-order function metric lookup.
- Removed the file-cache `serde_json::Value` materialization and second
  serialization while retaining JSON validation and atomic path-safe writes.
- Re-profiled the retained implementation with temporary environment-gated
  instrumentation and reverted it cleanly after selecting H12.

## Final benchmark matrix

| Consumer | Tier | v0.3.2 (s) | Final (s) | Delta | Speedup |
|---|---:|---:|---:|---:|---:|
| Plinty | warm | 2.373 | 2.013 | -15.2% | 1.18x |
| Plinty | cold | 8.907 | 3.062 | -65.6% | 2.91x |
| Plinty | nocache | 9.706 | 2.654 | -72.7% | 3.66x |
| OAIZ | warm | 4.651 | 4.306 | -7.4% | 1.08x |
| OAIZ | cold | 6.901 | 5.281 | -23.5% | 1.31x |
| OAIZ | nocache | 8.123 | 4.477 | -44.9% | 1.81x |

All 18 final samples returned zero and matched the required consumer output
hashes. The measured result is a substantial cold/nocache improvement, not a
literal 10x end-to-end speedup.

## Evidence-based cuts

- H5 Go traversal fusion: residual theoretical wall saving is below roughly
  0.3 seconds and requires the highest-risk fact/order rewrite.
- H6 TS visitor fusion: post-H1 extraction-pass totals have only a
  low-millisecond ideal wall ceiling.
- H13 pathological splitting: the worst file no longer dominates either
  consumer's Go/TS stage.
- H7 structural facts: engine-only facts cannot move the benchmark packs
  without the consumer rule migration excluded from this task.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `PATH=/opt/data/home/.local/bin:$PATH CARGO_INCREMENTAL=0 cargo test --workspace --all-features --locked`
- `cargo build --release -p polint --all-features --locked`
- Sequential six-cell A/B matrices after every retained wave
- Final matrix: `/workspace/bench2-results/results-final.tsv`
- Mission history: `/workspace/bench2-results/mission-LOG.md`

Generated rule-host test targets were scoped-cleaned after every gate. Free
space never fell below 48 GiB during the completed continuation.
