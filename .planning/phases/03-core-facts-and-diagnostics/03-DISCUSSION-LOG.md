# Phase 3: Core Facts and Diagnostics - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-04-28T11:16:51Z
**Phase:** 03-core-facts-and-diagnostics
**Mode:** `--auto`
**Areas discussed:** Fact model boundaries, Span and source locations, Rule runner behavior, Diagnostic contract, File discovery determinism, Testing focus

---

## Fact Model Boundaries

| Option | Description | Selected |
|--------|-------------|----------|
| Harden current `AnalysisDb` | Keep the existing Vec-backed fact store and typed IDs; add missing facts/tests without a query-engine rewrite. | yes |
| Introduce query framework now | Add a richer query layer such as Salsa-style infrastructure in Phase 3. | |
| Minimize to only current rule needs | Keep only facts consumed by current built-in rules and defer broader model stability. | |

**Auto choice:** Harden current `AnalysisDb`.
**Notes:** Recommended because the project decision already says not to start with Salsa, and later Go/TS/cache phases need a stable shared contract before deeper semantics.

---

## Span and Source Locations

| Option | Description | Selected |
|--------|-------------|----------|
| Byte offsets plus one-based line/column | Keep parser-friendly byte ranges and human-friendly diagnostic ranges, then harden conversion tests. | yes |
| Line/column only | Simpler output model, but loses parser-native precision and makes future source edits harder. | |
| Byte offsets only | Parser-friendly, but less ergonomic for diagnostics and snapshots. | |

**Auto choice:** Byte offsets plus one-based line/column.
**Notes:** Recommended because the current model already uses this shape and Phase 3 can prove UTF-8/newline behavior with unit and property tests.

---

## Rule Runner Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Harden current registry/runner | Preserve `Rule`, `RuleMeta`, `Capabilities`, `RuleCtx`, `RuleOptions`, and `run_rules`; add tests for filtering, panic/error containment, capability declarations, dedupe, and deterministic parallel output. | yes |
| Rewrite around execution boundaries now | Keep the fact model independent of how rules are loaded; avoid coupling Phase 3 to rule loading mechanics. | |
| Delay runner hardening to SDK phase | Leave core runner behavior mostly untested until Phase 6. | |

**Auto choice:** Harden current registry/runner.
**Notes:** Recommended because CORE-02 is explicitly in Phase 3, while SDK breadth and dynamic loading have later phases.

---

## Diagnostic Contract

| Option | Description | Selected |
|--------|-------------|----------|
| Harden human and JSON diagnostics now | Support labels, evidence, fixes, suggestions, help, fingerprints, sorting, dedupe, and snapshots for human/JSON. | yes |
| Finish SARIF in Phase 3 | Pull production CI/SARIF completeness forward from Phase 8. | |
| Keep diagnostics minimal | Only assert JSON parseability from Phase 2. | |

**Auto choice:** Harden human and JSON diagnostics now.
**Notes:** Recommended because DIAG-01 and TEST-03 are Phase 3 scope, while DIAG-03 is mapped to Phase 8.

---

## File Discovery Determinism

| Option | Description | Selected |
|--------|-------------|----------|
| Prove deterministic output | Add tests/invariants around discovery ordering, include/exclude effects, and supported file filtering without expanding discovery semantics. | yes |
| Redesign discovery | Replace `ignore`/`globset` or broaden traversal behavior. | |
| Defer all discovery testing | Treat Phase 2 coverage as sufficient. | |

**Auto choice:** Prove deterministic output.
**Notes:** Recommended because FS-02 is a Phase 3 requirement and Phase 2 already established the discovery implementation.

---

## Testing Focus

| Option | Description | Selected |
|--------|-------------|----------|
| Focused unit, snapshot, and property tests | Add targeted tests for shared core invariants, human/JSON snapshots, span properties, diagnostic ordering, and discovery determinism. | yes |
| Broad end-to-end fixture expansion | Cover all clean/failing Go and TS behavior now. | |
| Only unit tests | Skip snapshots/properties until final hardening. | |

**Auto choice:** Focused unit, snapshot, and property tests.
**Notes:** Recommended because TEST-01, TEST-03, and TEST-04 are in Phase 3, while broader Go/TS and CI coverage remains mapped to later phases.

---

## the agent's Discretion

- Exact plan slicing is left to the planner.
- Minor helper APIs are allowed when they make the Phase 3 contract easier to test and use.
- Source-compatible public struct adjustments are allowed if they are needed to satisfy the Phase 3 contract.

## Deferred Ideas

- Full Go semantic/type information.
- Exact dynamic branch coverage.
- Production SARIF-like CI completeness.
- Cache persistence and profiling.
- Broad Go/TS clean/failing fixture coverage.
