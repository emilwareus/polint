# Quick Task 260509-typed: Typed Future Capability Contract

**Date:** 2026-05-09
**Mode:** quick local execution

## Goal

Make the static rule contract consistent for future analysis capabilities before release: facts should come from typed views, unsupported hard capabilities should not run rules with empty placeholder facts, and docs should not point future work toward `RuleCtx` fact helpers.

## Tasks

- [x] Add reserved typed `DataFlow<'_>` / `dataflow` capability wiring.
- [x] Skip rules whose requested capabilities are unsupported or setup-missing.
- [x] Update roadmap/research/docs/AGENTS to describe future CFG/call graph/dataflow/coverage/module graph APIs as typed views.
- [x] Add focused tests for unsupported/setup-missing capability diagnostics, unsupported rule skipping, and dataflow planning.
- [x] Run focused, workspace, release, installed CLI, and example verification.
