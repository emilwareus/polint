# Final Report: Evaluation Harness

## Executive Decision

Build polint's evaluation system as an **external-benchmark-first harness** with native engine fixtures for the parts external benchmarks do not measure.

```text
external benchmark suite
  -> suite adapter
  -> canonical expected facts/diagnostics/edges/paths
  -> run polint in default mode
  -> run polint in extension mode
  -> score suite-native metrics
  -> score polint unified metrics
  -> report accuracy, cost, provenance, cache, and extension delta
```

Do not create a big private synthetic benchmark as the main proof of quality. That creates self-confirming evidence. Lean hard on external benchmarks where they exist, then fill only the product-specific gaps with native fixtures.

## The Critical Insight

The user preference is correct: if we are benchmarking, we should rely on other people's benchmarks wherever possible.

That is the only credible path for a tool that wants to be trusted. OWASP, SecBench.js, RealVuln, DroidBench, SecuriBench Micro, CryptoAPI-Bench, Juliet/SARD, CodeQL tests, gosec tests, Pyre/Pysa tests, and Jelly-style dynamic traces encode years of painful edge cases. Rebuilding those ourselves would waste time and bias us toward cases polint already handles.

But "completely on external benchmarks" is impossible for polint because the product is not just a scanner. It is an analysis kernel that AI agents can extend with Rust code. External benchmarks usually evaluate final findings:

```text
did the tool report CWE-89 near this line?
did the tool detect this package vulnerability?
did the tool find this tainted source-to-sink path?
```

They usually do not evaluate:

- whether a fact has correct provenance;
- whether extension facts merge safely with native facts;
- whether a cache key invalidates exactly the affected layer;
- whether unknown calls are surfaced as actionable facts;
- whether an agent-authored model improved recall without hiding uncertainty;
- whether a typed SDK view preserves the right precision labels;
- whether two provider orders produce deterministic output.

So the right answer is:

```text
External benchmarks for outcome truth.
Native polint fixtures for engine truth.
Both in one harness.
```

## What We Cloned And Studied

Local benchmark and implementation clones live under `research/evaluation-harness/repos/`, which is gitignored.

The most important external suites:

| Suite | Language | Why It Matters |
|---|---:|---|
| OWASP Benchmark Java | Java | Widely used scanner benchmark with explicit expected results, TP/FN/TN/FP scoring, and runnable web app cases. |
| OWASP BenchmarkPython | Python | New Python counterpart with expected results and runnable-app shape. |
| BenchmarkUtils | Java | Scorecard implementation for OWASP metrics and result parsing. |
| SecBench.js | JS/TS ecosystem | Executable npm vulnerability benchmark; strong for server-side JS package security. |
| RealVuln Benchmark | Python | Real vulnerable Python apps, false-positive traps, scoring, manifests, scanner outputs, and LLM/agent harness ideas. |
| CrossCommitVuln-Bench | Python/history | Shows per-commit SAST misses vulnerabilities whose exploitable chain spans multiple commits. |
| SecCodeBench | Java, C/C++, Python, Go, Node.js | Agentic coding benchmark; not scanner-first, but valuable for future "agent writes rule/extension" evaluation. |
| SecuriBench Micro | Java | Classic servlet taint/security microbenchmarks. |
| DroidBench | Java/Android | Taint/lifecycle/callback benchmark for Android data-flow engines. |
| CryptoAPI-Bench | Java | Crypto API misuse benchmark with interprocedural, field, path, and multi-class patterns. |
| gosec | Go | Best available Go security sample corpus and performance baselines, but not an independent broad benchmark. |
| CodeQL tests | Go, JS, Python, Java | Very useful micro-fixtures and expected outputs; should be used as reference taxonomy, not copied as product truth. |
| Pyre/Pysa | Python | Strong source for Python model, taint, CFG, and call graph test design. |
| Jelly | JS/TS | Strongest source for JS/TS call graph evaluation, including dynamic call graph comparison. |

## External Benchmark Coverage By Goal

