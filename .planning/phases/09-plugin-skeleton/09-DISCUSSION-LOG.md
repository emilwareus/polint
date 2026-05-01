# Phase 9: Plugin Skeleton - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-01T11:55:00Z
**Phase:** 09-plugin-skeleton
**Areas discussed:** Experimental Scope, WIT and Host Query Surface, Loader and Manifest Validation, Documentation and Truthfulness, Test Proof

---

## Experimental Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Experimental skeleton | Build a clean boundary without runtime execution or auto-compilation. | ✓ |
| Production plugin runtime | Compile, load, schedule, and execute repo-local Wasm rules now. | |
| Documentation only | Leave code mostly untouched and document future work. | |

**User's choice:** `[auto]` Experimental skeleton.
**Notes:** This matches the roadmap and prior decisions not to claim dynamic repo-local Rust rule loading in v1.

---

## WIT and Host Query Surface

| Option | Description | Selected |
|--------|-------------|----------|
| Stable-ID host API | Metadata, capabilities, diagnostics, and narrow host fact queries by stable IDs. | ✓ |
| AST payload API | Pass full AST/source JSON payloads to plugins. | |
| Broad speculative API | Add many future host queries before the runtime exists. | |

**User's choice:** `[auto]` Stable-ID host API.
**Notes:** This carries forward the prompt constraint that host owns ASTs/facts and plugins query by stable IDs.

---

## Loader and Manifest Validation

| Option | Description | Selected |
|--------|-------------|----------|
| Validate-only host skeleton | Parse manifests, validate component paths, and optionally validate component bytes behind a feature. | ✓ |
| Full instantiate-and-run host | Instantiate components and invoke plugin rules in the analysis pipeline. | |
| No host code | Keep only WIT files. | |

**User's choice:** `[auto]` Validate-only host skeleton.
**Notes:** The existing `PluginHost` already points in this direction and should be hardened rather than replaced.

---

## Documentation and Truthfulness

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit experimental docs | Mark Wasm repo-local rules experimental and document the stable-ID host API direction. | ✓ |
| Product-ready docs | Present plugins as ready for normal `polint check` use. | |
| Minimal TODO docs | Leave only TODO comments. | |

**User's choice:** `[auto]` Explicit experimental docs.
**Notes:** Previous phases repeatedly chose truthful boundaries over overclaiming.

---

## Test Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Focused deterministic tests | Cover WIT contents, manifest validation, missing paths, gating, and optional feature validation where practical. | ✓ |
| End-to-end plugin execution tests | Build and execute a real Wasm plugin in the rule pipeline. | |
| Manual verification only | Rely on full workspace checks without plugin-specific assertions. | |

**User's choice:** `[auto]` Focused deterministic tests.
**Notes:** This gives Phase 9 meaningful proof without scope creep into the future runtime.

---

## the agent's Discretion

- Exact WIT names/package structure.
- Whether a tiny Wasm example is practical or should be deferred.
- Final doc location and plan split.

## Deferred Ideas

- Automatic repo-local Rust rule compilation to Wasm.
- Running Wasm plugins in `polint check`.
- Broad host query APIs or AST payload transfer.
