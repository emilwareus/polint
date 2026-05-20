# Quick Task 260520-jho: Speed up CI and measure runtime

**Date:** 2026-05-20
**Status:** In progress

## Goal

Reduce PR CI wall-clock time while preserving the important local guarantees:
formatting, MSRV check, clippy, full Linux workspace tests, cross-platform Rust
coverage, install smoke, SARIF, docs, and supply-chain checks.

## Tasks

1. Add Rust build caching to Cargo-heavy CI jobs.
2. Split the old three-OS clippy/test/install matrix into focused jobs:
   Ubuntu clippy, Ubuntu full tests, macOS/Windows platform tests, and Ubuntu
   install smoke.
3. Keep Go setup available where tests can invoke Go tooling.
4. Run local workflow syntax/diff checks and the relevant local validation suite.
5. Push the branch, fetch the new GitHub Actions run, and compare runtime against
   the prior PR run.
