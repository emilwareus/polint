# Phase 23: Input Snapshots and Cache-Key Vocabulary - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-18
**Phase:** 23-input-snapshots-and-cache-key-vocabulary
**Mode:** `--auto`
**Areas discussed:** Vocabulary Boundary, Input Snapshot Coverage, Provider Output Metadata And Stats, Verification Strategy

---

## Vocabulary Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Typed vocabulary and instrumentation only | Add internal snapshot/key types and metadata without changing persistent cache reuse. Recommended because Phase 24 owns persistent layer cache behavior. | yes |
| Start persistent layer reuse now | Fold Phase 24 cache reuse and stale-reuse safeguards into Phase 23. Rejected as scope creep. | |
| Let the agent decide | Leave the boundary to implementation planning. | |

**User's choice:** Auto-selected recommended default: Typed vocabulary and instrumentation only.
**Notes:** Carries forward Phase 20, 21, and 22 decisions that Phase 23 owns cache-key vocabulary while Phase 24 owns persistent layer cache behavior.

---

## Input Snapshot Coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Full SAE-FND-04 input families | Snapshot file text, config, Go lifecycle, TS/JS lifecycle, rule digests, model digests, and official tool invocation digests where present. Recommended because it matches the roadmap success criteria. | yes |
| Source/config only | Start with a smaller snapshot and add lifecycle/tool/model identity later. Rejected because it risks another cache identity rewrite. | |
| Let the agent decide | Leave exact coverage to implementation planning. | |

**User's choice:** Auto-selected recommended default: Full SAE-FND-04 input families.
**Notes:** Lifecycle inputs should include explicit absent/setup-missing placeholders where exact inputs are not available yet.

---

## Provider Output Metadata And Stats

| Option | Description | Selected |
|--------|-------------|----------|
| Internal kernel/provider run metadata | Attach provider output metadata and cache stats to private kernel/provider run reports. Recommended because Phase 23 must not create a public surface. | yes |
| Public report or CLI output | Expose provider metadata in public `polint check` JSON or a stable command. Rejected because public inspect/test surfaces are later phases. | |
| Let the agent decide | Leave metadata attachment entirely to planning. | |

**User's choice:** Auto-selected recommended default: Internal kernel/provider run metadata.
**Notes:** Metadata should consume provider manifests for real snapshot/key/report purposes where practical, replacing synthetic manifest-consumption debt from Phase 20.

---

## Verification Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Crate-private tests plus native eval fixture coverage | Use unit tests for digest/key semantics and the Phase 22 harness for deterministic cache/snapshot invariants. Recommended because it proves behavior while staying internal. | yes |
| Only unit tests | Avoid eval fixture integration. Rejected because the roadmap calls for snapshot coverage across multiple input families and Phase 22 built the harness for this. | |
| Public inspect/debug command | Add a visible command for snapshot inspection. Rejected as premature public CLI expansion. | |

**User's choice:** Auto-selected recommended default: Crate-private tests plus native eval fixture coverage.
**Notes:** Tests must avoid timestamps, absolute paths, raw source text, and nondeterministic ordering in snapshot/key output.

---

## the agent's Discretion

- Exact module placement and type names.
- Exact digest labels and serde/debug helper names.
- How to divide implementation plans between digest/key model, snapshot construction, provider metadata/stats, and eval/test coverage.

## Deferred Ideas

- Persistent layer cache and stale-reuse safeguards - Phase 24.
- Public inspect/test/rule manifest surfaces - Phase 25.
- Demand query, summary, extension, and red-green cache behavior - later v1.2 phases.
