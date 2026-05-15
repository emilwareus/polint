# Repository Index

Local clones are under `research/evaluation-harness/repos/`, which is gitignored. They are research inputs, not vendored product dependencies.

## Evaluation Harness Repositories

| Repository | Local Path | Commit Checked | Primary Use | Notes |
|---|---|---:|---|---|
| OWASP Benchmark Java | `repos/BenchmarkJava` | `61b831658171` | Java scanner benchmark | Expected results CSV has 2,740 labelled cases plus header. |
| OWASP BenchmarkPython | `repos/BenchmarkPython` | `9f0d34945a88` | Python scanner benchmark | Expected results CSV has 1,230 labelled cases plus header; version is preliminary. |
| OWASP BenchmarkUtils | `repos/BenchmarkUtils` | `3f856fc51930` | OWASP scoring implementation | Implements TP/FN/TN/FP, TPR, FPR, F-score, and scorecard generation. |
| SecBench.js | `repos/SecBench.js` | `bc3156219138` | JS/TS executable security benchmark | Local clone contains 704 `.test.js` exploit/test files. |
| Real-Vuln-Benchmark | `repos/Real-Vuln-Benchmark` | `822b505300ab` | Real Python scanner benchmark | 26 repos, 796 findings, 676 vulnerabilities, 120 FP traps. |
| CrossCommitVuln-Bench | `repos/crosscommitvuln-bench` | `e544e645fe16` | Temporal vulnerability benchmark | 15 annotated Python CVEs where vulnerability chains span commits. |
| SecCodeBench | `repos/sec-code-bench` | `45ae4dcba5b0` | Agentic secure coding benchmark | 98 cases across Java, C/C++, Python, Go, Node.js. Useful for future agent workflows. |
| DroidBench | `repos/DroidBench` | `0fe281b8bc34` | Android taint benchmark | 120 Android test cases in v2.0 according to project docs. |
| CryptoAPI-Bench | `repos/CryptoAPI-Bench` | `e6b6b50fef69` | Java crypto misuse benchmark | Good for interprocedural, field, path, and multi-class crypto misuse cases. |
| SecuriBench Micro | `repos/securibench-micro` | `6a5a72488ea8` | Java servlet taint benchmark | Classic security microbenchmarks for static analyzers. |
| gosec | `repos/gosec` | `de65614d10a6` | Go security analyzer samples | Useful test cases and taint/performance benchmark scripts, not broad independent ground truth. |
| PyT | `repos/pyt` | `f4ec9e127497` | Python CFG/taint examples | Older but useful for Python microcase taxonomy. |
| Pyre/Pysa | `repos/pyre-check` | `34af3721bc04` | Python type/taint/call graph tests | Strong source for model and taint fixture design. |
| Jelly | `repos/jelly` | `b799ed4f0d68` | JS/TS call graph evaluation | Supports dynamic call graph construction and static/dynamic comparison ideas. |
| BugsJS Dataset | `repos/BugsJS-bug-dataset` | `7abbad3e4df1` | Real JS project bug corpus | Better for project-scale and regression workflows than direct static-analysis ground truth. |

## Repositories From Prior Research Reused Here

| Repository | Local Path | Commit Checked | Why It Still Matters |
|---|---|---:|---|
| CodeQL | `research/data-flow/repos/codeql` | `a84332ac150e` | Rich multi-language query tests and expected outputs; use as reference taxonomy and microcase inspiration. |
| Go x/tools | `research/call-graphs/repos/golang-tools` | `a3954b5c7496` | Go call graph APIs, tests, and analysis package patterns. |
| OPAL | `research/call-graphs/repos/opal` | see call graph index | Java call graph research implementation and test ideas. |
| SootUp/Soot | `research/call-graphs/repos/sootup`, `research/call-graphs/repos/soot` | see call graph index | Java program analysis architecture and call graph evaluation inputs. |

## Cloning Policy

Do not commit cloned benchmark repositories.

Commit only:

- adapter code;
- pinned source URLs and commits;
- small manifest files;
- generated summaries;
- downloaded papers where allowed by the existing research convention.

The benchmark source itself should stay outside git history to avoid repository bloat, license ambiguity, and accidental vendoring.
