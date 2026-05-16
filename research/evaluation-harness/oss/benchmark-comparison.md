# OSS Benchmark Comparison

This comparison ranks the inspected benchmark/code repositories by how much they should influence polint's evaluation harness.

## Priority Ranking

| Rank | Suite / Repo | Influence | Reason |
|---:|---|---|---|
| 1 | RealVuln | Very high | Real code, false-positive traps, scoring engine, reproducibility manifest, LLM/agent harness ideas. |
| 2 | OWASP Benchmark + BenchmarkUtils | Very high | Mature expected-results and scorecard model; easiest first external adapter. |
| 3 | SecBench.js | Very high | Executable JS package security benchmark with real npm vulnerability behavior. |
| 4 | gosec | High for Go | Best available Go security corpus and performance/taint samples. |
| 5 | CodeQL tests | High reference value | Multi-language microcases and expected outputs across security/data-flow/reference behavior. |
| 6 | Pyre/Pysa | High design value | Strong Python model/taint/extension lessons. |
| 7 | Jelly | High for JS/TS call graphs | Demonstrates static/dynamic call graph comparison and partial ground-truth handling. |
| 8 | CryptoAPI-Bench | Medium-high for Java | Focused domain benchmark for crypto API misuse sensitivity. |
| 9 | SecuriBench Micro | Medium-high for Java | Classic taint/security microcases. |
| 10 | DroidBench | Medium-high for lifecycle/data flow | Strong Android lifecycle benchmark, but less relevant until Java/Android support. |
| 11 | CrossCommitVuln-Bench | Strategic | Important for temporal/persistent analysis, not first implementation. |
| 12 | SecCodeBench | Strategic | Important for agentic rule/extension workflows, not first scanner harness. |
| 13 | BugsJS | Supporting | Real JS project history, but not direct static-analysis ground truth. |
| 14 | PyT | Supporting | Older Python CFG/taint examples and taxonomy. |

## Accuracy And Bias Analysis

| Suite | Accuracy Strength | Bias / Limitation | polint Mitigation |
|---|---|---|---|
| OWASP Benchmark | Clear labels and confusion-matrix scoring | Synthetic/generated cases can be overfit | Preserve native score, pair with RealVuln/SecBench.js/real repos. |
| RealVuln | Real app findings and FP traps | Python-only today, evolving benchmark | Use pinned manifests and revalidate schema. |
| SecBench.js | Executable exploit/test behavior | Heavy dependency setup; package ecosystem drift | Pin commits, use smoke/nightly/release tiers. |
| gosec | Go-specific and practical | Own-tool test corpus, not independent truth | Pair with CodeQL tests and native fixtures. |
| CodeQL tests | Deep edge-case taxonomy | Expected output reflects CodeQL model choices | Use as reference/microcase inspiration, not absolute product truth. |
| Pyre/Pysa | Strong model-driven taint architecture | Python/Pysa-specific abstractions | Translate lessons into Rust extension lifecycle. |
| Jelly | Strong graph comparison model | JS/TS-focused and dynamic traces are partial | Use partial-truth result classes. |
| CryptoAPI-Bench | Domain-specific API misuse precision | Java crypto only | Use later for domain model and path sensitivity evaluation. |
| SecuriBench Micro | Isolated taint cases | Old servlet-centric style | Use for analysis behavior, not real-world claims. |
| DroidBench | Lifecycle/callback taint depth | Android-specific | Use to design lifecycle models and callback entrypoints. |
| CrossCommitVuln-Bench | Exposes temporal SAST blind spot | Python CVE dataset, advanced workflow | Use after persistent facts/history exist. |
| SecCodeBench | Agentic coding and dynamic verification | Not scanner-first | Use to evaluate agent-generated rules/extensions later. |

## Adapter Difficulty

| Suite | Adapter Difficulty | Why |
|---|---:|---|
| OWASP Benchmark | Low | Expected CSV is simple; scoring model is clear. |
| Native polint fixtures | Low | We control fixture schema. |
| gosec samples | Medium | Need map sample/test expectations into polint expected diagnostics/facts. |
| CodeQL tests | Medium | Many expected formats and query-specific semantics. |
| RealVuln | Medium | JSON ground truth is clear; repo cloning/scanner output mapping adds setup. |
| Pyre/Pysa | Medium | Useful fixtures but Pysa model semantics need translation. |
| SecBench.js | High | Package installs, executable tests, and vulnerability metadata normalization. |
| Jelly dynamic call graph | High | Requires dynamic trace generation/comparison and graph normalization. |
| DroidBench | High | Android build/lifecycle setup. |
| CrossCommitVuln-Bench | High | Multi-commit checkout and temporal state. |
| SecCodeBench | High | Agent/tool execution, Docker verifiers, and security judges. |

## First Adapter Order

1. Native polint fixtures.
2. OWASP expected-results CSV.
3. gosec/CodeQL-inspired supported-language microcases.
4. SecBench.js smoke subset for TS/JS.
5. RealVuln when Python adapter work begins, or sooner as adapter-only validation.
6. Jelly dynamic call graph for JS/TS once call graph facts exist.
7. Pyre/Pysa, CryptoAPI-Bench, SecuriBench, DroidBench, CrossCommitVuln, and SecCodeBench as their corresponding analysis families mature.
