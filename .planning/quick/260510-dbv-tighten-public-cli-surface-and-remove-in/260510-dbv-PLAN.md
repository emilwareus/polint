# Quick Task 260510-dbv: Tighten Public CLI Surface And Remove Internal Debug Commands

**Date:** 2026-05-10
**Status:** In progress

## Goal

Reduce accidental public API commitments in the polint CLI. Only expose commands
that are complete, valuable to users, and intentionally supportable.

## Tasks

1. Record API-surface guidance in `AGENTS.md`.
2. Remove top-level internal/debug CLI commands: graph export, fixture-test alias,
   profiling alias, and public explain commands tied to implementation details.
3. Update docs, examples, generated skill text, and tests so advertised workflows
   use supported commands only.
4. Verify with formatting and focused CLI tests.
