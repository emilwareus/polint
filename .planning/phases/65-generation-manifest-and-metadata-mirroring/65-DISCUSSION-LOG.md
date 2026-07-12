# Phase 65: Generation Manifest and Metadata Mirroring - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-12
**Phase:** 65-generation-manifest-and-metadata-mirroring
**Mode:** `--auto --chain`
**Areas discussed:** Generation lifecycle and reader visibility, Kernel identity mirroring, Invalidation dependency coverage, Validation/recovery/store telemetry

---

## Generation Lifecycle and Reader Visibility

### Atomic commit unit

| Option | Selected |
|--------|----------|
| Full-run generation with provider-generation children | ✓ |
| Activate each provider generation independently | |
| One undifferentiated store snapshot | |

**Auto-selected recommendation:** Start with the full validated run because current validation is global; retain provider-generation children for later finer-grained validation.

### Readability gate

| Option | Selected |
|--------|----------|
| Activate only after all checks and writes complete | ✓ |
| Expose pending rows to readers with status flags | |
| Select the numerically newest generation | |

**Auto-selected recommendation:** Readers require both active selection and complete status.

### Interrupted attempts

| Option | Selected |
|--------|----------|
| Retain isolated pending/failed metadata for recovery | ✓ |
| Delete every failed attempt immediately | |
| Replace active generation before ingest starts | |

**Auto-selected recommendation:** Keep the old complete generation readable and make failed work diagnosable but never queryable.

### Generation identity

| Option | Selected |
|--------|----------|
| Internal relational handle; semantic identity remains digest-based | ✓ |
| Generation ID becomes semantic identity | |
| Timestamp-based generation identity | |

**Auto-selected recommendation:** Never let row IDs, generation counters, timestamps, or insertion order affect semantics.

---

## Kernel Identity Mirroring

### SQLite representation

| Option | Selected |
|--------|----------|
| First-class typed columns with limited deterministic detail payloads | ✓ |
| One JSON blob per Rust object | |
| Store-native replacement keys | |

**Auto-selected recommendation:** Keep identity and invalidation fields directly queryable and losslessly tied to existing Rust types.

### Durable fact identity

| Option | Selected |
|--------|----------|
| `FactFamily` plus stable key; run ID remains transient | ✓ |
| Persist run-local `FactRef` IDs as stable IDs | |
| Assign store-native canonical fact IDs | |

**Auto-selected recommendation:** Preserve the existing explicit separation between run-local handles and stable semantic keys.

### Vocabulary gaps

| Option | Selected |
|--------|----------|
| Extend kernel vocabulary first, then mirror it | ✓ |
| Add store-only identity variants | |
| Encode gaps in free-form metadata | |

**Auto-selected recommendation:** Keep one source of truth for invalidation and identity.

### Phase boundary for `FactMeta`

| Option | Selected |
|--------|----------|
| Persist metadata sidecars only; semantic payloads remain Phase 66 | ✓ |
| Persist all semantic facts now | |
| Define schema but persist no metadata rows | |

**Auto-selected recommendation:** Prove identity/generation joins without pulling broad fact ingest forward.

---

## Invalidation Dependency Coverage

### Edge storage

| Option | Selected |
|--------|----------|
| One canonical edge relation with bidirectional indexes | ✓ |
| Separate writable forward and reverse edge copies | |
| One serialized dependency-index blob | |

**Auto-selected recommendation:** Store each dependency once and derive both traversals deterministically.

### Required input taxonomy

| Option | Selected |
|--------|----------|
| Full META-04 taxonomy with explicit absence/status | ✓ |
| Source and config only | |
| One aggregate workspace digest | |

**Auto-selected recommendation:** Make every later invalidation frontier input explicit now.

### Rule-pack edits

| Option | Selected |
|--------|----------|
| Invalidate analysis only when analysis inputs change | ✓ |
| Invalidate all analysis for any rule edit | |
| Rule changes never affect analysis | |

**Auto-selected recommendation:** Preserve the established boundary between analysis inputs and rule execution results.

### Proof level

| Option | Selected |
|--------|----------|
| Mutation matrix, preserve-hit controls, and ordering determinism | ✓ |
| A few happy-path invalidation tests | |
| Defer all proof to warm-reuse work | |

**Auto-selected recommendation:** Prove both invalidation and non-invalidation for every required class before reuse depends on it.

---

## Validation, Recovery, and Store Telemetry

### Activation validation

| Option | Selected |
|--------|----------|
| Existing validators followed by store-plan validation | ✓ |
| SQLite constraints replace kernel validation | |
| Commit first and validate on read | |

**Auto-selected recommendation:** Preserve current semantic authority and add storage-specific integrity checks before activation.

### Recovery behavior

| Option | Selected |
|--------|----------|
| Fall back to old complete generation; rebuild only on ambiguous integrity | ✓ |
| Automatically delete and rebuild on every failure | |
| Read the newest partial generation | |

**Auto-selected recommendation:** Protect valid old data and require destructive recovery only when selection cannot be trusted.

### Durable statistics

| Option | Selected |
|--------|----------|
| Deterministic counts/digests; timing excluded from identity | ✓ |
| Timestamps and duration participate in identity | |
| Persist no statistics until final hardening | |

**Auto-selected recommendation:** Record what later status/pruning needs without contaminating deterministic semantics.

### Public behavior

| Option | Selected |
|--------|----------|
| Remain private and behavior-neutral | ✓ |
| Add an experimental public store flag | |
| Enable store-backed rule reads immediately | |

**Auto-selected recommendation:** Preserve Phase 64's disabled zero-I/O path and byte-identical public behavior.

---

## the agent's Discretion

- Exact private module/type names and SQL schema decomposition.
- Exact internal representation of pending/failed bookkeeping.
- Exact SQL indexes and deterministic encoding for non-core detail lists.
- Exact injected-failure seam and private telemetry/status variants.

## Deferred Ideas

None. Later fact ingest, summary reuse, query/CLI, search, and pruning work stayed in their mapped phases.
