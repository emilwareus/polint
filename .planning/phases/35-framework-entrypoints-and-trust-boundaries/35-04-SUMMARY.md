---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 04
subsystem: analysis-entrypoints
tags: [ts-js-recognizers, express, mcp-sdk, jest, vitest, mocha, commander, yargs, framework-detection]
dependency_graph:
  requires: [entrypoint-facts, entrypoint-store, entrypoints-provider-kernel-wiring]
  provides: [ts-js-framework-recognizers, ts-js-entrypoint-facts, ts-js-unresolved-framework-facts]
  affects: [entrypoints-provider, trust-boundary-extraction, eval-fixtures]
tech_stack:
  added: []
  patterns: [import-scan-framework-detection, call-site-pattern-matching, test-call-entrypoints, unresolved-framework-emission]
key_files:
  created:
    - crates/polint/src/analysis/entrypoints/recognizers_ts.rs
  modified:
    - crates/polint/src/analysis/entrypoints/mod.rs
decisions:
  - Use caller function as fallback handler target when deeper handler resolution cannot resolve the specific handler function (same pattern as Go recognizers)
  - Test entrypoints use SetupAware precision because they depend on test runner being configured (per D-08)
  - CLI entrypoints (commander, yargs) use Conservative precision per D-09 due to heuristic evidence quality
  - Unrecognized TS/JS framework imports use UnsupportedFrameworkVersion reason with evidence containing the import path
  - Express imports without matching registration patterns emit UnrecognizedPattern unresolved facts
  - MCP SDK import detection uses prefix matching on @modelcontextprotocol/ to cover all SDK subpaths
metrics:
  duration: 4 min
  completed: 2026-05-24
---

# Phase 35 Plan 04: TS/JS Framework Recognizers Summary

TS/JS Express, MCP TypeScript SDK, test framework, and CLI framework recognizers producing EntrypointFact and UnresolvedFrameworkFact rows from import table scanning and call-site pattern matching.

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-24T05:44:28Z
- **Completed:** 2026-05-24T05:48:28Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Express app.get/post/put/delete/patch/options/head calls produce EntrypointFact with HttpRoute kind including method metadata
- Express app.use calls produce HttpMiddleware entrypoints
- Express app.route calls produce HttpRoute entrypoints with route path metadata
- MCP TypeScript SDK server.tool calls produce EntrypointFact with McpTool kind
- MCP TypeScript SDK server.resource calls produce EntrypointFact with McpResource kind
- MCP TypeScript SDK server.prompt calls produce EntrypointFact with McpPrompt kind
- jest/vitest/mocha describe/it/test calls produce EntrypointFact with Test kind and SetupAware precision
- @jest/globals import also detected as jest framework
- commander program.command/action patterns produce EntrypointFact with CliCommand kind and Conservative precision
- yargs yargs.command patterns produce EntrypointFact with CliCommand kind and Conservative precision
- Unrecognized TS/JS framework imports (fastify, koa, hapi, nest, @nestjs, next, nuxt, remix, sveltekit, astro) produce UnresolvedFrameworkFact per D-10
- Express and MCP SDK imports without matching registration patterns produce UnrecognizedPattern facts

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement TS/JS framework recognizers (Express, MCP SDK, test, CLI)** - `dc01b33` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/entrypoints/recognizers_ts.rs` - TS/JS framework recognizer with recognize_ts_entrypoints producing TsRecognizerOutput (entrypoints + unresolved facts), 16 unit tests
- `crates/polint/src/analysis/entrypoints/mod.rs` - Added pub(crate) mod recognizers_ts

## Decisions Made

- Use caller function as fallback handler target: consistent with Go recognizers pattern where the handler is typically in the same scope as the registration
- Test entrypoints use SetupAware precision (not ResolvedStatic like Go) because they depend on test runner configuration being present
- CLI framework patterns use Conservative precision per D-09 reflecting heuristic evidence quality
- MCP SDK detection uses @modelcontextprotocol/ prefix matching to cover all possible subpath imports

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- TS/JS recognizer module is ready for integration into the entrypoints provider (Plan 35-05 or similar)
- Trust boundary extraction can build on the entrypoint facts produced by TS/JS recognizers
- Extension overlay integration can test unknown reduction for TS/JS frameworks

## Self-Check: PASSED
