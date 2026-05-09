# Quick Task 260509-ibk Summary: Static Capability Derivation

**Date:** 2026-05-09
**Status:** Complete

## What Changed

- Added `polint-macros` and the normal `#[polint::rule]` authoring path.
- Added typed SDK fact views in `polint::sdk::facts` and the prelude:
  `Imports`, `StringLiterals`, `JsxAttributes`, `GoTests`,
  `BranchObligations`, `Functions`, `Packages`, `TsComponents`, `TsClasses`,
  plus reserved future views.
- Changed generated rules so capabilities are derived from typed fact-view
  parameters instead of handwritten `Capabilities::new()` declarations.
- Moved broad fact access off the normal `RuleCtx` surface; `RuleCtx` now stays
  focused on diagnostics, options, source paths, and capability/setup metadata.
- Rewrote examples and `polint new-rule` scaffolds to use macro-derived rules.
- Updated docs, generated skill content, README guidance, and AGENTS.md with the
  new rule-authoring contract.

## Verification

- `cargo check --workspace --locked`
- `cargo fmt --all -- --check`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `cargo install --path crates/polint --locked --force`
- Installed `polint --version` reports `polint 0.1.7`
- Ran `polint check --format json --fail-on none` for every checked-in example
- Ran installed-CLI `polint init`, `polint new-rule ts`, `polint explain plan`,
  and `polint check` against a temp repo under `target/`

## Notes

Superseded by quick task `260509-rul`: `Rule` is now an opaque value, not a
public trait. Manual `impl Rule` is no longer retained as a compatibility path;
tests that need precise capability behavior use internal constructors, and
user-facing rules use `#[polint::rule]`.
