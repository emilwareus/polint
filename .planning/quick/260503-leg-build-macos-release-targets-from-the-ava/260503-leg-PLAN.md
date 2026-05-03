# Quick Task 260503-leg: Build macOS release targets from the available macOS runner

**Date:** 2026-05-03
**Status:** Complete

## Goal

Avoid blocking the private release publication on a separate Intel macOS hosted
runner by building macOS x64 and macOS ARM assets from the available macOS runner
with explicit Rust target triples.

## Tasks

1. Replace the `macos-13` build entry with an explicit
   `x86_64-apple-darwin` target built on `macos-14`.
2. Build every release asset using its configured target triple and package the
   target-specific binary path.
3. Validate workflow YAML and push so the main-only release workflow reruns.
