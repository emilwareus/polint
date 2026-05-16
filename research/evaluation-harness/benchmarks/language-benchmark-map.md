# Language Benchmark Map

This file answers the practical question: which external benchmarks should polint lean on by language?

## Summary Recommendation

| Language | Primary External Benchmarks | Strength | Missing Piece |
|---|---|---|---|
| Go | gosec samples, CodeQL Go tests, Go x/tools tests | Existing Go security and analysis test corpora | No broad independent OWASP/RealVuln-style Go scanner benchmark. |
| TypeScript / JavaScript | SecBench.js, Jelly dynamic call graph comparison, CodeQL JS tests, BugsJS | Strong executable package security and call graph research | Need framework/route lifecycle fixtures and TS-specific project-scale tests. |
| Java | OWASP Benchmark Java, SecuriBench Micro, DroidBench, CryptoAPI-Bench, Juliet/SARD | Best external benchmark ecosystem | polint does not support Java yet; use mostly for harness design until adapter exists. |
| Python | RealVuln, OWASP BenchmarkPython, Pyre/Pysa tests, PyT, CrossCommitVuln-Bench | Strong real-app and agent-era benchmark momentum | polint does not support Python yet; BenchmarkPython is new/preliminary. |

## Go

### Use First

| Suite | Harness Role | Tier |
|---|---|---|
| gosec test samples | Security rule and taint microcases | Fast/nightly |
| CodeQL Go query tests | Security/data-flow/reference microcases | Nightly/reference |
| Go x/tools callgraph tests | Call graph expected behavior and API patterns | Fast/nightly |
| Native polint Go fixtures | Engine facts, extension merge, cache, unknowns | Fast |

### Why Go Needs Native Fixtures

Go does not currently have the same public benchmark maturity as Java/Python/JS for scanner outcomes. gosec is useful, but it is a tool's own test corpus. CodeQL tests are useful, but they reflect CodeQL's query/model decisions. Therefore Go needs more native polint fixtures than the other languages.

Native Go fixtures should cover:

- multi-module `go.mod` and `go.work` behavior;
- build tags;
- interface dispatch;
- method values;
- goroutines as entrypoints;
- HTTP handlers;
- command/package boundaries;
- extension-provided framework models.

## TypeScript / JavaScript

### Use First

| Suite | Harness Role | Tier |
|---|---|---|
| SecBench.js | Executable npm vulnerability benchmark | Nightly/release, smoke subset fast |
| Jelly dynamic call graph comparison | Static/dynamic call graph recall | Nightly/research |
| CodeQL JS query tests | Data-flow/security microcases | Nightly/reference |
| BugsJS | Real JS project scale and regression workflows | Research |
| Native TS/JS fixtures | Oxc facts, module systems, framework entrypoints, extension behavior | Fast |

### Special JS/TS Scoring Rules

JS/TS benchmark scoring must distinguish:

```text
observed dynamic edge     -> must be included by a recall-oriented static graph
extra static edge         -> unconfirmed unless ground truth proves it impossible
prototype/property edge   -> often heuristic/conservative
framework route edge      -> exact only if modeled by native/provider/extension evidence
```

The harness should not over-penalize conservative static edges when using dynamic traces.

## Java

### Use First When Java Support Exists

| Suite | Harness Role | Tier |
|---|---|---|
| OWASP Benchmark Java | Scanner TP/FP/FN/TN and scorecard comparability | Nightly/release |
| SecuriBench Micro | Servlet taint microcases | Nightly |
| CryptoAPI-Bench | Crypto misuse and path/field/interprocedural sensitivity | Nightly/release |
| DroidBench | Android lifecycle/callback taint | Research/release |
| Juliet/SARD Java | Broad synthetic CWE coverage | Release/research |
| CodeQL/Soot/WALA/OPAL tests | Reference microcases and call graph behavior | Research/reference |

### Java Accuracy Lessons For All Languages

Java benchmarks show why the harness needs multiple benchmark classes:

- OWASP-style scanner metrics are easy to compare but can be overfit.
- SecuriBench/DroidBench isolate taint behavior but are not whole-codebase realism.
- CryptoAPI-Bench stresses domain-specific API misuse and path/field sensitivity.
- Juliet/SARD provides breadth but not modern framework realism.

Do not let a single Java score become a proxy for engine quality.

## Python

### Use First When Python Support Exists

| Suite | Harness Role | Tier |
|---|---|---|
| RealVuln | Real Python web app scanner benchmark with FP traps | Nightly/release |
| OWASP BenchmarkPython | Synthetic/runnable scanner benchmark | Nightly/release |
| Pyre/Pysa tests | Taint model, source/sink, CFG, call graph, model-generation patterns | Fast/nightly/reference |
| PyT examples | CFG/taint microcase taxonomy | Reference |
| CrossCommitVuln-Bench | Temporal/cross-commit analysis | Research |
| CodeQL Python tests | Security/data-flow/reference microcases | Nightly/reference |

### Why Python Is Important To Polint Even Before Support

Python has the most relevant current benchmark movement for agent-era SAST:

- RealVuln compares rule-based scanners and LLM/agent scanners on real apps.
- CrossCommitVuln-Bench shows why snapshot-only scanning can miss vulnerabilities.
- Pyre/Pysa models show how powerful framework/source/sink modeling becomes when user-supplied models are first-class.

These lessons should influence the Rust extension lifecycle even before a Python parser adapter exists.

## Cross-Language Benchmark Strategy

### Fast Tier

Use:

- native polint fixtures for supported languages;
- small Go/TS benchmark subsets;
- adapter parser tests for unsupported-language external suites;
- deterministic output checks.

### Nightly Tier

Use:

- full external suites for supported languages;
- external adapter validation for future languages;
- runtime/memory/provider/cache baselines;
- default-vs-extension deltas.

### Release Tier

Use:

- full supported-language external benchmark pack;
- research-tier suites with pinned versions;
- published report with source commits and methodology.

## Clear Policy

When an external benchmark exists and has credible ground truth, polint should use it.

When no credible benchmark exists for the exact engine behavior, create the smallest native fixture that tests that behavior and clearly label it as an engine invariant test, not an independent benchmark.
