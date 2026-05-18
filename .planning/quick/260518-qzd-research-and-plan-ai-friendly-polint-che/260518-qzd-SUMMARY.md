# Quick Task 260518-qzd: AI-Friendly polint Check Output - Summary

**Date:** 2026-05-18
**Status:** Planned

## Completed

- Researched AI-agent context management and structured-output guidance.
- Inspected current polint CLI, diagnostics, init, rule-host, and generated skill
  code paths.
- Produced a concrete implementation plan for `polint check --format
  ai-friendly`.

## Key Decision

Implement AI-friendly output as a new check format that keeps stdout small and
writes a versioned JSON report under `.polint/output/`. The terminal should show
counts by rule and max 10 examples, then teach the agent to query the saved file
with bounded `jq` commands.

## Artifacts

- Research: `.planning/quick/260518-qzd-research-and-plan-ai-friendly-polint-che/260518-qzd-RESEARCH.md`
- Plan: `.planning/quick/260518-qzd-research-and-plan-ai-friendly-polint-che/260518-qzd-PLAN.md`
