---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 03
subsystem: analysis-entrypoints
tags: [go-recognizers, net-http, chi, cobra, testing, framework-detection]
dependency_graph:
  requires: [entrypoint-facts, entrypoint-store, entrypoints-provider-kernel-wiring]
  provides: [go-framework-recognizers, go-entrypoint-facts, go-unresolved-framework-facts]
  affects: [entrypoints-provider, ts-js-recognizers, trust-boundary-extraction, eval-fixtures]
tech_stack:
  added: []
  patterns: [import-scan-framework-detection, call-site-pattern-matching, naming-convention-entrypoints, unresolved-framework-emission]
key_files:
  created:
    - crates/polint/src/analysis/entrypoints/recognizers_go.rs
  modified:
    - crates/polint/src/analysis/entrypoints/mod.rs
decisions:
  - Use caller function as fallback handler target when deeper handler resolution cannot resolve the specific handler function
  - Testing entrypoints use ResolvedStatic precision because naming convention is deterministic
  - Cobra entrypoints use Conservative precision per D-09 due to heuristic evidence quality
  - Unrecognized Go framework imports use UnsupportedFrameworkVersion reason with evidence containing the import path
  - Chi imports without matching registration patterns emit UnrecognizedPattern unresolved facts
patterns-established:
  - "Go framework recognizer pattern: scan imports for framework detection, then match call site shapes against known registration patterns"
  - "Naming-convention entrypoint pattern: scan function names against known test prefixes without call-site analysis"
  - "Unresolved framework detection: emit explicit unknown facts for framework imports without matching patterns"
requirements-completed: []
metrics:
  duration: 5 min
  completed: 2026-05-24
---

# Phase 35 Plan 03: Go Framework Recognizers Summary

Go net/http, chi, testing, and cobra recognizers producing EntrypointFact and UnresolvedFrameworkFact rows from import table scanning, call-site pattern matching, and function naming conventions.

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-24T05:35:13Z
- **Completed:** 2026-05-24T05:40:13Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Go net/http HandleFunc and Handle registrations produce EntrypointFact with HttpRoute kind
- Go chi router method registrations (Get, Post, Put, Delete, Patch, Options, Head, Connect, Trace) produce EntrypointFact with HttpRoute kind including method metadata
- Go chi r.Use produces HttpMiddleware entrypoints, r.Route produces HttpRoute with prefix evidence
- Go Test*/Benchmark*/Example*/Fuzz* functions in _test.go files produce EntrypointFact with Test kind and ResolvedStatic precision
- Go cobra.Command AddCommand patterns produce EntrypointFact with CliCommand kind and Conservative precision
- Unrecognized Go framework imports (gin, echo, fiber, gorilla, etc.) produce UnresolvedFrameworkFact per D-10
- Chi imports without matching registration patterns produce UnrecognizedPattern facts

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement Go HTTP framework recognizers (net/http and chi)** - `5d8ed28` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/entrypoints/recognizers_go.rs` - Go framework recognizer with recognize_go_entrypoints producing GoRecognizerOutput (entrypoints + unresolved facts)
- `crates/polint/src/analysis/entrypoints/mod.rs` - Added pub(crate) mod recognizers_go

## Decisions Made

- Use caller function as fallback handler target: when the handler function cannot be resolved from call-site arguments, the caller function is used as the target since in Go patterns the registration and handler are typically in the same scope
- Testing entrypoints bypass call-site scanning entirely and use function naming convention only, matching the Go testing toolchain behavior
- Unrecognized framework imports use a broad marker set (router, http, server, handler, mux, gin, echo, fiber, gorilla) to catch common Go web frameworks not yet covered by native recognizers
- Chi imports detected in files without matching call-site patterns emit UnrecognizedPattern rather than being silently skipped

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Go recognizer module is ready for integration into the entrypoints provider (Plan 35-05 or similar)
- TS/JS recognizers (Plan 35-04) can follow the same pattern established here
- Trust boundary extraction can build on the entrypoint facts produced by Go recognizers

## Self-Check: PASSED

---
*Phase: 35-framework-entrypoints-and-trust-boundaries*
*Completed: 2026-05-24*
