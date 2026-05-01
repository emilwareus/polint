# Phase 7: Cache and Performance - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-01
**Phase:** 7-Cache and Performance
**Areas discussed:** Cache key and payload boundary, Disabled cache semantics, Deterministic parallelism, Per-rule profiling output, Validation and performance proof

---

## Cache key and payload boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Content-addressed parse/fact metadata only | Include file hash, config hash, rule hash, cache/schema version, and relevant language/parser/fact schema inputs; cache only metadata that can be validated safely. | yes |
| Broad full-analysis cache | Cache larger analysis outputs, potentially including full AST-like or rule-result data. | |
| Leave cache skeleton only | Keep the current cache crate as a placeholder and avoid integration work. | |

**User's choice:** `[auto]` Content-addressed parse/fact metadata only.
**Notes:** Recommended because it satisfies `PERF-01` while keeping v1 cache correctness and invalidation simple.

---

## Disabled cache semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Bypass both reads and writes | `--no-cache` guarantees no cache reads and no cache writes. | yes |
| Bypass reads only | Avoid stale hits but still populate cache for later runs. | |
| Best-effort disable | Disable cache where convenient without a hard observable contract. | |

**User's choice:** `[auto]` Bypass both reads and writes.
**Notes:** Recommended because the roadmap explicitly says `--no-cache` fully bypasses cache reads/writes.

---

## Deterministic parallelism

| Option | Description | Selected |
|--------|-------------|----------|
| Safe parallelism with deterministic reduction | Use Rayon for per-file parsing/fact extraction and rule execution only where final ordering and IDs remain deterministic. | yes |
| Parallelize aggressively | Maximize concurrency even if ordering needs later repair or carries more risk. | |
| Keep all execution sequential | Preserve determinism by avoiding Phase 7 parallelism. | |

**User's choice:** `[auto]` Safe parallelism with deterministic reduction.
**Notes:** Recommended because it satisfies `PERF-02` and carries forward Phase 3 deterministic output guarantees.

---

## Per-rule profiling output

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic rule rows with variable durations | Keep rule order and row shape stable, while treating duration values as variable. | yes |
| Human-only approximate output | Keep current rough output without making it easy to test. | |
| Full benchmark suite | Add benchmark-style performance measurement and speedup assertions. | |

**User's choice:** `[auto]` Deterministic rule rows with variable durations.
**Notes:** Recommended because `PERF-03` requires per-rule timing, but exact timings should not be snapshot-tested.

---

## Validation and performance proof

| Option | Description | Selected |
|--------|-------------|----------|
| Focused cache on/off, deterministic repeated-run, and timing-structure tests | Prove cache disabling, cache behavior, repeated-run determinism, and profile output shape. | yes |
| Microbenchmarks with speedup claims | Add benchmark fixtures and claim specific runtime improvements. | |
| Manual verification only | Rely on manual command runs for cache/performance behavior. | |

**User's choice:** `[auto]` Focused cache on/off, deterministic repeated-run, and timing-structure tests.
**Notes:** Recommended because Phase 7 success criteria emphasize behavior and determinism rather than benchmark numbers.

---

## the agent's Discretion

- Exact cache file layout, serde structs, parser/fact metadata shape, and atomic write helper names.
- Exact `profile-rules` row formatting, provided the output is deterministic enough to test.
- Exact plan split across cache foundation, parser/fact cache integration, deterministic parallelism, profiling, and tests.

## Deferred Ideas

None.
