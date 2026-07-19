# Phase 65 Review Findings Triage

Date: 2026-07-19

## Purpose

The abandoned implementation produced valuable bug discoveries. This document preserves the findings without treating the abandoned fixes as an indivisible patch.

Disposition labels:

- **Store core:** retain the invariant and regression in the restarted store slice that owns it.
- **Prerequisite PR:** real kernel correctness issue; fix independently before persisting or trusting the affected metadata.
- **Independent hardening:** worthwhile but not part of semantic-store metadata delivery.
- **Design-dependent:** keep the lesson; no fix is needed if the restarted design omits the mechanism.
- **Hygiene:** retain normal tooling coverage only.

## Initial store review

| ID | Finding | Disposition | Restart treatment |
|---|---|---|---|
| WR-01 | Production publication could skip full plan integrity validation. | Store core | Activation must require a sealed typed validation result. |
| WR-02 | Identical-generation reuse trusted headers and partial statistics rather than active rows. | Store core | Reopen and authenticate the complete slice projection before reuse. |
| WR-03 | Current-schema validation accepted weakened constraints or same-name objects. | Store core | Validate the exact schema owned by each small migration; reject unknown current-version drift. |
| PERF-01 | The semantic-store RSS boundary and its measurement were not trustworthy. | Design-dependent | Retain a real paired benchmark, but move it out of required fast CI and keep the fixture small. |

## First deep review

| ID | Finding | Disposition | Restart treatment |
|---|---|---|---|
| WR-05 | Decoded active rows were not bound back to stored semantic identities. | Store core | Recompute identity from typed decoded rows. |
| PERF-02 | Forged compressed stable-key prefixes could trigger repeated huge allocations. | Design-dependent | Do not compress stable keys initially; if compression returns, require preflight and one canonical decode. |
| WR-06 | `StableFactKey` equality and ordering could disagree across encodings. | Design-dependent | Avoid alternate encodings; otherwise enforce trait laws over one canonical representation. |
| WR-07 | Syntax cache dependencies were incomplete and insufficiently authenticated. | Prerequisite PR | Repair one syntax provider’s exact key/edge contract before mirroring it. |
| WR-08 | Failed providers could be persisted as trusted. | Prerequisite PR | Typed provider outcomes must precede trusted store metadata. |
| WR-09 | Production snapshots claimed model and Go-tool inputs that were not actually certified. | Prerequisite PR | Mark uncertified inputs explicitly and omit affected outputs from reuse. |
| WR-10 | Missing syntax providers were recorded as present upstream layers. | Prerequisite PR | Preserve unsupported/setup-missing/absent states exactly. |
| WR-11 | Go semantic identity depended on Rust `Debug` text. | Prerequisite PR | Use explicit versioned canonical fields, never debug formatting. |
| PERF-03 | Semantic graph identity included reachability output it did not consume. | Prerequisite PR | Remove undeclared inputs in a focused cache-key PR. |
| IN-01 | `git diff --check` failed on trailing whitespace. | Hygiene | Keep the standard whitespace gate. |

## Second deep review

| ID | Finding | Disposition | Restart treatment |
|---|---|---|---|
| WR-12 | Writer accepted stable keys that the reader rejected. | Store core | Writer and reader share one codec and validation boundary. |
| PERF-04 | Stable-key bounds ran after SQLite allocation and omitted row overhead. | Store core | Preflight storage class, row count, byte length, and aggregate overhead before allocation. |
| WR-13 | Input child relationships and declared counts were unauthenticated. | Store core | Child ownership and counts participate in canonical identity. |
| WR-14 | Cache-write warnings were classified as provider failures. | Prerequisite PR | Separate run-local telemetry from semantic provider outcome. |
| WR-15 | Non-syntax provider failures collapsed to absence downstream. | Prerequisite PR | Preserve failed versus absent through the provider DAG. |
| WR-16 | Solver consumed reachability roots without authenticating the producer. | Prerequisite PR | Bind consumed upstream status and output identity. |
| PERF-05 | Semantic graph hashed four provider outputs it did not consume. | Prerequisite PR | Fix scoped cache identity independently. |
| WR-17 | Stable input-status codec repair was incomplete. | Store core | One closed, versioned status codec with strict decoding. |
| SEC-01 | Predictable shared-temp frontend cache could execute a preseeded binary. | Independent hardening | Track as Go runtime/cache security work with its own threat model. |
| SEC-02 | Sealed tool identity did not bind the executable actually run. | Independent hardening | Required before durable reuse of Go semantic facts, not before minimal generation storage. |
| WR-18 | Behavior-affecting Go environment was outside identity. | Independent hardening | Define an explicit environment policy before Go-result reuse. |
| WR-19 | Source-mode frontend cache omitted source, toolchain, and target provenance. | Independent hardening | Fix in the Go semantic cache subsystem. |
| PERF-06 | Adaptation-model discovery could grow traversal memory before its limit. | Independent hardening | Add bounded traversal in a dedicated adaptation-model PR. |
| SEC-03 | Model root/symlink validation was check-then-open. | Independent hardening | Fix with an anchored/open-then-verify design in the model-loading subsystem. |

