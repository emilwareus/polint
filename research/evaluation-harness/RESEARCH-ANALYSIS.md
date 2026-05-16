# Research Analysis: Evaluation Harness

## Product-Specific Goal

polint's evaluation harness has to measure a different product than classic static analyzers.

Classic static-analysis benchmark:

```text
tool scans corpus -> tool emits findings -> benchmark scores findings
```

polint benchmark:

```text
engine extracts typed facts
  -> kernel schedules providers
  -> extensions may add validated facts
  -> rules query typed SDK views
  -> diagnostics and evidence are emitted
  -> benchmark scores outcomes and engine behavior
```

This difference matters. Scanner benchmarks tell us whether a final alert appears. They do not tell us whether the underlying facts are reusable, whether the agent extension lifecycle is safe, or whether cache invalidation is correct.

## External-Benchmark-First, Not External-Only

The best benchmark strategy is to start from external suites and resist inventing private substitutes.

Reasons:

- External suites force us to handle cases we did not design.
- They make claims comparable to other tools.
- They prevent accidental benchmark gaming.
- They encode edge cases across languages and vulnerability families.
- They give agents a concrete feedback loop: "your extension reduced these false negatives and added these false positives."

The limitation is that most external suites score final findings, not analysis facts. They are usually blind to:

- fact provenance;
- precision labels;
- unknown facts;
- extension validation;
- fact merge conflicts;
- cache invalidation;
- deterministic scheduling;
- typed SDK view stability.

Therefore the harness should have two layers:

| Layer | Source | Purpose |
|---|---|---|
| External benchmark adapters | OWASP, SecBench.js, RealVuln, gosec, CodeQL tests, Pyre/Pysa, DroidBench, etc. | Measure scanner outcomes, data-flow/call-graph recall, language coverage, and real-world behavior. |
| Native polint fixtures | Small generated repos and expected facts | Measure engine invariants and extension lifecycle correctness. |

## Benchmark Classes

### 1. Expected-Result Scanner Benchmarks

Examples: OWASP Benchmark Java, OWASP BenchmarkPython, RealVuln.

These suites provide explicit expected vulnerable/safe cases. They map naturally to confusion matrices.

Strengths:

- easy to automate;
- comparable metrics;
- good regression gates;
- useful per-category breakdowns;
- often include false-positive traps or safe variants.

Weaknesses:

- can be synthetic;
- can be overfit;
- line matching can be brittle;
- scanner output mapping can dominate score quality;
- may not expose the fact-level reason for success/failure.

Recommended polint use:

- primary scanner-level regression suite;
- suite-native score preserved;
- unified metrics added;
- explanation quality scored separately.

### 2. Executable Vulnerability Benchmarks

Examples: SecBench.js, OWASP runnable apps, DroidBench.

These cases include executable tests, apps, or exploit scripts.

Strengths:

- closer to exploitability;
- useful for validating evidence paths;
- exposes framework/package behavior;
- harder to game than pure labelled snippets.

Weaknesses:

- costly setup;
- dependency drift;
- test runtime variance;
- not always easy to map execution truth to static facts;
- dynamic execution gives partial evidence, not complete static truth.

Recommended polint use:

- nightly and release tiers;
- smoke subsets in CI;
- use executable proof as strong evidence for endpoint/source/sink truth.

### 3. Microbenchmarks For Analysis Features

Examples: SecuriBench Micro, DroidBench, CryptoAPI-Bench, CodeQL query tests, Pyre/Pysa tests, gosec samples.

Strengths:

- targeted edge cases;
- excellent for data-flow, callbacks, field sensitivity, path sensitivity, summaries, and sanitizers;
- can isolate one analysis behavior at a time.

Weaknesses:

- not representative of real repos;
- often language/framework-specific;
- sometimes encode the original tool's assumptions;
- expected outputs may not map directly to polint facts.

Recommended polint use:

