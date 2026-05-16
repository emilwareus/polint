# Type, Value, Points-To, And Alias Analysis Research

Date: 2026-05-15

This folder researches how polint should implement native type, value, points-to, and alias analysis across Go, TypeScript/JavaScript, Python, Java/JVM, and future languages.

The goal is not to embed Ty, Pyright, CodeQL, WALA, Soot, SVF, or any other external analyzer. The goal is to learn from the best available implementations and papers, then design a native Rust analysis engine with typed fact layers, explicit uncertainty, strong extension hooks, and validation against external oracles.

## Recommended Reading Order

1. [FINAL-REPORT.md](FINAL-REPORT.md): executive conclusions and design decisions.
2. [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): concrete native implementation path for polint.
3. [STANDARD.md](STANDARD.md): standardized vocabulary used to compare implementations.
4. [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): deeper research synthesis, accuracy/cost tradeoffs, and assumption review.
5. [oss/implementation-reports.md](oss/implementation-reports.md): per-tool implementation findings with source paths.
6. [algorithms/core-algorithms.md](algorithms/core-algorithms.md): algorithms and pseudo-code.
7. [languages/python.md](languages/python.md), [languages/typescript-javascript.md](languages/typescript-javascript.md), [languages/go.md](languages/go.md), [languages/java-jvm.md](languages/java-jvm.md), [languages/native-neutral.md](languages/native-neutral.md): language-specific reports.
8. [REPO-INDEX.md](REPO-INDEX.md): cloned OSS repositories and inspected paths.
9. [PAPER-INDEX.md](PAPER-INDEX.md): papers, official docs, and downloaded artifacts.
10. [VALIDATION.md](VALIDATION.md): validation, caveats, and source checks.

## Core Recommendation

Implement this as layered native facts, not as one monolithic "alias analysis":

```text
semantic index
  -> declared/resolved type facts
  -> local CFG and place facts
  -> value facts and allocation tokens
  -> local flow/narrowing facts
  -> summary facts
  -> bounded points-to facts
  -> derived alias facts
  -> call graph/data-flow/effect consumers
```

The practical first version should be exact where local syntax and type facts are exact, conservative where dynamic behavior exists, and explicit when the engine cannot know something. Agent-authored Rust extensions should be able to add model facts, summaries, type/value hints, call targets, and points-to constraints through validated typed sinks.

## Cloned Repositories

The implementation repositories are cloned under `research/type-alias-points-to/repos/`. That path is intentionally ignored by git through the existing `research/*/repos/` rule.

## Downloaded Papers And Docs

The `papers/` directory contains local snapshots of papers and official documentation used during the research pass. Prefer the canonical links in [PAPER-INDEX.md](PAPER-INDEX.md) when citing.
