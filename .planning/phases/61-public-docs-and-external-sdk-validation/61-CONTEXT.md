# Phase 61 Context: Public Docs and External SDK Validation

## Goal

Prove the v1.4 preview policy-query surface is understandable from public docs
and usable from outside-style repo-local rules under `.polint/rules`.

## Requirements

- VAL-01: Each preview view and each query family has at least one temp-repo
  style test where generated `.polint/rules` imports only
  `polint::sdk::prelude::*`, registers through `polint::runner::run_cli`,
  consumes real facts, and asserts diagnostics through
  `polint check --format json`.
- VAL-02: Public docs under `docs/facts/` describe preview status, syntax,
  limits, precision tiers, heuristic behavior, unknown/budget semantics, and
  realistic examples for every new view and query type.

## Current State

Phases 55-60 already added separate docs pages for events, calls,
control-flow, data-flow, evidence, and capability support. CLI tests also
exercise outside-style rules for events, calls, control-flow, data-flow, and
generated templates. Phase 61 should consolidate and harden this into an
explicit public contract rather than adding a second API.

## Design Direction

- Add one consolidated policy-query reference page linked from
  `docs/facts/README.md`.
- Keep per-view docs focused and link to the shared vocabulary page for query
  structs, patterns, precision/status, budgets, unknowns, and template
  starters.
- Add an external SDK matrix test that explicitly covers `Events<'_>`,
  `Calls<'_>`, `ControlFlow<'_>::missing_guard`,
  `ControlFlow<'_>::missing_cleanup`, and `DataFlow<'_>` using only
  `polint::sdk::prelude::*`.

