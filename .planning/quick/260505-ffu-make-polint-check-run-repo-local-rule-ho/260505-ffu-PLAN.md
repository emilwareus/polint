# Quick Task 260505-ffu: Make polint check run repo-local rule hosts directly

**Date:** 2026-05-05
**Status:** Complete

## Goal

Make the installed `polint` CLI the user-facing entry point for repo-local
rules. Users should run `polint check`, not `cargo run --manifest-path ...`.

## Tasks

1. Add local rule-host discovery and execution to `polint check`.
2. Keep diagnostics aggregated through normal polint output formats and exit
   code handling.
3. Add focused integration tests and update docs/examples away from cargo-run
   lint commands.
4. Keep profile selection explicit: no selected profile means run every
   discovered rule; selected profiles must match exactly.
