# Phase 62 Context: Promotion Gate, Boundary Proof, and Closeout

## Goal

Finish v1.4 by proving the preview policy-query surface is useful, documented,
externally usable, and bounded by public-surface gates.

## Requirements

- VAL-03: The public-surface leak gate proves raw CFG, call graph, semantic
  graph, data-flow graph, solver, provider, `AnalysisDb`, and private IDs are
  not reachable from supported SDK, CLI, runner, README, generated skill text,
  or docs/facts surfaces.
- VAL-04: Milestone exit verification runs full workspace tests, formatting,
  clippy, temp-repo SDK tests, cache invalidation tests, docs/example smoke
  tests, and deterministic repeated-run checks for the flagship policies.

## Current State

Phases 55-61 introduced and validated the preview SDK views, backed events and
calls queries, same-function control-flow policies, bounded data-flow policies,
normalized evidence/unknown semantics, flagship templates, and consolidated
public docs.

## Exit Criteria

- Public-surface leak gates pass.
- A deterministic flagship-template regression exists and passes.
- Full local verification is recorded.
- A milestone audit documents ready preview APIs, preview limits, and v1.5
  stabilization follow-ups.
- Requirements, roadmap, and state are updated.