- fact/provider regression cases;
- analysis-family acceptance tests;
- source of edge-case taxonomy.

### 4. Real-Repo Benchmarks

Examples: RealVuln, BugsJS, large pinned OSS repos, future language-specific real corpora.

Strengths:

- realistic dependency graphs and framework patterns;
- exposes performance and memory behavior;
- useful for agent extension evaluation.

Weaknesses:

- ground truth is expensive;
- results can drift unless commits are pinned;
- coverage is incomplete;
- scanner matching is noisy.

Recommended polint use:

- full nightly/release evaluation;
- performance tracking;
- agent-extended delta reports.

### 5. Temporal Benchmarks

Example: CrossCommitVuln-Bench.

Strengths:

- tests something normal SAST misses: vulnerability state that emerges over time;
- directly relevant to cache/incremental analysis and agent workflows.

Weaknesses:

- not a first implementation target;
- requires historical checkout orchestration;
- findings may need cumulative graph state.

Recommended polint use:

- later research/advanced evaluation;
- guide design of historical caches and persistent fact databases.

### 6. Agentic Coding Benchmarks

Example: SecCodeBench.

Strengths:

- evaluates agents that modify code, run tests, and interact with tools;
- aligns with polint's primary future users.

Weaknesses:

- not a scanner benchmark;
- score depends on agent/model behavior;
- expensive and less deterministic.

Recommended polint use:

- future evaluation of "agent writes rule/extension, then polint validates it";
- not part of the first static-analysis accuracy suite.

## Language-Specific Recommendations

### Java

Use:

- OWASP Benchmark Java as the first scanner benchmark.
- SecuriBench Micro for servlet taint.
- CryptoAPI-Bench for crypto misuse and data-flow sensitivity.
- DroidBench for Android lifecycle/callback/data-flow concepts.
- Juliet/SARD slices for broad synthetic CWE coverage.
- CodeQL/Soot/WALA/OPAL tests as reference cases where licensing permits.

Java has the best benchmark ecosystem. The risk is over-weighting Java because it has mature suites while polint's current implementation does not yet support Java. Use Java research to design harness abstractions, but do not let Java-specific assumptions shape the entire engine.

### TypeScript / JavaScript

Use:

- SecBench.js as the primary executable JS security benchmark.
- Jelly dynamic call graph comparison for call graph evaluation.
- BugsJS for real project scale and regression-style experiments.
- CodeQL JS query tests as microcase references.
- NodeGoat/OWASP Juice Shop later for route/framework modeling if needed.

JS/TS requires special care because dynamic property access, prototype pollution, module systems, package entrypoints, and framework conventions make exact static truth difficult. The harness should separate:

```text
dynamic observed edge/path
static required edge/path
unconfirmed static extra
confirmed false edge/path
```

### Python

Use:

- RealVuln as the primary real-app benchmark.
- OWASP BenchmarkPython as the primary synthetic/runnable benchmark.
- Pyre/Pysa tests for taint models, source/sink stubs, and model-generation ideas.
- PyT examples for CFG/taint microcases.
- CrossCommitVuln-Bench later for temporal analysis.
- CodeQL Python tests as microcase references.

Python has strong current momentum because RealVuln and CrossCommitVuln-Bench are recent and agent-aware. It is the best language for evaluating default-vs-agent-extended scanner deltas early, even if polint does not support Python yet.

### Go

Use:

- gosec samples, tests, and taint benchmark scripts.
- CodeQL Go query tests.
- Go x/tools callgraph tests and examples.
- SecCodeBench Go cases later for agentic secure coding workflows.
- Native polint Go fixtures for missing external truth.

Go is the weakest benchmark area. This should not block Go support, but it means claims must be honest:

```text
Go evaluation starts with public analyzer test corpora and native fixtures.
It will not have OWASP/RealVuln-level external breadth on day one.
```

## Metrics: What We Should Report

### Accuracy

Report:

