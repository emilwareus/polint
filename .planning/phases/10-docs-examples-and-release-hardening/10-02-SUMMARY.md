---
phase: 10-docs-examples-and-release-hardening
plan: "02"
subsystem: docs
tags: [examples, go, typescript, rule-authoring]

requires:
  - phase: 10-docs-examples-and-release-hardening
    provides: "Complete v1 README guide and examples inventory"
provides:
  - "Copyable example documentation for basic, Go custom rule, and TS custom rule flows"
  - "Runnable Go branch-obligation example config"
  - "Runnable TS design-token example config"
affects: [examples, user-docs, rule-authoring]

tech-stack:
  added: []
  patterns:
    - "Runnable example directories include minimal local .polint.toml files"
    - "Example docs state v1 dynamic-loading limitations explicitly"

key-files:
  created:
    - examples/go-branch-obligations/README.md
    - examples/go-branch-obligations/.polint.toml
    - examples/ts-design-tokens/README.md
    - examples/ts-design-tokens/.polint.toml
  modified:
    - examples/basic/README.md
    - examples/custom-rule-go/README.md
    - examples/custom-rule-ts/README.md

key-decisions:
  - "Top-level examples are command-oriented and copyable rather than tutorial-length."
  - "Go branch obligations are documented as heuristic branch-test evidence checks."
  - "TS design-token checks are documented as syntax-level raw color detection."

patterns-established:
  - "Each runnable example owns a minimal .polint.toml with explicit include and profile rules."
  - "Custom-rule examples explain SDK helpers and the v1 scaffold-only execution limitation."

requirements-completed: [FND-03, TEST-02]

duration: 3 min
completed: 2026-05-01
---

# Phase 10 Plan 02: Examples Summary

**Top-level examples now document the quickstart, custom rule authoring helpers, and runnable Go/TS built-in rule fixtures with honest v1 limitations.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-01T16:08:49Z
- **Completed:** 2026-05-01T16:11:31Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Expanded `examples/basic` with init/check/SARIF commands and Cargo development equivalents.
- Expanded custom Go and TS rule examples with SDK helper references, `polint test-rules`, and dynamic-loading limitations.
- Added runnable `.polint.toml` files and READMEs for Go branch obligations and TS design token examples.
- Documented the Go rule as heuristic and the TS rule as syntax-level raw color detection.

## Task Commits

1. **Tasks 1-3: Existing example docs and runnable Go/TS configs** - `9c6ef66` (docs)

**Plan metadata:** this summary commit

## Files Created/Modified

- `examples/basic/README.md` - Basic init/check flow and Cargo development commands.
- `examples/custom-rule-go/README.md` - Go rule scaffold, helper APIs, and rule-test command.
- `examples/custom-rule-ts/README.md` - TS rule scaffold, literal/JSX helper APIs, and rule-test command.
- `examples/go-branch-obligations/README.md` - Heuristic branch-obligation example documentation.
- `examples/go-branch-obligations/.polint.toml` - Minimal Go example config.
- `examples/ts-design-tokens/README.md` - Syntax-level design-token example documentation.
- `examples/ts-design-tokens/.polint.toml` - Minimal TSX example config.

## Decisions Made

Examples stay compact and copyable. The README remains the broader guide; example READMEs focus on running or authoring the specific example without overstating v1 capabilities.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `10-03`: the runnable example directories and mixed-language fixture can now be covered by CLI integration tests.

---
*Phase: 10-docs-examples-and-release-hardening*
*Completed: 2026-05-01*
