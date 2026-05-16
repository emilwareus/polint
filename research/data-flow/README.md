# Data Flow Research

Date: 2026-05-15

Bootstrap integration update: 2026-05-16

This folder is a research package for adding native data-flow facts to polint. It is intentionally parallel to `research/call-graphs/`, because useful interprocedural data flow depends on call graph precision, symbol identity, CFG quality, and language setup.

## Main Finding

The practical state of the art is not a single taint algorithm. The strongest systems combine:

1. a common fact/IR layer for symbols, references, calls, CFG, places, values, and effects;
2. sparse value-flow edges instead of dense per-rule CFG propagation everywhere;
3. local data-flow first, then function summaries, then interprocedural fixed points;
4. explicit source/sink/sanitizer/barrier models;
5. bounded access paths and field/property sensitivity;
6. call-graph-aware interprocedural propagation;
7. first-class unknown/havoc facts for dynamic calls, reflection, missing setup, and unsupported language features;
8. provenance, precision, and algorithm labels on every node, edge, and path.

For polint, the right product is ultimately a typed `DataFlow<'_>` SDK view backed by native Rust providers. The 2026-05-16 bootstrap revision is stricter about sequencing: first build internal `analysis::data_flow` facts that consume MIR, `PlaceId`, CFG, call-site/call-target facts, abstract domains, summaries, and validated extension sinks. Promote `DataFlow<'_>` only after fixtures, cache tests, docs, and temp-repo SDK tests prove the internal facts are stable.

This is a product shift from black-box static analysis to an agent-extensible analysis framework. The native engine should have strong defaults, but maximum accuracy should come from validated repo-local models that bind to symbols, calls, CFG nodes, spans, and call graph facts.

## Contents

- [FINAL-REPORT.md](FINAL-REPORT.md): final synthesis and architecture recommendation.
- [AGENT-EXTENSIBLE-DATA-FLOW.md](AGENT-EXTENSIBLE-DATA-FLOW.md): how repo-local data-flow models fit the intended product path.
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): deeper accuracy, complexity, benchmark, and tradeoff analysis.
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): concrete native Rust implementation plan for polint.
- [REPO-INDEX.md](REPO-INDEX.md): OSS repositories cloned and inspected.
- [PAPER-INDEX.md](PAPER-INDEX.md): papers and docs downloaded locally.
- [VALIDATION.md](VALIDATION.md): validation pass for references, source paths, and claims.
- [STANDARD.md](STANDARD.md): standardized vocabulary and comparison template.
- [algorithms/core-algorithms.md](algorithms/core-algorithms.md): Python-ish pseudocode for core algorithms.
- [oss/implementation-comparison.md](oss/implementation-comparison.md): standardized OSS implementation notes and pseudocode.
- [languages/go.md](languages/go.md): Go-specific data-flow notes.
- [languages/typescript-javascript.md](languages/typescript-javascript.md): JS/TS-specific data-flow notes.
- [languages/java.md](languages/java.md): Java/JVM-specific data-flow notes.
- [languages/python.md](languages/python.md): Python-specific data-flow notes.
- [implementation/polint-data-flow-path.md](implementation/polint-data-flow-path.md): implementation path tied to polint's SDK/capability model.
- [implementation/BOOTSTRAP-INTEGRATION.md](implementation/BOOTSTRAP-INTEGRATION.md): revised implementation design against the analysis kernel, call graph, CFG, summaries, abstract domains, extension model sinks, cache keys, and evaluation harness.

## Local Clone Policy

All third-party repositories are under `research/data-flow/repos/`, which is gitignored. Research notes reference exact local paths and commit hashes, but the cloned source is not intended to be committed.
