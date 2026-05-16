# Validation Notes

## What Was Validated

### Local Repository Clones

The researched benchmark repositories were cloned under `research/evaluation-harness/repos/` and commit IDs were recorded in `REPO-INDEX.md`.

Verified local facts:

- BenchmarkJava expected-results file has 2,740 labelled cases plus header.
- BenchmarkPython expected-results file has 1,230 labelled cases plus header.
- SecBench.js local clone contains 704 `.test.js` files.
- BenchmarkUtils contains scorecard code and templates using TP/FN/TN/FP, TPR, FPR, precision, F-score, and `TPR - FPR`.
- RealVuln local README describes 26 Python repos, 796 findings, 676 vulnerabilities, and 120 false-positive traps.
- CrossCommitVuln-Bench local README describes 15 annotated Python CVEs and per-commit/cumulative detection metrics.
- gosec local clone includes sample files, taint tests, and benchmark baseline scripts.
- Pyre/Pysa local clone includes taint model stubs, integration fixtures, call graph tests, CFG tests, and model-generation utilities.
- CodeQL local clone from previous data-flow research includes many `.expected` query-test outputs across Go, Python, JavaScript, and Java.

### Web/Paper Sources

Web searches and source pages were checked on 2026-05-15 for:

- OWASP Benchmark project metrics and scorecard framing.
- SecBench.js ICSE 2023 paper and publication page.
- RealVuln dashboard/repository/paper.
- CrossCommitVuln-Bench arXiv/repository.
- SecCodeBench repository and technical report.
- NIST Juliet and SARD.
- DroidBench, CryptoAPI-Bench, SecuriBench Micro, gosec, Pyre/Pysa, and Jelly repositories.

Downloaded PDFs are listed in `PAPER-INDEX.md`.

## Accuracy Caveats

### Current/Evolving Benchmarks

RealVuln, CrossCommitVuln-Bench, BenchmarkPython, and SecCodeBench are current/evolving projects. Re-check repository commits, schema versions, and public claims before turning this research into code.

### Licenses

The benchmark repositories were cloned for research. Before committing any extracted fixtures, expected outputs, or copied source snippets, check each suite's license. Prefer adapter manifests that reference external checkouts over copying benchmark content.

### Exact Counts

Counts are from current local clones, not necessarily stable project-wide facts. Record suite commits in every evaluation run.

### Dynamic Ground Truth

Dynamic traces and executable exploits are strong evidence, but they are not complete static ground truth. For call graph and data-flow scoring, dynamic-observed edges/paths should not make all static extras false by default.

### Third-Party Expected Outputs

CodeQL, Pyre/Pysa, gosec, and similar project tests are valuable, but their expected outputs reflect those tools' modeling choices. Use them as reference corpora and microcase inspiration, not as an unquestioned definition of polint correctness.

### Synthetic Benchmarks

OWASP, Juliet/SARD, SecuriBench Micro, DroidBench, and CryptoAPI-Bench are useful because they isolate behavior. They are also easier to overfit than real applications. Always pair synthetic scores with real-repo scores when making capability claims.

## Validation Still Needed Before Implementation

- Check licenses for every suite adapter target.
- Decide which external benchmark content can be downloaded by CI and which requires local opt-in.
- Verify Docker/network requirements for SecBench.js, RealVuln, DroidBench, and SecCodeBench.
- Build one adapter prototype for OWASP expected-results CSV and compare its metrics against BenchmarkUtils on synthetic observed outputs.
- Build one native fixture prototype that validates extension rejection and default-vs-extension delta.
- Decide whether `polint eval` lives as a hidden command in `polint` or as an internal `polint-eval` crate.
- Define machine-stable performance baseline policy for CI.
