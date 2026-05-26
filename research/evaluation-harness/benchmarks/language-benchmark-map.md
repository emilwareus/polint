# Supported-Language Benchmark Map

Date: 2026-05-26

polint currently supports Go and TypeScript / JavaScript. Current benchmark
planning, scorecards, promotion gates, and adaptation runs must stay inside
those languages.

## Summary

| Language | Current Suites | Role | Caveat |
|---|---|---|---|
| Go | Go x/tools RTA callgraph, gosec samples, native polint Go fixtures | Primary graph benchmark, security sample coverage, competitor comparison, and engine fact coverage. | Go x/tools RTA is call-edge focused; CFG/dataflow still need native polint goldens. |
| TypeScript / JavaScript | Jelly callgraph micro, SecBench.js smoke, native polint TS/JS fixtures | Primary graph benchmark, executable vulnerability smoke coverage, and engine fact coverage. | Jelly JSON gives suite-native call graph edges; CFG/dataflow still need native polint goldens. |

## Go

### Use Now

| Benchmark | Measures | Tier |
|---|---|---|
| Go x/tools RTA callgraph | Go-native call-edge expectations from official `golang.org/x/tools` RTA fixtures | Fast/nightly/release |
| gosec samples | Practical Go security cases and gosec comparison rows | Fast/nightly/release |
| Native Go fixtures | CFG, calls, summaries, data-flow, evidence, cache, and extension facts | Fast/promotion |

### Go Accuracy Notes

Go graph claims should lead with Go x/tools RTA callgraph results, then use
native polint fixtures for CFG, direct calls, summaries, data-flow, evidence,
and cache behavior. Treat gosec as a secondary security benchmark, not the main
accuracy story.

## TypeScript / JavaScript

### Use Now

| Benchmark | Measures | Tier |
|---|---|---|
| Jelly callgraph micro | Suite-native JS/TS call graph edge expectations from Jelly JSON outputs | Fast/nightly/release |
| SecBench.js smoke | Executable server-side JS package vulnerability cases | Fast/nightly/release |
| Native TS/JS fixtures | CFG, calls, summaries, data-flow, evidence, cache, and extension facts | Fast/promotion |
| BugsJS | Project-scale JS regression corpus | Research/reference |

### TS/JS Accuracy Notes

Jelly callgraph micro is the primary current external graph suite for JS/TS.
SecBench.js remains useful for vulnerability detection, but it does not measure
graph shape. Adapted-run reports should show whether repo-local models improve
call-edge recall/precision without benchmark-specific label leakage.

## Out Of Current Scope

Do not add scored benchmark manifests, adapters, or comparison rows for languages
without a polint frontend. Future language support should introduce its own
benchmark phase and re-open this map at that time.
