# Quick Task 260510-eur: Prompt Before Overwriting Existing Installed polint Skills

**Date:** 2026-05-10
**Status:** Complete

## What Changed

- `polint add-skill` now prompts before overwriting an existing installed
  `SKILL.md` when `--force` is not passed.
- Declining the prompt leaves the existing skill untouched and returns the
  existing error message.
- Accepting the prompt overwrites the skill.
- `--force` still overwrites without prompting.
- Symlink refusal behavior remains unchanged.

## Verification

- `cargo test -p polint add_skill --quiet`
- `cargo test -p polint add_skill_prompts_before_overwriting_existing_skill --quiet`
- Direct CLI smoke for first install, decline, accept, and `--force`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
