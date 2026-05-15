# Paper And Source Index

This index records research sources used for the evaluation-harness recommendation. Local PDFs are stored in `papers/` where downloaded.

## Downloaded Papers

| Source | Local File | Why It Matters |
|---|---|---|
| SecBench.js: An Executable Security Benchmark Suite for Server-Side JavaScript, ICSE 2023 | `papers/secbench-js-icse-2023.pdf` | Strong external benchmark model for executable JS package vulnerabilities. |
| RealVuln: Benchmarking Rule-Based, General-Purpose LLM, and Security-Specialized Scanners on Real-World Code, 2026 | `papers/realvuln-2026.pdf` | Current benchmark for real Python vulnerable apps, false-positive traps, and LLM/agent scanner evaluation. |
| CrossCommitVuln-Bench: A Dataset of Multi-Commit Python Vulnerabilities Invisible to Per-Commit Static Analysis, 2026 | `papers/crosscommitvuln-bench-2026.pdf` | Shows why snapshot/per-commit evaluation is insufficient for long-lived analysis state. |
| SecCodeBench v2, 2026 | `papers/seccodebench-v2-2026.pdf` | Agentic coding benchmark relevant to future rule/extension authoring evaluation. |

## Web And Project Sources

| Source | URL | Key Use |
|---|---|---|
| OWASP Benchmark project | <https://owasp.org/www-project-benchmark/> | Standard TP/FN/TN/FP, TPR/FPR, scorecard framing, and Java benchmark context. |
| OWASP Benchmark Java | <https://github.com/OWASP-Benchmark/BenchmarkJava> | Java expected-results CSV and runnable benchmark app. |
| OWASP BenchmarkPython | <https://github.com/OWASP-Benchmark/BenchmarkPython> | Python expected-results CSV and new Python benchmark shape. |
| OWASP BenchmarkUtils | <https://github.com/OWASP-Benchmark/BenchmarkUtils> | Scorecard implementation and supported-tool normalization patterns. |
| SecBench.js publication page | <https://publications.cispa.saarland/3909/> | ICSE publication metadata and paper download. |
| SecBench.js repository | <https://github.com/cristianstaicu/SecBench.js> | Executable server-side JS benchmark implementation. |
| RealVuln dashboard/site | <https://realvuln.kolega.dev/> | Current public benchmark results and F3 framing. |
| RealVuln repository | <https://github.com/kolega-ai/Real-Vuln-Benchmark> | Ground-truth schema, scorer, repo manifest, agent/LLM harness. |
| CrossCommitVuln-Bench arXiv | <https://arxiv.org/abs/2604.21917> | Multi-commit vulnerability benchmark and per-commit SAST limitations. |
| CrossCommitVuln-Bench repository | <https://github.com/motornomad/crosscommitvuln-bench> | Annotation schema, scripts, and dataset structure. |
| SecCodeBench repository | <https://github.com/alibaba/sec-code-bench> | Agentic secure coding evaluation framework. |
| NIST Juliet 1.1 paper | <https://www.nist.gov/publications/juliet-11-cc-and-java-test-suite> | Synthetic C/C++ and Java benchmark with known flaws and non-flawed cases. |
| NIST SARD | <https://www.nist.gov/itl/ssd/software-quality-group/samate/software-assurance-reference-dataset-sard> | Large public software assurance reference dataset. |
| DroidBench repository | <https://github.com/secure-software-engineering/DroidBench> | Android taint benchmark. |
| CryptoAPI-Bench repository | <https://github.com/CryptoAPI-Bench/CryptoAPI-Bench> | Java crypto misuse cases. |
| CryptoAPI-Bench SecDev paper PDF | <https://www.sazzadur.com/pdfs/SecDev19_CryptoAPI-Bench.pdf> | Crypto misuse benchmark design and evaluation framing. |
| CryptoAPI-Bench evaluation article | <https://arxiv.org/abs/2112.04037> | Reports 181 unit cases and compares static detection tools. |
| SecuriBench Micro | <https://github.com/too4words/securibench-micro> | Java servlet microbenchmarks for static security analyzers. |
| gosec repository | <https://github.com/securego/gosec> | Go security analyzer, samples, and taint benchmark baseline. |
| Pyre/Pysa repository | <https://github.com/facebook/pyre-check> | Python taint model tests, integration fixtures, model generation ideas. |
| Jelly repository | <https://github.com/cs-au-dk/jelly> | JS/TS static and dynamic call graph comparison implementation. |

## Prior Research Papers Used By Reference

These were already downloaded or indexed in call graph and data-flow research. They are still relevant to the harness:

| Source | Existing Location | Harness Relevance |
|---|---|---|
| Static JavaScript Call Graphs: A Comparative Study, 2024 | `research/call-graphs/papers/static-js-call-graphs-comparative-2024.pdf` | JS/TS call graph metrics and dynamic/static comparison. |
| PyCG, ICSE 2021 | `research/call-graphs/papers/pycg-icse-2021.pdf` | Python call graph benchmark/evaluation ideas. |
| JARVIS Python call graph, 2023 | `research/call-graphs/papers/jarvis-python-call-graph-2023.pdf` | Demand-driven Python call graph precision/cost tradeoffs. |
| OPAL TotalRecall, ISSTA 2024 | `research/call-graphs/papers/opal-totalrecall-issta-2024.pdf` | Java call graph soundness/recall evaluation. |
| Java call graph unsoundness, 2026 | `research/call-graphs/papers/java-call-graph-unsoundness-2026.pdf` | Shows why call graph benchmark truth must include unsoundness accounting. |
| FlowDroid, PLDI 2014 | `research/data-flow/papers/flowdroid-pldi-2014.pdf` | DroidBench-style taint evaluation and lifecycle modeling. |
| Incremental CodeQL, FSE 2023 | `research/analysis-kernel/papers/incremental-codeql-fse-2023.pdf` | Incremental evaluation and invalidation lessons. |
| IncIDFA, OOPSLA 2025 | `research/analysis-kernel/papers/incidfa-oopsla-2025.pdf` | Incremental data-flow analysis and cache correctness lessons. |

## Source Accuracy Notes

- Repository commit IDs are recorded in `REPO-INDEX.md`.
- Some benchmark papers and repositories are newer than the model cutoff and were verified through web search and local clones on 2026-05-15.
- RealVuln, CrossCommitVuln-Bench, and SecCodeBench are current/evolving. Revalidate their schemas and claims before implementation.
- Exact benchmark licensing must be checked before any source or expected-output subset is committed.
