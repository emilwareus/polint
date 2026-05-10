# Quick Task 260510-eur: Prompt Before Overwriting Existing Installed polint Skills

**Date:** 2026-05-10
**Status:** In progress

## Goal

Change `polint add-skill` so an existing installed skill triggers an interactive
overwrite prompt instead of immediately failing, while preserving `--force` and
symlink safety behavior.

## Tasks

1. Add overwrite confirmation to `crates/polint/src/cli/skill.rs`.
2. Update CLI integration tests for decline, accept, and `--force`.
3. Run focused tests plus formatting and clippy.
