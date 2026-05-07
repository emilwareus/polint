# Quick Task 260507-rap Summary

**Date:** 2026-05-07
**Status:** Complete

## Completed

- Added `RuleOptions::settings` as the public arbitrary TOML settings surface
  for rule-specific config.
- Preserved unknown `[[rules.config]]` fields through config parsing and runner
  option mapping.
- Added a temp-repo external-consumer CLI test that generates a rule pack, points
  it at the local `polint` crate as an outside dependency, reads
  `ctx.options().settings`, consumes string literal facts, and emits a JSON
  diagnostic.
- Included custom rule settings and newer config sections in deterministic cache
  digests, with regression coverage for arbitrary TOML keys and string-list
  boundary cases.
- Clarified capability docs so unavailable fact families are not implied as
  provided models.
- Improved generated rule templates with a diagnostic/reporting hint and custom
  settings pointer.
- Added fact reference docs for functions, imports, branches, TS/JS facts, and
  literals/JSX attributes.
- Updated README, consumer setup docs, installed skill text, AGENTS guidance, and
  the rule-authoring review document.

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
