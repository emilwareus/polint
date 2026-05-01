---
phase: 09-plugin-skeleton
plan: "01"
subsystem: plugin
tags: [wasm, wit, diagnostics, stable-ids]

requires:
  - phase: 08-sdk-and-rule-scaffolding
    provides: "SDK conventions and rule-facing diagnostic contracts"
provides:
  - "Experimental WIT rule boundary with typed metadata and diagnostics"
  - "Stable-ID host fact query contract for future Wasm plugins"
  - "Unit tests pinning WIT anchors and rejecting full AST/source payload names"
affects: [plugin-host, rule-sdk, diagnostics]

tech-stack:
  added: []
  patterns:
    - "Expose the plugin WIT contract through include_str! and pin important anchors with unit tests"
    - "Use stable IDs in the host API instead of transferring full AST, source, or graph payloads"

key-files:
  created: []
  modified:
    - crates/polint-plugin/src/rule.wit
    - crates/polint-plugin/src/lib.rs

key-decisions:
  - "The plugin WIT boundary exposes typed metadata, capabilities, run, typed diagnostics, and narrow host fact queries."
  - "The host API stays stable-ID based and deliberately omits full AST/source payload transfer."

patterns-established:
  - "Plugin contract tests assert semantically important WIT anchors rather than treating the WIT file as incidental text."
  - "Future plugin host work should add behavior behind the experimental boundary without wiring plugin execution into polint check."

requirements-completed: [PLUG-01]

duration: 5 min
completed: 2026-05-01
---

# Phase 09 Plan 01: WIT Contract Summary

**Experimental plugin WIT contract now has typed metadata, diagnostics, capabilities, and stable-ID host fact queries.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-01T13:05:00Z
- **Completed:** 2026-05-01T13:10:42Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added typed WIT records for rule metadata, diagnostics, text ranges, and severity.
- Kept plugin host queries narrow through `file-id`, `function-id`, and `branch-id` stable IDs.
- Added unit tests for required WIT anchors and a negative test against `ast-json`, `source-text`, and `syntax-tree` payload names.

## Task Commits

1. **Tasks 1-3: WIT contract tests and typed WIT interface** - `adb9f35` (feat)

**Plan metadata:** this summary commit

## Files Created/Modified

- `crates/polint-plugin/src/rule.wit` - Defines the experimental WIT rule interface.
- `crates/polint-plugin/src/lib.rs` - Exposes the WIT contract and tests required anchors.

## Decisions Made

The plugin boundary uses stable IDs for host facts and typed diagnostics for plugin output. This preserves the sandbox direction from the phase context and avoids committing future plugin execution to full AST/source transfer.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `09-02`: manifest loading can now validate manifests against a pinned experimental WIT boundary.

---
*Phase: 09-plugin-skeleton*
*Completed: 2026-05-01*