- TP, FP, FN, TN;
- precision;
- recall;
- F1;
- F2 and F3 for security recall emphasis;
- FPR;
- false-positive trap hit rate;
- per-language breakdown;
- per-CWE or category breakdown;
- per-provider/fact-family breakdown.

Security scanners often prefer recall, but polint cannot ignore precision. Agent-authored extensions can trivially improve recall by adding overly broad facts. The harness must make that tradeoff visible.

### Graph Quality

Report:

- required edge recall;
- forbidden edge violations;
- dynamic observed edge recall;
- extra edge count;
- unconfirmed extra edge count;
- unresolved call count;
- unknown call rate;
- average candidate callees per call site;
- call graph build time and memory.

Do not score graph precision naively when truth is partial. Dynamic traces show executed edges, not all possible edges.

### Data-Flow Path Quality

Report:

- source-to-sink recall;
- safe-source/safe-sink FP rate;
- sanitizer/barrier correctness;
- path explanation length;
- noisy path rate;
- missing evidence spans;
- access-path precision;
- summary reuse rate.

Path quality matters because polint is for AI agents. A correct but useless 80-step explanation is worse than a concise path the agent can act on.

### Engine Quality

Report:

- provider runtime;
- provider memory;
- facts emitted per provider;
- rejected facts;
- validation failures;
- cache hits/misses by layer;
- invalidated layers;
- deterministic output hash;
- unknown facts by family;
- setup-gap diagnostics.

These metrics are where polint differs from a black-box scanner.

### Extension Delta

For every agent/user extension benchmark, report:

- default score;
- extended score;
- new true positives;
- removed false positives;
- new false positives;
- resolved unknowns;
- new unknowns;
- accepted extension facts;
- rejected extension facts;
- runtime overhead;
- cache invalidation scope.

An extension should be considered good only if it improves meaningful metrics without unacceptable precision, runtime, or cache cost.

## Matching Policy

### Diagnostics

Match by:

```text
file identity
  + diagnostic kind or acceptable CWE
  + line tolerance
  + optional function/source/sink identity
```

Line tolerance should be configurable per suite. RealVuln uses a line-tolerant design; OWASP generated test cases often allow tighter matching by test id/category.

### Facts

Match by:

```text
stable key if available
else structural selector
else source span + normalized semantic kind
```

External suites rarely label facts, so native fixtures should own exact fact matching.

### Graph Edges

Match by normalized node keys and call-site spans:

```text
caller stable key
callee stable key
call site span bucket
edge kind
```

Extras against partial ground truth should be `UNCONFIRMED`, not automatically `FP`.

### Paths

Match by endpoint first:

```text
source matches
sink matches
required intermediates included
forbidden intermediates absent
evidence span acceptable
```

Do not demand exact path equality unless the fixture is native and intentionally tests the solver.

## Runtime And Memory Baselines

Runtime baselines must be machine-aware. A hard "must finish in 10s" threshold is noisy across laptops and CI.

Better:

- record machine class;
- record input size;
- record provider-level timings;
- compare against previous baseline on same CI class;
- use regression thresholds, e.g. `>20%` wall-time increase and `>10%` RSS increase fails nightly;
- keep fast CI thresholds looser and focused on severe regressions.

Performance output should include:

```text
parse time
symbol time
module graph time
call graph time
data-flow time
rule time
extension time
validation time
cache load/save time
peak RSS
facts/sec
edges/sec
paths/sec
```

## Final Recommendation

Implement the harness before serious call graph/data-flow implementation. It should become the scoreboard for all future research-to-code decisions.

The first implementation does not need all benchmark adapters. It needs the right shape:

```text
standard suite manifest
canonical expected/observed model
OWASP CSV adapter
native fixture adapter
unified metrics
default-vs-extension report
provider performance stats
stable JSON output
```

After that, every new analysis family gets introduced with an evaluation story, not just a provider implementation.
