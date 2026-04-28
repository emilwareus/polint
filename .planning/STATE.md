---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: Executing Phase 03
last_updated: "2026-04-28T11:29:32.379Z"
progress:
  total_phases: 10
  completed_phases: 2
  total_plans: 6
  completed_plans: 3
  percent: 50
---

# State: exlint

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-04-28)

**Core value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

**Current focus:** Phase 03 — core-facts-and-diagnostics

## Current Status

- Repository root: `/Users/emilwareus/Development/exlint`.
- Active branch policy: work directly on `main`; do not use GSD worktrees for this project.
- Planning initialized from `docs/INITIAL_PROMPT.md`.
- Requirements and roadmap created.
- Source implementation committed on `main` as `7828215` (`Implement initial polint workspace`).
- Verification passed: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Phase 1 has been closed through GSD plan execution and verification.
- Phase 2 has been closed through GSD plan execution and verification.
- Next action: discuss/plan Phase 3 core facts, diagnostics, deterministic output, and SDK-facing primitives on `main`.

## Phase Progress

| Phase | Status | Notes |
|-------|--------|-------|
| 1 | Complete | Rust workspace foundation committed and verified |
| 2 | Complete | CLI, config, discovery, and JSON output first loop verified |
| 3 | Ready to plan | Core facts and diagnostics have working initial implementation |
| 4 | In Progress | Go adapter parses with tree-sitter-go and extracts practical syntax facts |
| 5 | In Progress | TypeScript adapter parses with Oxc and extracts practical syntax facts |
| 6 | In Progress | SDK and requested example rules have working initial implementation |
| 7 | In Progress | Cache crate exists; deeper parse/fact persistence remains |
| 8 | In Progress | SARIF-like output, exit codes, profile-rules, explain, and graph commands exist |
| 9 | In Progress | WIT and plugin host skeleton exist |
| 10 | In Progress | README, examples, fixtures, and tests exist; more snapshots/hardening remain |

## Important Context For Execution

- Do not fake functionality. If a feature remains heuristic or experimental, label it that way.
- Keep built-in rules as SDK examples, not a comprehensive ruleset.
- Use deterministic ordering everywhere output can be observed.
- Prefer a smaller complete v1 over broad shallow behavior.
- Keep source and GSD planning changes in `/Users/emilwareus/Development/exlint` on `main`.
- Do not create or use GSD worktrees for this project.
