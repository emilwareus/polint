---
phase: 25-rule-manifest-inspect-and-test-skeleton
plan: 01
subsystem: rule-authoring
tags: [rust, sdk, macros, rule-manifest]

requires:
  - phase: 24-sdk-public-api-and-rule-authoring-contract
    provides: typed SDK fact-view rule authoring surface
provides:
  - crate-private rule manifest projection
  - hidden macro bridge for generated fact-view metadata
  - deterministic manifest tests for capabilities, fact views, and options
affects: [phase-25, rule-authoring, sdk-private-bridge, macros]

tech-stack:
  added: []
  patterns:
    - crate-private manifest model with a doc-hidden generated-code carrier
    - deterministic sorting at manifest projection boundaries
    - macro-derived fact-view requirements from typed SDK parameters

key-files:
  created:
    - crates/polint/src/rule_manifest.rs
  modified:
    - crates/polint/src/lib.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/sdk/mod.rs
    - crates/polint-macros/src/lib.rs

requirements-completed: [SAE-FND-06]

completed: 2026-05-18
---

# Phase 25 Plan 01: Rule Manifest Foundation Summary

Added the internal manifest substrate used by later `inspect` and `test` work without changing the normal public rule-authoring API.

## Accomplishments

- Added crate-private `rule_manifest` types for rule metadata, typed fact-view requirements, capability rows, and resolved option metadata.
- Extended opaque `Rule` values with a crate-private `manifest(options)` projection.
- Added a hidden `sdk::__private::make_rule_with_manifest` bridge used by macro output.
- Updated `#[polint::rule]` expansion to emit deterministic fact-view rows with `view_type`, `canonical_path`, `capability`, and `parameter_name`.
- Kept `RuleManifest` and internal manifest structs out of `sdk::prelude`; only a doc-hidden generated-code carrier exists under `sdk::__private`.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p polint-macros --locked`
- `cargo test -p polint --lib rule_manifest --locked`
- `cargo test -p polint --lib sdk_prelude_exports_rule_authoring_surface --locked`
- Acceptance greps verified the internal module registration, required manifest fields, hidden bridge, macro emission, and no bare public declarations in `rule_manifest.rs`.

## Deviations

- Used a doc-hidden `sdk::__private::FactViewRequirement` carrier so generated code in external rule packs can compile while the concrete manifest model remains crate-private.

## Next

Plan 25-02 can build `polint inspect rule` on top of `Rule::manifest(...)` and serialize the public CLI JSON contract.
