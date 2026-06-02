# Phase 46: Go Semantic Frontend & Sidecar - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-01
**Phase:** 46-Go Semantic Frontend & Sidecar
**Areas discussed:** Sidecar identity and packaging, Protocol and process boundary, Go fact schema and stable identity, Semantic graph lowering, Lifecycle/cache/taxonomy, Verification

---

## Sidecar Identity And Packaging

| Option | Description | Selected |
|--------|-------------|----------|
| Distinct `polint-go-frontend` sidecar | Add a new semantic sidecar and preserve the current `polint-go-symbols` contract. | ✓ |
| Merge semantic facts into `polint-go-symbols` | One binary/schema handles both symbol facts and semantic graph facts. | |
| Replace the current symbol sidecar | Move current symbols/references onto the new semantic sidecar immediately. | |

**User's choice:** Auto-selected the distinct sidecar option.
**Notes:** This preserves current public symbol/reference behavior while allowing the semantic protocol to evolve privately. Existing materialization and installed-binary patterns should be reused.

---

## Protocol And Process Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Versioned NDJSON with explicit terminator | Stream typed rows, validate schema, and fail closed on missing terminator or malformed rows. | ✓ |
| Single JSON document | Simpler to emit but harder to distinguish partial output and large-repo streaming concerns. | |
| Ad hoc stderr/stdout parsing | Fast to prototype but too brittle for a process boundary. | |

**User's choice:** Auto-selected versioned NDJSON with explicit terminator.
**Notes:** One long-lived sidecar per `polint check` is the default. Timeout, cancellation, kill/wait, and orphan cleanup are mandatory acceptance behavior.

---

## Go Fact Schema And Stable Identity

| Option | Description | Selected |
|--------|-------------|----------|
| Official Go identities first | Use `go/packages`, `go/types`, object paths, package IDs, module paths, receiver types, and SSA identities where available. | ✓ |
| Source-text-derived identities | Derive names from AST text and spans when convenient. | |
| Benchmark-shaped identities only | Emit just enough names to satisfy current benchmark oracles. | |

**User's choice:** Auto-selected official Go identities first.
**Notes:** This phase completes the Phase 42 full-import-path RelString deferral and keeps unsupported/synthetic cases explicit instead of fabricating names.

---

## Semantic Graph Lowering

| Option | Description | Selected |
|--------|-------------|----------|
| Lower proven facts into existing graph constraints | Use Phase 44 `NodeKind`/`ConstraintKind`, emitting direct/static obligations and preserving unresolved dynamic cases. | ✓ |
| Emit solver-derived call edges now | Try to produce final call graph edges directly from the frontend. | |
| Store raw sidecar DTOs only | Defer all graph integration to later phases. | |

**User's choice:** Auto-selected lowering proven facts into existing graph constraints.
**Notes:** Interface dispatch and RTA expansion remain Phase 48. Phase 46 should emit constraints/candidates that downstream solver phases can consume honestly.

---

## Lifecycle, Cache, And Taxonomy

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse Go lifecycle and preserve distinct categories | Use existing `.polint.toml` lifecycle inputs, digest sidecar/toolchain versions, and preserve `GoPackagesLoadFailed`, `GoVersionUnsupported`, `GoSidecarTimeout`. | ✓ |
| Add sidecar-specific config files/flags | Give the sidecar its own lifecycle knobs outside existing Go config. | |
| Collapse failures into setup missing | Simpler output but loses the categories required by GO-04. | |

**User's choice:** Auto-selected existing lifecycle plus distinct categories.
**Notes:** Row-level package errors are preferred when the rest of the run remains trustworthy; whole-run failure is reserved for process/protocol/toolchain failures.

---

## Verification

| Option | Description | Selected |
|--------|-------------|----------|
| Fixture-heavy acceptance | Cover lifecycle variants, graph snapshots, protocol failures, orphan cleanup, cache invalidation, determinism, and leak gate. | ✓ |
| Unit tests only | Test DTO parsing and a few lowering helpers. | |
| Benchmark-only acceptance | Let Go RTA benchmark improvements prove the phase. | |

**User's choice:** Auto-selected fixture-heavy acceptance.
**Notes:** Benchmarks matter, but Phase 46 must first prove the sidecar protocol and graph facts are deterministic, private, and failure-safe.

---

## Agent's Discretion

- Exact DTO names and module layout.
- Whether sidecar helper code is shared with `polint-go-symbols` or duplicated for clarity.
- Exact provider placement after checking the current dependency DAG.
- Exact test module distribution.

## Deferred Ideas

- Go RTA solver and dynamic dispatch are Phase 48.
- Unified solver core is Phase 47.
- Public unknowns CLI is Phase 52.
- Public SDK views are out of v1.3.
