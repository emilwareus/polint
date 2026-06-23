# Phase 62-02 Summary: Deterministic Flagship Template Gate

**Status:** Complete
**Date:** 2026-06-20

## Completed

- Added `new_rule_policy_templates_are_deterministic`.
- The test generates representative flagship templates in a temp repo:
  `secret-to-log`, `sensitive-write-guard`, and `raw-reachable-api`.
- The generated rule pack is external-user shaped: `.polint/rules`, public SDK
  imports, local `polint` dependency rewrite, and no internal modules.
- The test runs `polint test --no-cache --format json` twice and asserts stable
  summary and case payloads.

## Verification

- `cargo test -p polint --test cli new_rule_policy_templates_are_deterministic --locked`

