# Quick Task 260502-dto: Improve examples with real minimal linted code, README coverage, and CLI e2e tests

**Date:** 2026-05-02
**Status:** Complete

## Goal

Make the checked-in examples runnable mini repositories with real Go or TypeScript/JavaScript source code, focused README instructions, and CLI e2e tests that prove the examples emit the intended diagnostics.

## Tasks

1. Add or update example directories so each contains `.polint.toml`, minimal linted source code, and a README.
2. Cover the built-in example rule families with real checked-in example code.
3. Add CLI e2e tests that run `polint check` against the checked-in example directories and assert expected rule IDs and files.
4. Run focused CLI tests and workspace formatting/checks as appropriate.