| Goal | External Coverage | Gap |
|---|---|---|
| Scanner vulnerability TP/FP/FN/TN | Strong for Java, Python, JS; weaker for Go | Need Go-native and repo-specific fixtures. |
| Security taint/data-flow paths | Strong in Java/Android/Python/JS; medium in Go | Need canonical path/evidence schema. |
| Call graph recall | Strongest in JS/TS via dynamic traces; Java/Python research suites exist; Go weaker | Need graph partial-truth scoring and language-specific adapters. |
| Framework lifecycle modeling | Partial: RealVuln, Pysa models, DroidBench, SecBench.js | Need polint fixtures for routes/jobs/queues/MCP/CLI/serverless. |
| Agent extension impact | Emerging: RealVuln and SecCodeBench are useful | Need product-native default-vs-extension evaluation. |
| Provenance, merge, cache, scheduling | Almost none | Must use native fixtures and invariant tests. |
| Runtime/memory | Possible but not standardized across suites | Need polint's own provider stats and machine-stable baseline mode. |

## Accuracy Lessons

### OWASP Benchmark

OWASP is useful because the expected result files make confusion-matrix scoring straightforward. BenchmarkJava's checked clone has 2,740 labelled cases plus header; BenchmarkPython's checked clone has 1,230 labelled cases plus header. BenchmarkUtils implements the classic metrics:

```text
recall / TPR = TP / (TP + FN)
FPR          = FP / (FP + TN)
score        = TPR - FPR
```

Polint should preserve OWASP-native scorecards for comparability, but not use `TPR - FPR` as the only product metric. A tool can overfit synthetic benchmarks, and RealVuln explicitly calls out the gap between synthetic single-case benchmark success and real application performance. Use OWASP as a regression and breadth suite, not as proof of real-world dominance.

### SecBench.js

SecBench.js is highly valuable because cases are executable package vulnerabilities, not just annotated snippets. The local clone contains 704 `.test.js` files. It is especially useful for JS/TS because npm package behavior, prototype pollution, ReDoS, command injection, code injection, and path traversal often depend on library API semantics that purely syntactic tools miss.

The cost is setup complexity. It should be nightly/release-tier first, with a curated smoke subset in CI.

### RealVuln

RealVuln is the best current signal for scanner behavior on real Python web apps. It has 26 repositories, 796 labelled findings, 676 vulnerabilities, and 120 false-positive traps. It uses F3 as a recall-weighted metric, includes line-tolerant matching, and has a reproducibility manifest.

This is close to polint's product thesis because it evaluates real code and includes LLM/agent runners. The limitation is language coverage: today it is Python-only.

### CrossCommitVuln-Bench

CrossCommitVuln-Bench matters because it breaks a hidden assumption in normal static-analysis CI: scanning each commit independently can miss a vulnerability chain whose pieces arrive across time. This should influence polint's future cache and historical analysis design. It is not a first harness target, but it is important for "world's most capable" long-term goals.

### Go

Go has the weakest external benchmark coverage among the target languages. gosec provides real rules, test samples, a taint benchmark baseline, and useful performance scripts. CodeQL Go query tests provide many expected-output microcases. But there is no equivalent of OWASP Benchmark Java or SecBench.js for broad Go scanner evaluation.

Recommendation: treat Go as a mixed strategy:

```text
gosec samples + CodeQL Go tests + Go x/tools/callgraph tests + polint-native fixtures
```

This is one place where relying completely on external benchmarks is not feasible yet.

## Time Complexity Of The Harness

The harness itself should be linear or near-linear in the size of observed outputs. Analyzer runtime will dominate.

| Component | Expected Complexity | Notes |
|---|---:|---|
| Expected-result parsing | `O(N)` | N expected rows/findings/facts. |
| Diagnostic matching | `O(E + O)` with indexes | E expected, O observed. Index by file, CWE/kind, function, line bucket. |
| Fact matching | `O(E + O)` | Stable keys make this cheap; structural fallback may cost more. |
| Graph edge comparison | `O(E_static + E_truth)` | Hash edge keys. Dynamic truth is partial, so extras are "unconfirmed," not always false. |
| Path matching | `O(P * L)` after endpoint indexing | P candidate paths, L path length. Avoid all-pairs path comparison. |
| Suite scoring | `O(C + F)` | C cases, F findings. |
| Delta report | `O(default + extended)` | Compare normalized outputs by stable keys. |
| Baseline storage | `O(report size)` | Keep compressed JSON and summary Markdown. |

