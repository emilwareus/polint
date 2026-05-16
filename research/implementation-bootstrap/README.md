# Implementation Bootstrap Research

Date: 2026-05-16

## Research Question

How should polint implement the first native semantic-analysis bootstrap in
modern Rust without building itself into a corner?

The concrete scope is the first vertical slice after the research tracks:

- semantic MIR;
- place identity;
- direct call facts;
- P0 abstract domains;
- direct summaries;
- minimal cache and invalidation;
- model-extension sinks;
- Rust module/API shape that preserves performance and evolvability.

## Why It Matters

The earlier research establishes what polint should become: a native,
multi-language static-analysis engine that AI agents can extend with repo-local
Rust analysis models. This research asks how to start coding that engine inside
the current repository without accidentally freezing weak internal APIs,
overloading `AnalysisDb`, or exposing premature SDK contracts.

The core implementation risk is not "can we add more facts?" The risk is
adding the first semantic facts in a way that makes later call graph, data flow,
summaries, abstract domains, and extension merges expensive to change.

## Status

Complete as a research/design pass.

No product code was changed. The output is an implementation-ready design review
grounded in:

- current polint source-code inspection;
- existing call graph, data-flow, analysis-kernel, CFG, type/alias, summaries,
  and abstract-interpretation research;
- the local Rust best-practices skill;
- official Rust API and language-design guidance.

## Practical Conclusion

Use an internal `analysis` kernel module with typed fact stores, stable keys,
metadata sidecars, enum-driven native provider scheduling, typed errors, and
deterministic merge validation.

Do not start by expanding `FunctionFact.calls` or dumping semantic facts into
the current monolithic `AnalysisDb`. Keep public SDK views reserved until the
internal fact family has documentation, fixtures, cache tests, and temp-repo
rule tests.

## Files

- [FINAL-REPORT.md](FINAL-REPORT.md): main synthesis and critical code review.
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): concrete
  implementation path.
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): deeper Rust design and
  algorithmic analysis.
- [STANDARD.md](STANDARD.md): vocabulary and report structure.
- [VALIDATION.md](VALIDATION.md): validation pass and remaining gaps.
- [REPO-INDEX.md](REPO-INDEX.md): local source files inspected.
- [PAPER-INDEX.md](PAPER-INDEX.md): official docs and local research sources.
- [implementation/RUST-ARCHITECTURE.md](implementation/RUST-ARCHITECTURE.md):
  module/API design notes.
- [implementation/FIRST-VERTICAL-SLICE.md](implementation/FIRST-VERTICAL-SLICE.md):
  coding sequence.
- [decisions/DECISIONS.md](decisions/DECISIONS.md): accepted/rejected design
  decisions.
