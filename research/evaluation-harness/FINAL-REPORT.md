# Evaluation Harness Final Report

Date: 2026-05-26

## Scope Correction

polint currently supports Go and TypeScript / JavaScript. Current benchmark
adapters, scorecards, promotion gates, and adapted-run comparisons must cover
only those languages.

Unsupported-language benchmark artifacts should not appear as active manifests,
adapter implementations, promotion gates, or scored comparison tables. They can
be reconsidered only after a future language frontend exists.

## Recommendation

Use a supported-external-plus-native evaluation strategy:

1. Native polint fixtures remain the first promotion gate for internal engine
   behavior: CFG, calls, summaries, data flow, evidence, cache, deterministic
   output, and extension/adaptation deltas.
2. SecBench.js provides the current external TS/JS scanner benchmark.
3. gosec samples provide the current external Go scanner benchmark and gosec
   competitor comparison.
4. Competitor rows should include Semgrep, CodeQL, gosec, or suite-native
   references only when the result applies to the same supported suite.
5. Adapted rows should be produced by a separate adaptation agent with a
   recorded prompt, budget, allowed inputs, forbidden inputs, changed artifacts,
   digests, case-level deltas, and runtime/cache overhead.

## Current Supported Suites

| Suite | Language | Role | Manifest |
|---|---|---|---|
| SecBench.js smoke | TypeScript / JavaScript | Executable package vulnerability smoke benchmark. | `research/evaluation-harness/suites/secbench-js-smoke.toml` |
| gosec samples | Go | Practical Go security samples and gosec comparison baseline. | `research/evaluation-harness/suites/gosec-samples.toml` |
| Native polint fixtures | Go, TypeScript / JavaScript | Engine fact/graph/path/cache/adaptation promotion proof. | `tests/eval-fixtures/` |

## Metrics

Reports should include:

- TP, FP, FN, TN where the suite defines them;
- precision, recall, F1/F2/F3, false-positive rate, and unknown counts;
- graph/path metrics for native fixtures;
- runtime, memory when available, cache reuse, and provider stats;
- extension overhead and accepted/rejected extension facts;
- comparison rows for other products, polint baseline, and polint agent-adapted.

## Public Claim Policy

Public precision claims must cite measured reports from supported suites. Do not
publish aggregate scores that silently mix unsupported languages, adapter-only
runs, or unavailable external clones.
