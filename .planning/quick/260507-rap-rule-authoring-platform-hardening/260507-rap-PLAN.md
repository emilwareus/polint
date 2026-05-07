# Quick Task 260507-rap: Rule authoring platform hardening

**Date:** 2026-05-07
**Status:** Complete

## Goal

Make the rule-authoring story less example-coupled and more provably useful to
external repo-local rule authors.

## Tasks

1. Add arbitrary per-rule settings to the public rule options surface.
2. Add a temp-repo integration test proving a generated external rule can use
   SDK facts, settings, and diagnostics.
3. Make capability docs honest about current behavior and unavailable facts.
4. Add fact reference docs for functions, imports, branches, TS/JS, and literals.
5. Update AGENTS guidance so future examples preserve the external-consumer
   contract.
6. Ensure new rule config participates in deterministic cache/rule digests.
7. Run focused tests, docs, clippy, and workspace verification.

## Verification

- `cargo fmt --all`
- `cargo test -p polint --lib --locked rule_config_preserves_custom_settings -- --nocapture`
- `cargo test -p polint --test cli --locked external_generated_rule_uses_sdk_facts_settings_and_reports_diagnostic -- --nocapture`
- `cargo test -p polint --lib --locked`
- `cargo test -p polint --test cli --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `RUSTDOCFLAGS="-D warnings" cargo doc -p polint --all-features --no-deps --locked`
- `cargo test --workspace --locked`
- `cargo test --workspace --all-features --locked`
