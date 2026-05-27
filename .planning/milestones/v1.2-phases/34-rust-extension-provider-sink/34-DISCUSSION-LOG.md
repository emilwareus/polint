# Phase 34: Rust Extension/Provider Sink - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-23
**Phase:** 34-rust-extension-provider-sink
**Areas discussed:** Extension runtime and activation, Typed sinks and merge boundary, Validation/provenance/precision, Cache/quarantine/delta evidence, Public surface and deferrals
**Mode:** `--auto`

---

## Extension Runtime and Activation

| Option | Description | Selected |
|--------|-------------|----------|
| Process-isolated repo-local Rust executable | Extensions live under `.polint/extensions/<name>` and communicate through a versioned protocol. | ✓ |
| In-process Rust dynamic library | Load extension code into the host process for lower overhead. | |
| Declarative model files first | Use TOML/YAML-like data models before executable Rust extensions. | |

**User's choice:** Auto-selected process-isolated repo-local Rust executable.
**Notes:** This matches the extension research recommendation and avoids host crashes or Rust ABI/toolchain coupling.

---

## Typed Sinks and Merge Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Small typed sink vertical slice | Implement the smallest sink that exercises manifest, protocol, validation, merge, provenance, cache, and eval. | ✓ |
| Broad framework-specific sinks now | Add entrypoints, call graph, data flow, effects, and type/value sinks together. | |
| Raw database mutation | Let extensions write directly into `AnalysisDb`. | |

**User's choice:** Auto-selected small typed sink vertical slice.
**Notes:** Framework and precision-specific sinks are deferred to later roadmap phases; raw `AnalysisDb` mutation is rejected by project constraints and research invariants.

---

## Validation, Provenance, and Precision

| Option | Description | Selected |
|--------|-------------|----------|
| Validate before merge with precision ceilings | Reject malformed facts before storage and preserve provenance, confidence/status, and precision labels. | ✓ |
| Accept then diagnose later | Store extension facts first and report validation problems after downstream providers run. | |
| Trust compiled extension output | Treat successful compilation as sufficient trust. | |

**User's choice:** Auto-selected validate before merge with precision ceilings.
**Notes:** The phase requirement explicitly calls for validation, provenance, precision ceilings, and activation status.

---

## Cache, Quarantine, and Delta Evidence

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse Phase 33 quarantine and real extension digests | Feed extension code/manifest/input digests into existing cache keys and quarantine extension-influenced nodes on changes. | ✓ |
| Separate extension cache model | Build a parallel cache/quarantine subsystem for extension output. | |
| No cache participation until later | Run extensions but keep their output outside deterministic cache identity. | |

**User's choice:** Auto-selected reuse Phase 33 quarantine and real extension digests.
**Notes:** This keeps Phase 34 aligned with SAE-INT-04 and the existing `LayerKey`, `SummaryKey`, and `InputSnapshot` extension slots.

---

## Public Surface and Deferrals

| Option | Description | Selected |
|--------|-------------|----------|
| Private or preview-only unless necessary | Keep the host/protocol and debug output internal/test-facing unless a narrow SDK surface is required for fixture crates. | ✓ |
| Promote full public extension SDK now | Treat `polint::extension` and extension CLI commands as stable product contracts in Phase 34. | |
| Hide all extension behavior from tests and docs | Avoid any external fixture crate or visible command until later phases. | |

**User's choice:** Auto-selected private or preview-only unless necessary.
**Notes:** This respects public API discipline while still letting Phase 34 prove a real repo-local provider boundary.

---

## the agent's Discretion

- Planner may choose the exact first sink/fact family as long as it proves the whole boundary and avoids scope creep into Phase 35+ semantics.
- Planner may choose JSON, JSONL, or another serde-friendly protocol format optimized for deterministic tests.
- Planner may choose plan splits across host/protocol, manifest/discovery, sink/validation, cache/quarantine, eval/no-leak, and docs/fixtures.

## Deferred Ideas

- Framework entrypoints and trust boundaries belong to Phase 35.
- Refined call graph, data-flow, slicing/evidence, benchmarks, and public SDK/query ergonomics belong to Phases 37-41.
