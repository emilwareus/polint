# Phase 20: Private Analysis Kernel Facade - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 20-private-analysis-kernel-facade
**Areas discussed:** Kernel boundary, Provider manifests, Inspection/debug path, Compatibility and tests
**Mode:** `--auto`

---

## Kernel Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Private facade, runner keeps rule execution | Move provider orchestration behind a crate-private kernel; runner still handles CLI/reporting/rules. | ✓ |
| Full internal engine rewrite | Move planning, providers, rules, reporting, and cache into a new engine in one phase. | |
| Public SDK-facing kernel | Expose kernel concepts to rule authors immediately. | |

**User's choice:** Auto-selected recommended default: private facade, runner keeps rule execution.
**Notes:** This preserves public API discipline and keeps Phase 20 behavior-preserving.

---

## Provider Manifests

| Option | Description | Selected |
|--------|-------------|----------|
| Metadata-first manifests | Add deterministic internal descriptors for existing providers without driving scheduling yet. | ✓ |
| Scheduler-backed manifests now | Make manifests immediately drive dependency closure and topological scheduling. | |
| Documentation-only provider list | Record providers in docs/tests without internal manifest types. | |

**User's choice:** Auto-selected recommended default: metadata-first manifests.
**Notes:** Phase 20 establishes ownership; later phases own scheduler, validation, cache keys, and layer persistence.

---

## Inspection And Debug Path

| Option | Description | Selected |
|--------|-------------|----------|
| Internal/test-facing snapshot helper | Make provider order inspectable through crate-private deterministic reports used by tests. | ✓ |
| Hidden CLI command | Add hidden debug CLI output for provider order. | |
| Stable public CLI command | Add a user-facing provider inspection command. | |

**User's choice:** Auto-selected recommended default: internal/test-facing snapshot helper.
**Notes:** This satisfies inspectability without widening public CLI surface.

---

## Compatibility And Tests

| Option | Description | Selected |
|--------|-------------|----------|
| Behavior-preservation tests plus manifest unit tests | Prove existing `polint check` behavior remains unchanged and manifests/order are deterministic. | ✓ |
| Broad new integration suite | Add many new public CLI tests around provider inspection. | |
| Minimal compile-only proof | Rely on existing tests without focused manifest/order coverage. | |

**User's choice:** Auto-selected recommended default: behavior-preservation tests plus manifest unit tests.
**Notes:** Existing behavior is the main acceptance gate; new tests should be focused and not imply public provider APIs.

---

## the agent's Discretion

- Exact internal module names and type names.
- Whether source loading is modeled as the first provider inside the facade or as a helper called by the facade.
- Exact test helper shape for provider-order inspection.

## Deferred Ideas

- Full demand scheduler.
- Fact metadata, validation, and merge gates.
- Layer cache keys and persistent layer cache.
- Public query or provider inspection APIs.
