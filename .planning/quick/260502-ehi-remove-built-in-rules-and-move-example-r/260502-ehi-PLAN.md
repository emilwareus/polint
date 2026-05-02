# Quick Task 260502-ehi: Remove built-in rules and move example rules into examples

**Date:** 2026-05-02
**Status:** Complete

## Goal

Remove globally bundled example rules from the shipped `polint` CLI. Example policies should live in `examples/` as example rule code, and the product should stay a framework with no built-in lint policy pack.

## Tasks

1. Move `polint-rules` out of `crates/` and into `examples/`, then make it an example rule crate rather than a CLI dependency.
2. Update `polint-cli` and default config/docs so no `examples/*` rules are registered or implied by default.
3. Update examples and tests so example policies are proven through the example rule crate/runner rather than through bundled CLI rules.
4. Run formatting, clippy, and workspace tests.
