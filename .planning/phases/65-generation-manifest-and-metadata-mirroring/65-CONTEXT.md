# Phase 65: Generation Manifest and Metadata Mirroring - Context

**Gathered:** 2026-07-28
**Status:** Ready for planning; implementation remains gated by the five-minute required-CI budget

<domain>
## Phase Boundary

This planning pass covers only restart slice **R1: the private minimal generation
lifecycle**. Build on the Phase 64 SQLite boundary so the store can reserve a
pending generation, publish that same generation as complete, atomically select
one complete generation as active, and keep the previous active generation
readable when publication fails.

R1 is one independently reviewable storage invariant. It does **not** implement
or pre-create the original all-at-once Phase 65 manifest and metadata model, and
it does not satisfy or close STORE-04, STORE-05, META-01, or META-04 by itself.
The planner must leave the broader Phase 65 work open for R2-R6 rather than
marking the phase or those requirements complete after R1.

</domain>

<decisions>
## Implementation Decisions

### Delivery Boundary
- **D-01:** Plan and implement R1 only. Do not resume, cherry-pick, or reproduce the abandoned nineteen-plan Phase 65 implementation.
- **D-02:** Treat the restart budgets as hard split/stop gates: at most three implementation tasks, fifteen product/test files, 2,500 handwritten added lines, one durable schema family, and zero provider families in R1.
- **D-03:** If R1 reveals a missing identity, provider, runtime, or validation contract, stop that path and route it to a separate prerequisite. Do not widen R1 to certify it.
- **D-04:** Completing R1 must not mark the original Phase 65 roadmap success criteria or requirement checkboxes complete. R1 is the first restart slice inside that larger roadmap container.

### Generation Lifecycle Semantics
- **D-05:** The only readable durable truth is the one active pointer selecting one complete generation. Pending, failed, unselected, malformed, and future-schema state is unreadable.
- **D-06:** Reservation creates an opaque pending generation handle. Publication marks that same handle complete and rotates the single active pointer only after validation, within one protected publication transaction.
- **D-07:** A pending generation can never be activated. Selection is explicit; readers must not infer the active generation from insertion order, maximum ID, timestamps, or recency.
- **D-08:** Every injected publication failure leaves the candidate unreadable and preserves the previously active complete generation, including after closing and reopening the store.
- **D-09:** No active generation is a valid initial state. Once a complete generation has been published, failed later attempts must not turn that state into “no active generation.”

### Persistence and API Boundary
- **D-10:** Add one exact transactional migration containing only the relational generation handle, its minimal closed lifecycle status, and the singleton active selection needed by R1.
- **D-11:** A generation handle is an opaque store-local relational handle. It is not a semantic identity, cache key, source hash, run identity, or provider generation ID.
- **D-12:** Do not add run manifests, `InputSnapshot`, workspace identity, providers, capabilities, facts, layers, query/summary keys, dependency indexes, validation events, statistics, or Go/tool metadata.
- **D-13:** Keep all lifecycle operations and SQL behind the crate-private typed store facade. No `rusqlite` connection, transaction, statement, table name, row ID, or raw SQL surface may escape the store module.
- **D-14:** Preserve default-disabled behavior and the hard disabled short circuit before path validation or I/O. Add no public CLI, config, SDK, output, diagnostic, ordering, or exit-code contract.
- **D-15:** Reuse Phase 64 migration, connection, error-classification, ownership, writer-lease, and typed recovery policies. Future or malformed schema remains a typed private refusal without opportunistic mutation.

### Verification and Stop Gates
- **D-16:** Focused private-store tests must cover reservation, same-handle completion, rejection of pending activation, exact active selection, initial no-active state, reopen behavior, migration from the Phase 64 schema, and idempotent reopen.
- **D-17:** Inject failures at every state-changing publication seam and prove the previous active complete generation survives. Tests must authenticate persisted relationships and invariants, not merely count rows or inspect a header.
- **D-18:** No required individual test may exceed 60 seconds, and ordinary correctness tests may not be globally serialized. Exhaustive crash, scale, performance, and cross-platform matrices remain scheduled or merge-queue work.
- **D-19:** The required pull-request critical path must be at or below five minutes. GitHub Actions run `29752687999` on current `main` had an approximately 9 minute 52 second critical path, so the autonomous chain must stop before implementation unless planning identifies a truthful sub-five-minute required R1 path without bundling a broad CI redesign into R1.
- **D-20:** Crossing any size or CI budget requires the human split decision mandated by the restart plan. `--auto` does not waive this gate.
- **D-21:** Public `check` and `review` behavior must remain unchanged. Disabled mode still performs zero store I/O, and malformed/future state is contained in private typed outcomes.

### the agent's Discretion
- Internal Rust type names and whether lifecycle code lives beside the facade or in one new private module, provided the file and visibility budgets hold.
- Exact private table and column names, constraints, and indexes, provided the schema represents only the locked R1 state and validates its singleton/complete-selection invariants.
- The deterministic failure-injection mechanism and test fixture layout, provided production behavior is not exposed or weakened for tests.
- Whether reservation and publication are methods on a narrowly scoped private writer/session type or equivalent typed facade, provided raw SQLite values never cross the boundary.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and Roadmap Contract
- `.planning/PROJECT.md` — Defines polint’s product, performance, reliability, truthfulness, and repository constraints.
- `.planning/REQUIREMENTS.md` — Records the broader STORE-04, STORE-05, META-01, and META-04 milestone requirements that R1 must not prematurely close.
- `.planning/ROADMAP.md` — Defines the original Phase 65 goal and success criteria; this context narrows the next restart delivery unit without claiming the remaining criteria complete.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-CONTEXT.md` — Locks the private/default-disabled store boundary, cache-owned path, migration discipline, connection policy, and public-answer parity inherited by R1.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-VERIFICATION.md` — Verifies the exact Phase 64 foundation on `main` and identifies the reusable store artifacts.

