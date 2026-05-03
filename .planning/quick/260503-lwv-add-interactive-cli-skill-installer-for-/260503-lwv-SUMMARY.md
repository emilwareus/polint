# Quick Task 260503-lwv Summary

**Date:** 2026-05-03
**Status:** Complete

## Completed

- Added `polint add-skill` with interactive agent selection for Claude Code,
  Codex, or both.
- Added scriptable options: `--agent claude`, `--agent codex`, `--all`, and
  `--force`.
- Added safe repo-local installation paths:
  `.claude/skills/polint/SKILL.md` for Claude Code and
  `.agents/skills/polint/SKILL.md` for Codex by default, with
  `.codex/skills/` honored when it already exists.
- Added generated skill content that explains CLI usage, rule layout, SDK rule
  authoring, config patterns, and agent constraints around no built-in policy
  rules.
- Documented the workflow in the README.

## Verification

- `cargo run -p polint-cli -- add-skill --help`
- `cargo test -p polint-cli add_skill`
- isolated temp-repo smoke test for `add-skill --all --force`
- `cargo fmt -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `git diff --check`
- `cargo test --workspace`
