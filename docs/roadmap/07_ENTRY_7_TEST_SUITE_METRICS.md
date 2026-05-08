# Entry 7: Test Suite Metrics

## Goal

Turn raw test facts into reusable, cross-language test quality metrics.

## Why

Rules should not have to reinvent common test-quality logic. Agents and CI need
simple evidence about whether risky code has meaningful tests.

## Difficulty

**M for Go**, **M/L for TS/JS**, **M/L for Python later**, **M/L for Java
later**.

## What To Build

- `TestMetricFact`
- `RelatedTestEvidence`
- `TestFramework`
- `RuleCtx::test_metrics_for_file`
- `RuleCtx::test_metrics_for_function`

## Build Method

1. Add normalized test metric facts over existing `TestFact`.
2. For Go, aggregate assertion count, subtests, table rows, evidence terms, and
   related production files.
3. For TS/JS, detect Jest/Vitest/Mocha `describe`, `it`, `test`, assertions,
   table tests, and snapshot calls.
4. Keep framework-specific evidence optional.
5. Expose normalized metrics for rules.
6. Add docs and examples for test-quality policies.

## Done When

- Go exposes reusable metrics from current `TestFact`.
- TS/JS exposes basic Jest/Vitest/Mocha metrics.
- Rules can query related test evidence by file/function.
- Metrics clearly state heuristic limits.

## Later Languages

- Python should support pytest and unittest.
- Java should support JUnit and TestNG.
