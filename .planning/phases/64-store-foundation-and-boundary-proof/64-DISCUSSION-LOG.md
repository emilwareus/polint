# Phase 64: Store Foundation and Boundary Proof - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-10
**Phase:** 64-store-foundation-and-boundary-proof
**Mode:** `--auto --chain`
**Areas discussed:** Activation and store ownership, Schema and recovery posture, Writer contention, Kernel integration, Verification and scope boundary

---

## Activation and Store Ownership

| Option | Description | Selected |
|--------|-------------|----------|
| Private disabled-by-default foundation | Keep activation crate-private, use internal tests/harnesses, and add no public Phase 64 switch. Store data stays under the existing cache root. | ✓ |
| Enable by default immediately | Open/create the store for every normal command before it provides reusable data. | |
| Add a public experimental flag | Introduce a CLI/config compatibility commitment during the private foundation phase. | |

**Selection:** Auto-selected the recommended private disabled-by-default foundation.
**Notes:** `--no-cache` remains the future hard-disable signal. `CacheLayout` and `POLINT_CACHE_DIR` own placement and path safety.

---

## Schema and Recovery Posture

| Option | Description | Selected |
|--------|-------------|----------|
| Typed skip/rebuild outcome | Migrate supported schemas transactionally, refuse future schemas without mutation, and represent corruption/invalidity as private controlled outcomes. | ✓ |
| Always delete and recreate | Replace any unopenable database automatically, including potentially newer schemas. | |
| Fail the entire check command | Treat private persistence failure as policy-analysis failure. | |

**Selection:** Auto-selected typed, non-destructive recovery outcomes.
**Notes:** Destructive replacement is permitted only through an explicit safe helper operating on a verified polint-owned cache path.

---

## Writer Contention

| Option | Description | Selected |
|--------|-------------|----------|
| Bounded lease then skip persistence | Use WAL, `BEGIN IMMEDIATE`, and a short busy timeout; the losing process skips persistence without changing policy output. | ✓ |
| Wait indefinitely | Block policy execution until the writer becomes available. | |
| Allow concurrent writers | Let multiple processes interleave store work without a single-writer boundary. | |

**Selection:** Auto-selected bounded writer leasing with skip-on-contention.
**Notes:** Query/read paths use independently opened read-only connections.

---

## Kernel Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Post-validation typed facade | Open/validate the store only through a crate-private facade after kernel facts are validated; expose status as private telemetry. | ✓ |
| Provider SQL access | Pass connections or SQL strings into provider implementations. | |
| Rule SQL access | Make store internals available to SDK rules or `RuleCtx`. | |

**Selection:** Auto-selected post-validation typed integration.
**Notes:** Enabled, disabled, busy, future-schema, invalid-schema, and corrupt-store modes must preserve normalized check diagnostics and exit behavior.

---

## Verification and Scope Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Full foundation proof | Migration, pragma, read-only, contention, disabled-I/O, parity, leak, and Phase 63 regression-budget coverage. Keep schema minimal. | ✓ |
| Unit tests only | Test migration helpers without outside-user parity or boundary proof. | |
| Pull later store features forward | Add manifests, generations, fact ingest, or query behavior during Phase 64. | |

**Selection:** Auto-selected the complete foundation proof with strict later-phase boundaries.
**Notes:** Phase 65 owns generation manifests; Phase 66 facts/indexes; Phase 67 summaries/frontier; Phase 68 queries; later phases own CLI/search/recovery scale work.

---

## the agent's Discretion

- Exact private Rust type/module names.
- Exact bounded busy timeout/retry values.
- Minimal v1 schema representation and migration storage.
- Private telemetry representation and explicit safe-rebuild helper shape.

## Deferred Ideas

None. Later capabilities remain in their roadmap phases.
