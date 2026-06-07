---
quick_id: 260606-qjp
slug: deeply-research-jelly-js-semantics-gaps-
status: planned
created: 2026-06-06
description: Deeply research Jelly JS semantics gaps and add failing unit probes
---

# Quick Task 260606-qjp: Deep Jelly JS Semantics Research

## Objective

Research how Jelly implements the remaining JS/TS call graph semantics more
deeply, identify fixture-backed gaps, port representative Jelly test obligations
into Rust unit-style probes, and update the implementation plan.

## Tasks

1. Inspect Jelly's analysis internals for token flow, constraint variables,
   call graph registration, modules, native models, promises, generators, and
   recovery patches.
2. Inspect Jelly test fixtures for representative missing semantics.
3. Port selected Jelly fixture obligations to ignored Rust unit-style tests that
   fail against current polint behavior.
4. Update `performance/2026-06-06-jelly-gap-closure-research.md` with source
   findings, current failing probes, parser-vs-semantics diagnosis, and a
   concrete implementation plan.

## Verification

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- `cargo test -p polint analysis::calls::ts_value_flows::tests::jelly_gap --lib --locked -- --ignored --nocapture`
- `git diff --check`
