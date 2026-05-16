# Evaluation Harness Research

Date: 2026-05-15

This folder researches how polint should evaluate a native, multi-language, agent-extensible static-analysis engine.

The research question is not only "which benchmark should we run?" It is:

```text
How do we know that native analysis, rules, and agent-authored Rust extensions
actually improve precision, recall, evidence quality, runtime, memory, and cache
behavior across real codebases?
```

## Executive Conclusion

Use an **external-benchmark-first** evaluation strategy, but do not make it external-only.

External benchmarks should be the primary source for scanner-level outcome evaluation:

- OWASP Benchmark Java and BenchmarkPython for synthetic, runnable vulnerability cases.
- SecBench.js for executable server-side JavaScript package vulnerabilities.
- RealVuln for real Python web applications with false-positive traps.
- SecuriBench Micro, DroidBench, CryptoAPI-Bench, Juliet/SARD, CodeQL tests, gosec samples, and Pyre/Pysa tests for language- and analysis-family-specific coverage.
- Jelly-style dynamic call graph comparison for JS/TS call graph recall.
- CrossCommitVuln-Bench and SecCodeBench as forward-looking benchmarks for temporal and agentic workflows.

But external suites cannot fully measure polint's most important differentiator:

```text
default analysis
  + explicit unknowns
  + repo-local Rust extensions
  + validated extension merges
  + provenance-aware facts
  + cache/invalidation correctness
```

Those engine properties require a small native polint fixture layer in addition to external adapters.

## Folder Structure

| Path | Purpose |
|---|---|
| `FINAL-REPORT.md` | Main findings and recommendation. |
| `RECOMMENDED_IMPLEMENTATION.md` | Concrete implementation path for a hidden/internal `polint eval` harness. |
| `RESEARCH-ANALYSIS.md` | Deeper benchmark and metric analysis. |
| `STANDARD.md` | Standard vocabulary and manifest schema for benchmark adapters. |
| `REPO-INDEX.md` | OSS repositories cloned and inspected. |
| `PAPER-INDEX.md` | Papers, benchmark sites, and research sources. |
| `VALIDATION.md` | What was validated, what remains risky, and how to keep references accurate. |
| `algorithms/` | Pseudo-code for scoring, matching, scheduling, baselines, and extension deltas. |
| `benchmarks/` | Language-by-language external benchmark map. |
| `implementation/` | Suggested internal architecture and phased implementation. |
| `oss/` | Comparison of inspected OSS benchmark suites and priority ranking. |
| `decisions/` | Decision log. |
| `papers/` | Downloaded research PDFs. |
| `repos/` | Local clones of benchmark and implementation repositories. This directory is gitignored. |

## How To Read This

Start with `FINAL-REPORT.md`. Then read:

1. `RECOMMENDED_IMPLEMENTATION.md` for the build path.
2. `STANDARD.md` for the shared schema.
3. `RESEARCH-ANALYSIS.md` for benchmark tradeoffs and accuracy caveats.
4. `algorithms/scoring-and-matching.md` for concrete scoring logic.
5. `REPO-INDEX.md` and `PAPER-INDEX.md` for source traceability.
