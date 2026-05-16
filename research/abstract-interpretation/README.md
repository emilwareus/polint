# Abstract Interpretation Domains Research

Date: 2026-05-16

This folder researches abstract interpretation domains for polint's native,
multi-language analysis engine.

The core conclusion is that polint should not build one monolithic "value
analysis." It should build a deterministic abstract-domain kernel with small
composable domains, typed fact views, summary projection, explicit precision
labels, and a law-checked extension surface for agent-authored Rust models.

## Files

- [FINAL-REPORT.md](FINAL-REPORT.md): main synthesis and product recommendation.
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): concrete implementation path.
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): algorithms, accuracy, complexity, and rejected paths.
- [STANDARD.md](STANDARD.md): standard vocabulary for domains, states, precision, and reports.
- [REPO-INDEX.md](REPO-INDEX.md): cloned repositories and inspected source paths.
- [PAPER-INDEX.md](PAPER-INDEX.md): papers, docs, and downloaded artifacts.
- [SUBAGENT-FINDINGS.md](SUBAGENT-FINDINGS.md): consolidated parallel research findings.
- [VALIDATION.md](VALIDATION.md): validation, benchmark, and reference-check plan.
- [algorithms/ABSTRACT-INTERPRETATION-ALGORITHMS.md](algorithms/ABSTRACT-INTERPRETATION-ALGORITHMS.md): pseudo-code for the core algorithms.
- [domains/DOMAIN-PRIORITY.md](domains/DOMAIN-PRIORITY.md): domain-by-domain priority and precision plan.
- [tools/IMPLEMENTATION-REPORTS.md](tools/IMPLEMENTATION-REPORTS.md): detailed tool implementation reports.
- [benchmarks/EVALUATION-PLAN.md](benchmarks/EVALUATION-PLAN.md): benchmark suites, metrics, and ground truth.
- [implementation/NATIVE-RUST-PATH.md](implementation/NATIVE-RUST-PATH.md): internal Rust architecture recommendation.
- [implementation/MIR-CONTRACT.md](implementation/MIR-CONTRACT.md): semantic MIR contract required before domains.
- [implementation/SUMMARY-ALGEBRA.md](implementation/SUMMARY-ALGEBRA.md): summary keys, algebra, caller-place substitution, havoc, and invalidation.
- [implementation/EXTENSION-DOMAIN-CONTRACT.md](implementation/EXTENSION-DOMAIN-CONTRACT.md): extension product, isolation, merge, and cache contract.
- [implementation/BOOTSTRAP-SEQUENCE.md](implementation/BOOTSTRAP-SEQUENCE.md): corrected implementation order that avoids circular dependencies.
- [decisions/DECISIONS.md](decisions/DECISIONS.md): decision log.
- [languages/](languages/): language-specific notes for Go, TS/JS, Python, JVM, and Rust.

## Core Decision

Build a native abstract interpretation layer as:

```text
semantic facts + CFG + places + summaries
  -> deterministic fixpoint solver
  -> reduced product of small domains
  -> typed domain fact views
  -> rule diagnostics and extension products
```

The first useful domains should be:

1. reachability and control outcome;
2. nil/null/undefined/None/nullish;
3. boolean truthiness;
4. literal constants;
5. string literal/template/prefix facts;
6. definite assignment / initializedness;
7. numeric intervals, later congruence and packed octagons;
8. object/record/TypedDict/property shape;
9. resource and typestate;
10. path predicates and guard/refinement facts.

Relational numeric domains, disjunctive domains, path focusing, and full
symbolic execution should be opt-in precision tiers, not the default engine
mode.

## Ignored Clones

Reference implementations were cloned under `research/abstract-interpretation/repos/`.
That path is gitignored by the existing `research/*/repos/` rule.
