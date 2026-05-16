# Incremental Query Engine And Caching Research

This folder researches how polint should build incremental analysis, demand
queries, persistent caching, and invalidation beyond the first minimal kernel
design.

The practical conclusion is:

```text
native layered incrementality first
  -> dependency-digest cache for fact layers
  -> dependency index and invalidation planner
  -> demand query engine for expensive facts
  -> summary SCC cache with equality/backdating
  -> extension-aware cache validation
  -> optional daemon red-green validation later
  -> optional relation/differential backend for high-volume recursive facts
```

Do not adopt Salsa, Souffle, Differential Dataflow, or a build-system engine as
the first hard dependency. Use their algorithms and implementation lessons, but
keep polint's cache keys, fact layers, extension lifecycle, provenance, and
precision contracts native.

## Reports

- [FINAL-REPORT.md](FINAL-REPORT.md): executive research report and decision.
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): concrete
  implementation path and Rust module shape.
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): deeper algorithm and tool
  analysis.
- [REPO-INDEX.md](REPO-INDEX.md): OSS repositories cloned and inspected.
- [PAPER-INDEX.md](PAPER-INDEX.md): research papers and official sources.
- [STANDARD.md](STANDARD.md): standard vocabulary for reviewing incremental
  systems.
- [VALIDATION.md](VALIDATION.md): validation notes and commands.
- [SUBAGENT-FINDINGS.md](SUBAGENT-FINDINGS.md): process notes.

## Supporting Notes

- [algorithms/core-algorithms.md](algorithms/core-algorithms.md): stripped-down
  Python-ish pseudocode for invalidation, red-green verification, demand
  queries, layer caches, and SCC summaries.
- [implementation/POLINT-INCREMENTAL-QUERY-ENGINE.md](implementation/POLINT-INCREMENTAL-QUERY-ENGINE.md):
  implementation-ready design for polint.
- [decisions/001-layered-incrementality-not-salsa-first.md](decisions/001-layered-incrementality-not-salsa-first.md):
  architecture decision record.
- [benchmarks/incremental-benchmark-plan.md](benchmarks/incremental-benchmark-plan.md):
  benchmark plan.

## Core Design Rule

Incrementality is not just a performance optimization in polint. It is part of
analysis truthfulness.

When an AI agent edits a Rust extension, model file, rule option, framework
mapping, summary, source file, lockfile, or language lifecycle setting, the
engine must know which facts are still valid, which facts need verification,
which facts must be recomputed, and which diagnostics must be quarantined.

If the cache cannot explain why a reused fact is valid, the cache should not be
trusted.