Do not put complex graph algorithms in the scorer unless the benchmark explicitly demands them. The scorer should compare normalized outputs. The analysis engine should own graph/path computation.

## Measurement Model

Every run should report both suite-native and polint-unified metrics.

Suite-native examples:

- OWASP: TP, FN, TN, FP, TPR, FPR, `TPR - FPR`.
- RealVuln: F3, precision, recall, per-CWE and per-severity breakdowns.
- SecBench.js: executable test pass/fail and vulnerability-class breakdowns.
- Jelly-style call graphs: dynamic-edge recall and static-edge precision caveats.

Polint-unified metrics:

- precision, recall, F1, F2, F3;
- false-positive trap hit rate;
- unresolved-call count and rate;
- unknown fact count by family;
- evidence span quality;
- graph edge recall against partial ground truth;
- source-to-sink path recall and path-noise score;
- runtime, CPU time, peak RSS, facts/sec;
- provider time by layer;
- cache hit/miss by layer;
- deterministic output hash;
- extension delta: new TP, new FP, removed FN, removed FP, new unknowns, resolved unknowns, rejected extension facts.

## Recommended Evaluation Tiers

### Tier A: Fast CI

Goal: catch regressions in seconds to one minute.

Use:

- native polint kernel fixtures;
- small OWASP subset;
- small SecBench.js subset;
- gosec sample subset;
- CodeQL-inspired microcases where licenses and usage allow;
- one RealVuln smoke target if setup is cheap.

### Tier B: Nightly

Goal: measure serious accuracy and cost changes.

Use:

- full OWASP Benchmark Java and Python;
- curated SecBench.js packages;
- RealVuln full Python suite;
- gosec/CodeQL Go samples;
- Pyre/Pysa-style Python taint fixtures;
- Jelly dynamic call graph subset;
- memory/runtime baselines.

### Tier C: Release / Research

Goal: publish credible capability claims.

Use:

- full external suites;
- Juliet/SARD slices;
- DroidBench;
- CryptoAPI-Bench;
- SecuriBench Micro;
- CrossCommitVuln-Bench experiments;
- SecCodeBench agent workflows;
- large real open-source repos pinned by manifest.

## Required Native Fixture Layer

Native fixtures should be small and deliberately boring. They should exist only for engine behavior external benchmarks do not specify.

Must-have native fixture categories:

- provider scheduling order and deterministic batching;
- fact provenance and precision labels;
- extension fact validation and rejection;
- merge conflicts between native and extension facts;
- cache key invalidation by source/config/rule/extension/provider version;
- default-vs-extension delta reports;
- stable fact keys across runs;
- graph partial-truth matching;
- unknown facts and setup-gap diagnostics;
- typed SDK view compatibility.

These fixtures are not a marketing benchmark. They are the engine's unit and integration test spine.

## Wrong Paths To Avoid

- Do not build a giant polint-only benchmark and call it proof.
- Do not use OWASP score alone as the product quality metric.
- Do not treat dynamic traces as complete call graph truth.
- Do not count every extra static edge as false when the ground truth is partial.
- Do not compare diagnostics by exact line only.
- Do not tune rules to benchmark filenames or generated naming patterns.
- Do not hide false positives by reporting only recall.
- Do not benchmark agent-extended mode without also showing the default baseline.
- Do not let extension facts bypass validation to improve benchmark scores.
- Do not make benchmark repos part of the product repository history.

## Recommended Next Step

After the analysis kernel research, build the evaluation harness before implementing serious call graph/data-flow features.

The first vertical slice should be:

```text
hidden polint eval command
  + native fixture adapter
  + OWASP expected-results adapter
  + unified metrics JSON
  + default-vs-extension report shape
  + provider runtime/cache stats
```

This gives us a scoreboard before we start adding expensive algorithms. It also prevents the project from mistaking "more sophisticated analysis" for "better measured outcomes."
