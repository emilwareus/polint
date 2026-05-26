---
status: superseded
phase: 40
plan: "40-02"
updated: 2026-05-26
---

# 40-02 Summary: Supported-Suite Adapter Scope

## Scope Correction

The original adapter-only unsupported-language work has been removed from the
active benchmark implementation. polint benchmarks now cover only languages the
engine supports today:

- Go
- TypeScript / JavaScript

## Active Artifacts

- `crates/polint/src/eval/adapter.rs`
- `crates/polint/src/eval/external/secbench_js.rs`
- `crates/polint/src/eval/external/gosec.rs`
- `research/evaluation-harness/suites/secbench-js-smoke.toml`
- `research/evaluation-harness/suites/gosec-samples.toml`

## Removed Scope

Unsupported-language adapters, manifests, downloaded papers, scorecards, and
adapted-run rows are excluded from current Phase 40 benchmark scope until the
corresponding language frontend exists.