## Third deep review

| ID | Finding | Disposition | Restart treatment |
|---|---|---|---|
| WR-20 | Per-file cache warnings became durable syntax facts. | Prerequisite PR | Keep transient warnings outside semantic payloads and digests. |
| WR-21 | Extensions could succeed using an unauthenticated partial universe. | Prerequisite PR | Declare required dependencies or explicit degraded semantics. |
| WR-22 | Late provider failures did not revoke advertised capabilities. | Prerequisite PR | Compute effective capabilities after provider completion. |
| WR-23 | Identical-generation validation could span two active snapshots. | Store core | Match and validate the active handle in one read transaction. |
| PERF-07 | Non-fact metadata materialized before allocation preflight. | Store core | Apply store-wide preflight to every persisted family. |
| WR-24 | Metadata ordinals were not authenticated. | Store core | Either derive order or bind each ordinal to canonical content; do not trust stored ordering alone. |
| SEC-04 | Go package loading could execute a different Go binary than identity described. | Independent hardening | Block Go-result reuse until executable selection is explicit. |
| PERF-08 | Custom frontend source traversal was unbounded before enforcing limits. | Independent hardening | Bounded iterative traversal in a separate Go-runtime PR. |
| REL-01 | Frontend staging names could collide and leak on failure. | Independent hardening | Use atomic allocation and RAII cleanup in the owning subsystem. |

## Fourth deep review

| ID | Finding | Disposition | Restart treatment |
|---|---|---|---|
| SEC-05 | Go identity omitted the delegated toolchain closure. | Independent hardening | Retain the correctness requirement, but first choose a proportional threat model; conservative non-reuse is acceptable. |
| WR-25 | Ambient Go module/proxy/cache state was unauthenticated. | Independent hardening | Use an explicit allowlisted environment before cross-process Go-result reuse. |
| REL-02 | Go probes/builds/runtime streams and descendants were not fully bounded. | Independent hardening | Keep timeout/stream/process cleanup work separate from SQLite metadata. |
| WR-26 | Global fact-validation failure did not revoke capabilities. | Prerequisite PR | Finalize provider trust and rule dispatch only after authoritative validation. |
| WR-27 | Publication could activate scalar data it had never authenticated. | Store core | Typed candidate validation occurs before complete/active transition. |
| WR-28 | Ordinal validation accepted value-preserving swaps. | Store core | Bind canonical row content to its derived position or omit persisted ordinals. |

## Additional convergence and hosted-CI findings

| Finding | Disposition | Restart treatment |
|---|---|---|
| Cross-file semantic IDs could collide when per-file indexes were merged. | Prerequisite PR | Fix independently; this is a product correctness bug regardless of the store. |
| Legacy symbol-graph cache payloads could preserve collided relationships. | Prerequisite PR | Rotate/reject the affected schema with the semantic-ID fix. |
| Performance children could execute mutable/different test binaries across paired samples. | Design-dependent | Retain immutable-child measurement only in the dedicated benchmark harness. |
| Module, symbol, and topology caches hashed unrelated Go semantic tool identity. | Prerequisite PR | Scope keys and dependency edges to actual provider inputs. |
| Linux owner discovery could fail on unrelated protected `/proc` entries. | Independent hardening | Relevant only if the expanded process-containment design is retained. |
| Windows fixture cache keys missed after semantically unrelated tool preparation changes. | Prerequisite PR | Covered by provider-scoped lifecycle key regressions. |
| Global semantic-test serialization expanded required CI to an hour. | Test architecture | Remove from ordinary correctness tests; isolate benchmark timing instead. |
| Windows required job hit its timeout after tests passed because the benchmark compiled and ran serially afterward. | Test architecture | Separate benchmark job; do not raise the timeout. |

## What should be kept

Keep the findings, invariants, and minimal regression cases. Do not keep the entire fix stack merely because it once addressed them.

Before restarting storage work, prioritize these independent bugs:

1. Cross-file semantic-ID collisions and legacy cache invalidation.
2. Typed provider outcomes and capability revocation after failure/validation.
3. Exact provider input/dependency scoping for caches.

The Go runtime/security findings are credible, but they need a separate threat-model and delivery plan. The minimal semantic-store generation lifecycle must not wait for or absorb them.