### Phase 65 Restart Authority
- `research/local-semantic-store/RESTART-PLAN.md` — Canonical restart slices, readiness rule, publication invariants, PR budgets, and explicit rejected approaches.
- `research/local-semantic-store/IDENTITY-READINESS.md` — R0 gate approving only the R1 generation state machine and listing the exact permitted files, behaviors, and exclusions.
- `research/local-semantic-store/REVIEW-FINDINGS-TRIAGE.md` — Routes valid findings into R1, prerequisite, independent hardening, and test-architecture work.
- `research/local-semantic-store/decisions/DECISIONS.md` — Durable semantic-store decisions, including single active truth, relational IDs, small delivery units, and conservative non-reuse.
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — Private store/query boundary and complete-generation commit protocol.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-LEARNINGS.md` — Retained lifecycle invariants, failure lessons, reviewability limits, and patterns from the abandoned implementation.
- `.planning/forensics/report-20260719-phase-65-scope-collapse.md` — Root-cause analysis and mandatory human split triggers after the 85,000-line failed delivery.

### Boundary and Delivery Evidence
- `docs/API-VISIBILITY-PLAN.md` — Requires internal kernel/store surfaces to remain crate-private and protects the supported SDK/runner boundary.
- `.github/workflows/ci.yml` — Current pull-request workflow whose required-path shape and timing must be reconciled with the restart budget outside any R1 scope expansion.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/store/mod.rs`: Existing crate-private `SemanticStore`, `StoreConfig`, typed status/recovery vocabulary, cache-path ownership checks, and hard disabled short circuit.
- `crates/polint/src/analysis_kernel/store/connection.rs`: Existing private writer/read-only connection policy, `BEGIN IMMEDIATE` writer lease, WAL, foreign keys, bounded busy handling, and SQLite error classification.
- `crates/polint/src/analysis_kernel/store/migrations.rs`: Strict transactional `PRAGMA user_version` migration runner. Version 1 intentionally contains only the bootstrap marker, making one small R1 migration the next schema family.
- `crates/polint/src/analysis_kernel/store/tests.rs`: Existing temp-store, contention, corrupt/future/invalid-schema, safe-rebuild, and disabled-I/O fixture patterns.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs`: Existing input vocabulary is explicitly **not** ready for persistence and is a boundary witness, not an R1 implementation dependency.

### Established Patterns
- Store paths, SQL, migrations, and `rusqlite` values remain inside `analysis_kernel::store`; callers exchange narrow typed values only.
- Schema preflight rejects future versions before mutation and exact current-schema validation maps malformed state to private recovery outcomes.
- Store maintenance occurs after kernel computation/validation and cannot influence diagnostics or policy answers while the store remains private.
- Tests prefer owned temporary repositories/databases and typed fixture snapshots over exposing production database handles.
- Workspace `unreachable_pub` discipline means new shared internals should default to `pub(crate)` or tighter.

### Integration Points
- Extend the private store facade and migration table set; do not connect generation state to providers, rule execution, public output, or `InputSnapshot` in R1.
- Preserve `analysis_kernel`’s existing private `StoreStatus` reporting and post-validation ordering without adding a new public activation path.
- Use `.github/workflows/ci.yml` only to assess the delivery gate. If the required path cannot meet five minutes, stop and route a separate CI/test-architecture prerequisite rather than adding it to R1.

</code_context>

<specifics>
## Specific Ideas

- The decisive regression shape is: publish generation A; reserve generation B; inject a failure before, during, and after candidate completion/selection boundaries; reopen; generation A must still be the sole readable active generation.
- The active choice must be authenticated by an explicit singleton relationship to a complete generation. “Newest row wins” is forbidden.
- Start from the clean implementation on current `main`. Historical PR 83 is evidence only and must not be cherry-picked wholesale.
- Current CI evidence: [GitHub Actions run 29752687999](https://github.com/emilwareus/polint/actions/runs/29752687999) completed successfully on 2026-07-20, but its Windows library-test job made the workflow critical path approximately 9 minutes 52 seconds.

</specifics>

<deferred>
## Deferred Ideas

- **R2:** Minimal audited run manifest; do not persist `InputSnapshot` v1 wholesale.
- **R3:** Typed provider execution outcomes and capability revocation correctness.
- **R4:** Persist one audited provider family with its exact consumed-input identity.
- **R5:** Expand provider support incrementally, one proved family at a time.
- **R6:** Private enablement and one measured cold/warm reuse pair before any default promotion.
- Cross-file semantic-ID/cache-schema, syntax dependency, semantic graph cache-input, Go RTA-root, and related issues tracked in GitHub issues #86-#91.
- Whole dependency-index, `FactMeta`, validation-event, query, summary, layer, provider, capability, statistics, and language-tool metadata persistence.
- Any repository-wide CI/test-architecture change needed to establish a sub-five-minute required path. That is a prerequisite delivery unit, not hidden R1 work.
- Branch-protection/ruleset administration and other repository-policy changes.

</deferred>

---

*Phase: 65-generation-manifest-and-metadata-mirroring*
*Context gathered: 2026-07-28*
