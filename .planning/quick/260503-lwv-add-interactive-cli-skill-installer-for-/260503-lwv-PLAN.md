# Quick Task 260503-lwv: Add interactive CLI skill installer for Claude and Codex

**Date:** 2026-05-03
**Status:** Complete

## Goal

Add a `polint` CLI command that installs a repo-local AI-agent skill explaining
how agents should use the CLI and write local rules. The command should be
interactive by default and support Claude and Codex first.

## Tasks

1. Add CLI command parsing and implementation for installing the skill into
   current-repo agent skill folders, with non-interactive options for tests and
   automation.
2. Add integration tests for Claude, Codex, combined installation, overwrite
   protection, unsafe target handling, and interactive selection.
3. Update the README with the new workflow and run focused verification.
