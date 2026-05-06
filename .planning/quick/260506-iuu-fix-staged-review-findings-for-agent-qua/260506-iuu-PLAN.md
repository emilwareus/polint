# Quick Task 260506-iuu: Fix staged review findings for agent-quality changes

**Date:** 2026-05-06
**Status:** Complete

## Goal

Fix the defects found in the staged agent-quality review and re-review until no
blocking issues remain.

## Tasks

1. Keep SARIF rule tags schema-compliant.
2. Ensure `--max-diagnostics` only caps rendered output and never hides failing
   diagnostics from `--fail-on`.
3. Fix the CI SARIF example binary path and strengthen the SARIF shape check.
4. Keep generated agent skill text aligned with the checked-in skill.
5. Run focused tests, clippy, and a final staged diff review.
