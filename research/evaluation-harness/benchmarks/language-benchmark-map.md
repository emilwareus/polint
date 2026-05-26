# Supported-Language Benchmark Map

Date: 2026-05-26

polint currently supports Go and TypeScript / JavaScript. Current benchmark
planning, scorecards, promotion gates, and adaptation runs must stay inside
those languages.

## Summary

| Language | Current Suites | Role | Caveat |
|---|---|---|---|
| Go | gosec samples, native polint Go fixtures | Security sample coverage, competitor comparison, and engine fact coverage. | No broad independent scanner benchmark equivalent to SecBench.js. |
| TypeScript / JavaScript | SecBench.js smoke, native polint TS/JS fixtures | Executable vulnerability smoke coverage and engine fact coverage. | Needs repo-local policy fixtures for non-security project conventions. |

## Go

### Use Now

| Benchmark | Measures | Tier |
|---|---|---|
| gosec samples | Practical Go security cases and gosec comparison rows | Fast/nightly/release |
| Native Go fixtures | CFG, calls, summaries, data-flow, evidence, cache, and extension facts | Fast/promotion |

### Go Accuracy Notes

Go has weaker public scanner benchmark coverage than JS/TS. Treat gosec as a
practical competitor/sample source, not complete ground truth. Public claims
should combine gosec sample results with native polint fixtures and explicit
unknown/setup-missing accounting.

## TypeScript / JavaScript

### Use Now

| Benchmark | Measures | Tier |
|---|---|---|
| SecBench.js smoke | Executable server-side JS package vulnerability cases | Fast/nightly/release |
| Native TS/JS fixtures | CFG, calls, summaries, data-flow, evidence, cache, and extension facts | Fast/promotion |
| Jelly-style dynamic comparison | JS/TS call graph recall ideas | Research/reference |
| BugsJS | Project-scale JS regression corpus | Research/reference |

### TS/JS Accuracy Notes

SecBench.js is the strongest current external suite, but it does not replace
repo-local policy fixtures. Adapted-run reports should show whether repo-local
rules/models improve detection without benchmark-specific label leakage.

## Out Of Current Scope

Do not add scored benchmark manifests, adapters, or comparison rows for languages
without a polint frontend. Future language support should introduce its own
benchmark phase and re-open this map at that time.
