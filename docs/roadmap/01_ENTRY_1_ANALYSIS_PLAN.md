# Entry 1: Capability-Driven Analysis Plan

## Goal

Make `Capabilities` operational. A rule declaring a capability should cause the
engine to plan, harvest, cache, and expose the requested facts.

## Why

Today capabilities are mostly descriptive. This weakens the product promise and
prevents large-repo performance from scaling with what rules actually need.

## Difficulty

**L**: shared model plus runner, adapter, cache, CLI, and test changes.

## What To Build

- `AnalysisPlan`
- `LanguagePlan`
- capability union logic over enabled rules
- deterministic analysis-plan encoding
- setup probes for language-specific deep facts
- `polint explain plan`

## Build Method

1. Add `AnalysisPlan` and `LanguagePlan` structs in `crates/polint/src/core/mod.rs`.
2. Add `Capabilities::union` and a deterministic encoder.
3. Build the plan in `runner::analyze_and_run` before adapter execution.
4. Thread the plan through Go and TS/JS adapter entrypoints.
5. Keep parser diagnostics on by default.
6. Gate optional harvesters behind plan flags.
7. Include the encoded plan in `rule_hash` or a new cache digest component.
8. Add `polint explain plan --profile <name>`.
9. Add external temp-repo tests proving capability changes affect the plan.

## Done When

- Rules can explain which capabilities they requested.
- Adapters receive an explicit plan.
- Cache keys change when the plan changes.
- Missing setup produces structured diagnostics.
- Existing Go and TS/JS rules still pass.

## Notes

Go `packages.Config.Mode` is a good reference for capability-style loading.
Oxc semantic analysis already separates symbol, reference, module, and CFG work
conceptually.
