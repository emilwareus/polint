# Decision 001: Evidence Is The User-Facing Product

Date: 2026-05-16

## Decision

Treat structured evidence as a first-class internal product surface. Slicing and
path explanation should be implemented to explain diagnostics, expose
uncertainty, and guide agent-authored extensions.

## Context

polint's goal is not to be a sealed black-box analyzer. It is a native
multi-language analysis engine that AI agents can extend with repo-local Rust
code. In that model, a diagnostic without evidence is incomplete. The user or
agent needs to know:

- what facts led to the diagnostic;
- which path connected source to sink;
- which summaries or model edges participated;
- which unknowns or setup gaps remain;
- whether a repo-local extension could improve precision.

## Consequences

Implement:

- typed evidence nodes and edges;
- evidence bundles attached to diagnostics;
- thin slices and path explanations before executable slices;
- provenance and precision on every edge;
- extension merge validation;
- JSON/SARIF renderers from the same internal evidence model.

Avoid:

- one-string explanations;
- single-path-only internal traces;
- raw AST exposure;
- LLM-generated slices as trusted facts;
- unbounded whole-program graph materialization.

## Status

Accepted for research roadmap. Implementation should follow the semantic
bootstrap, CFG/control dependence, def-use/data dependence, call graph, and
summary foundations.
