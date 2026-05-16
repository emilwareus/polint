# Program Slicing, Path Explanation, And Evidence

Date: 2026-05-16

## Research Question

How should polint compute and present program slices, path explanations, and
diagnostic evidence for a native multi-language analysis engine that AI agents
can extend with repo-local Rust code?

This research covers:

- backward slices: "what influenced this value or diagnostic?"
- forward slices: "what can this statement/value influence?"
- chops: "what region connects this source to this sink?"
- thin slices: small producer-focused slices for humans and agents;
- full dependence slices: data plus control dependence for stronger semantic
  explanation;
- path explanations: concrete source-to-sink or fact-to-diagnostic paths;
- evidence modeling: provenance, uncertainty, extension trust, and renderer
  output.

## Why This Matters

Call graphs, data flow, summaries, type/value facts, and abstract domains only
become useful to rule authors when diagnostics can explain why the engine reached
a conclusion. AI agents need the same thing at a larger scale: a slice or path
is the unit an agent can inspect, challenge, repair with an extension, or turn
into a code change.

The core conclusion is:

```text
Slicing is not a standalone feature.

It is the evidence layer over:
  semantic operations
  + CFG/control dependence
  + def-use/data dependence
  + call graph
  + summaries
  + data-flow edges
  + type/value/alias facts
  + extension/model facts
  + provenance and uncertainty.
```

## Status

Completed research package:

- foundational papers downloaded in `papers/`;
- recent neural/LLM slicing papers downloaded for comparison;
- reference implementations cloned under ignored `repos/`;
- implementation reports for WALA, CodeQL, Joern, Semgrep, Frama-C, and
  JavaSlicer;
- recommended polint implementation path;
- validation plan and roadmap update.

The cloned repositories are intentionally ignored by git through
`research/*/repos/`.

## Most Important Recommendation

Build `analysis::evidence` and `analysis::slicing` as internal query layers over
the native semantic fact store. Do not expose raw ASTs, do not start by trying to
generate executable slices, and do not rely on LLM-generated slices as trusted
engine facts.

Start with a small but precise internal model:

```text
EvidenceNode
EvidenceEdge
EvidenceBundle
SliceQuery
SliceResult
PathQuery
PathResult
```

Then attach evidence bundles to diagnostics and JSON/SARIF output. Public SDK
views should come later, after the internal format survives real diagnostics.

## Key Files

- `FINAL-REPORT.md`: main synthesis and product recommendation.
- `RECOMMENDED_IMPLEMENTATION.md`: concrete implementation path.
- `RESEARCH-ANALYSIS.md`: algorithms, complexity, accuracy, and failure modes.
- `STANDARD.md`: vocabulary and report structure.
- `REPO-INDEX.md`: cloned implementation index and inspected source paths.
- `PAPER-INDEX.md`: downloaded papers, docs, and source URLs.
- `VALIDATION.md`: fixture, benchmark, and extension validation plan.
- `algorithms/core-algorithms.md`: stripped-down pseudo-code for the core
  algorithms.
- `implementation/POLINT-EVIDENCE-ARCHITECTURE.md`: implementation-ready data
  model and provider plan.
- `decisions/001-evidence-is-the-user-facing-product.md`: decision record.
