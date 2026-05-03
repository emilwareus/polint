# Quick Task 260503-adu Summary

**Date:** 2026-05-03
**Status:** Complete

## Completed

- Rewrote all example READMEs to remove self-contained-directory and
  implementation-path meta-comments.
- Reframed each README around the policy being demonstrated, how to run it,
  the expected finding, and what a real fix would look like.
- Removed stale custom-rule prose about native rule hosts and replaced it with
  SDK-oriented guidance.

## Verification

- `rg -n "self-contained|local rule implementation lives|This directory|implementation lives|uses a tiny native rule host|checked-in rule crate|executable example|\\.polint/rules/.*/src" examples README.md`
- `cargo test -p polint-cli --test cli checked_in_examples_are_runnable_cli_fixtures -- --nocapture`
- `git diff --check`
