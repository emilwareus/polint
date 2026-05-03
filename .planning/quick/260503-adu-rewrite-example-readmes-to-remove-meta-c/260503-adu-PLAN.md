# Quick Task 260503-adu: Rewrite example READMEs to remove meta-comments and improve user guidance

**Date:** 2026-05-03
**Status:** Complete

## Goal

Remove boilerplate README text that explains the repository layout instead of
helping users understand the example. Replace it with concise, useful example
documentation: what policy is being demonstrated, how to run it, what finding to
expect, and how a developer would fix the issue.

## Tasks

1. Rewrite every `examples/*/README.md` to remove self-contained-directory and
   implementation-path meta-comments.
2. Keep the run commands users need, but make the surrounding prose focused on
   the policy and diagnostic behavior.
3. Verify no example README still contains the removed meta-comment pattern.
