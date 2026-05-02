# Quick Task 260502-qsd: Make examples self-contained with one local rule each

**Date:** 2026-05-02
**Status:** Complete

## Goal

Change the examples so each example directory behaves like its own repository:
it owns its fixture code, `.polint.toml`, and exactly one local Rust policy
rule under `.polint/rules/`.

## Tasks

1. Remove the shared `examples/rules` policy pack.
2. Add reusable native-runner infrastructure that local rule crates can call
   without bundling any product policy rules.
3. Move/copy the actual example rule code into the owning example directories,
   one rule crate per example.
4. Update example configs, READMEs, and CLI e2e tests to run each example
   through its own local rule crate.
5. Run formatting, workspace tests, and clippy before committing.
