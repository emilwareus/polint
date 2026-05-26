# Repository Index

Local clones are under `research/evaluation-harness/repos/`, which is gitignored.
They are research inputs, not vendored product dependencies.

## Current Supported Benchmark Repositories

| Repository | Local Path | Commit Checked | Language | Primary Use | Notes |
|---|---|---:|---|---|---|
| Go x/tools | `repos/golang-tools` | `7743a285e3d261ca235408e013ec5c14cb5170e4` | Go | Primary Go callgraph benchmark source. | Adapter reads `go/callgraph/rta/testdata/*.txtar` `WANT` call-edge assertions. |
| Jelly | `repos/jelly` | `b799ed4f0d68c670fe398830aaa51dd5c628cf74` | TypeScript / JavaScript | Primary JS/TS callgraph benchmark source. | Adapter reads suite-native call graph JSON `fun2fun` and `call2fun` edges. |
| SecBench.js | `repos/SecBench.js` | `bc3156219138` | TypeScript / JavaScript | Executable server-side package vulnerability benchmark. | Local clone contains 704 `.test.js` exploit/test files when present. |
| gosec | `repos/gosec` | `de65614d10a6` | Go | Go security analyzer samples and competitor baseline. | Useful test cases and taint/performance scripts; not broad independent ground truth. |

## Engine Accuracy Research

- [`GRAPH-ENGINE-BENCHMARK-RESEARCH.md`](GRAPH-ENGINE-BENCHMARK-RESEARCH.md)
  describes the scanner engine work needed to improve Go and JS/TS graph
  benchmark precision, recall, unknown handling, and runtime.

## Repositories From Prior Supported-Language Research Reused Here

| Repository | Local Path | Language | Why It Still Matters |
|---|---|---|---|
| CodeQL | `research/data-flow/repos/codeql` | Go and TypeScript / JavaScript slices only | Query tests can inspire microcases, but expected outputs reflect CodeQL modeling choices. |
| BugsJS Dataset | `repos/BugsJS-bug-dataset` | JavaScript | Project-scale regression corpus; not direct static-analysis ground truth. |

## Scope Policy

Do not add current benchmark manifests, adapters, reports, or scorecard rows for
languages polint cannot parse today. If a future phase adds a new language
frontend, its benchmarks can be reintroduced in that phase.

## Cloning Policy

Do not commit cloned benchmark repositories.

Commit only:

- adapter code;
- pinned source URLs and commits;
- small manifest files;
- generated summaries;
- downloaded supported-scope papers where allowed by the existing research convention.

The benchmark source itself should stay outside git history to avoid repository
bloat, license ambiguity, and accidental vendoring.
