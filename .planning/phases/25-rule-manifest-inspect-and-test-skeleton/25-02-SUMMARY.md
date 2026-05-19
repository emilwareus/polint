---
phase: 25-rule-manifest-inspect-and-test-skeleton
plan: 02
subsystem: rule-authoring-cli
tags: [rust, cli, json-schema, rule-manifest]

requires:
  - phase: 25-rule-manifest-inspect-and-test-skeleton
    plan: 01
    provides: internal rule manifest projection
provides:
  - public `polint inspect rule --format json`
  - local rule-host `inspect rule` delegation target
  - rule inspect JSON schema
  - temp-repo integration proof for macro-derived fact views
affects: [phase-25, cli, runner, rule-authoring, schemas]

tech-stack:
  added: []
  patterns:
    - parent CLI delegates repo-local manifest inspection to the child rule host
    - public JSON wire types are separate from internal manifest types
    - inspect builds an analysis plan for capability support without running analysis

key-files:
  created:
    - docs/schemas/polint-rule-inspect-v1.json
  modified:
    - crates/polint/src/rule_manifest.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs

requirements-completed: [SAE-FND-06]

completed: 2026-05-18
---

# Phase 25 Plan 02: Inspect Rule Summary

Added the first supported rule-manifest inspection command: `polint inspect rule --format json`.

## Accomplishments

- Added stable inspect wire types and schema URL in `rule_manifest.rs`.
- Added `docs/schemas/polint-rule-inspect-v1.json` for the public JSON contract.
- Added `polint-local-rules inspect rule --format json`, which loads config/options and builds capability support rows without parsing sources or running rules.
- Added parent `polint inspect rule --format json`, which discovers repo-local rule hosts, invokes child inspect, merges manifests, and emits deterministic JSON.
- Added temp-repo CLI coverage proving macro-derived `Imports<'_>` and `StringLiterals<'_>` fact views appear in inspect output while the fixture uses only `polint::sdk::prelude::*` and `polint::runner::run_cli`.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p polint --lib inspect_rule_report --locked`
- `cargo test -p polint --lib runner --locked`
- `cargo test -p polint --test cli inspect_rule_manifest_json_is_stable_for_local_rules --locked`
- `cargo test -p polint --test cli top_level_help_only_lists_supported_public_commands --locked`
- Acceptance greps verified schema fields, inspect command wiring, no internal inspect wire leaks, and temp-repo manifest assertions.

## Deviations

- The parent applies `--rule` filtering after merging child manifests so a selector only needs to match one local rule host. Direct child `polint-local-rules inspect rule --rule ...` still fails deterministically when no local rule matches.

## Next

Plan 25-03 can add `polint test` using the same local rule-host delegation model, with fixture execution through the real check path.
