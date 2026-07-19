# Local Semantic Store Restart Plan

Date: 2026-07-19

## Decision

Restart the semantic-store metadata feature from `origin/main`. Preserve the research and review findings, but do not carry the Phase 65 implementation forward as one unit and do not cherry-pick its commits wholesale.

The target user outcome remains:

> polint can safely remember validated analysis between processes, reuse only results whose inputs still match, and never serve a partial or corrupted write.

The first restart PRs remain private infrastructure. They must nevertheless have a small, testable behavioral contract and a short CI feedback loop.

## What remains valid

- SQLite/rusqlite is the private local store.
- `AnalysisDb` and the provider DAG remain the computation source of truth.
- A single active pointer selects readable truth.
- Pending or failed generations are never readable.
- SQLite row IDs are relational handles, not semantic identities.
- Public `check` and `review` behavior remains unchanged while the store is private and disabled.
- Telemetry does not participate in semantic identity.
- Store readers validate typed data before trusting it.

## What is explicitly rejected

- One phase or PR that repairs every kernel identity before storing anything.
- A 19-plan linear implementation.
- Persisting every provider, key family, dependency kind, and validation family at once.
- Solving general Go toolchain, process-containment, or filesystem security inside a metadata-store PR.
- Treating every valid review finding as same-PR work.
- Running exhaustive performance and cross-platform suites as required PR checks.
- Raising CI timeouts to accommodate an oversized suite.

## Readiness rule

Before a type becomes a durable identity input, prove all of the following in a focused test or audit:

1. Its semantic fields are explicit and stable.
2. It has a canonical, versioned codec.
3. Equality, ordering, and hashing use the same semantic representation.
4. Status values distinguish absent, unsupported, setup-missing, failed, and succeeded where applicable.
5. Machine-local and execution-only telemetry is excluded.
6. Every behavior-affecting input is either represented or the result is declared non-reusable.

If any item fails, stop the current store slice. Open a separate prerequisite issue/PR or conservatively omit that identity from reuse.

## Delivery slices

### R0 — Identity readiness audit

**Type:** tests and documentation only.

- Inventory existing `InputSnapshot`, provider outcome, `LayerKey`, dependency edge, validation event, and manifest contracts.
- Record which contracts are ready, conditionally usable, or not reusable.
- Add no new production identity vocabulary.
- Produce the exact schema inputs allowed in R1 and R2.

**Exit:** one reviewed readiness table; no product code; required CI under five minutes.

Initial audit: [IDENTITY-READINESS.md](IDENTITY-READINESS.md). It approves R1
and blocks full-snapshot, trusted-provider, whole-dependency-index, and fact
metadata persistence until their prerequisites are isolated.

### R1 — Minimal generation state machine

**Type:** one storage invariant.

- Build on the Phase 64 private SQLite facade already on main.
- Add only generation handle, status, and active selection needed for pending → complete → active.
- Exercise rollback, interruption seams, old-active preservation, and exact active selection.
- Store no provider, fact, query, dependency, or Go-tool metadata.

**Exit:** a previous complete generation survives every injected R1 publication failure.

### R2 — Minimal run manifest

**Type:** one canonical payload.

- Persist only the already-approved run/workspace/config identity projection from R0.
- Round-trip through typed decoding and recompute its identity before activation and reuse.
- Do not migrate provider cache keys or invent `InputSnapshot v2` in this PR.

**Exit:** same input reopens identically; a scalar tamper fails closed; unrelated telemetry changes do not alter identity.

### R3 — Provider outcome correctness

**Type:** independent kernel prerequisite.

- Introduce or repair typed provider outcomes without any SQLite changes.
- Ensure failure, dependency blocking, planned absence, and success remain distinct.
- Revoke hard capabilities after execution or authoritative validation failure.

**Exit:** provider trust is correct cold and warm before the store mirrors it.

### R4 — Mirror one provider family

**Type:** one producer end to end.

- Select the simplest provider declared ready by R0.
- Persist its manifest, outcome, output identity, and exact dependencies.
- Add one must-invalidate and one must-preserve-hit mutation pair.
- Do not generalize other providers in the same PR.

**Exit:** one provider family can be written, reopened, validated, and rejected after tampering.

### R5 — Expand provider mirroring incrementally

Repeat R4 in small PRs grouped only when providers share the same already-proven contract. Go semantic, extensions/models, and summary/query providers remain separate because their environment and trust inputs differ.

### R6 — Private enablement and measured reuse

- Enable the store only in an internal/test configuration.
- Measure one representative cold/warm pair.
- Keep exhaustive scale, crash, and platform matrices in scheduled or merge-queue workflows.
- Promote default reuse only after the required PR path remains below five minutes and the scheduled suite is stable.

## Pull-request budgets

These are stop-and-split triggers, not aspirational metrics:

- maximum 3 implementation tasks;
- maximum 15 changed product/test files unless explicitly approved;
- maximum 2,500 handwritten added lines unless explicitly approved;
- maximum 1 new durable schema family per PR;
- maximum 1 provider family per first implementation;
- required CI critical path at or below 5 minutes;
- no required individual test above 60 seconds;
- no global serialization of ordinary correctness tests;
- no new platform runtime subsystem unless that subsystem is the stated PR goal.

Crossing a trigger pauses implementation and requires a human split decision.

## Review policy

Every finding receives one disposition:

- **Fix now:** introduced by the PR or required for its stated invariant.
- **Block and split:** reveals a missing prerequisite without which the PR is unsafe.
- **Independent backlog:** valid bug outside the changed contract.
- **Design removed:** applies only to an implementation deliberately omitted from the restart.
- **Rejected:** not reproducible or outside the documented threat model.

The loop converges when all **Fix now** findings are resolved and re-reviewed. It does not require implementing independent backlog findings.

## Test strategy

- Unit tests: pure, parallel, and fast.
- Store lifecycle tests: in-memory or tiny temporary databases with injected boundaries.
- Provider mutation tests: one provider and one mutation pair at a time.
- Platform checks: focused path/filesystem/process canaries only where the PR changes platform code.
- Performance tests: separate optimized job, compiled once, not called a smoke, and not part of every PR unless performance is the PR’s goal.
- Full workspace, exhaustive fixtures, crash matrices, and scale gates: scheduled or merge queue.

## Salvage policy for the abandoned branch

Salvage:

- review findings and their regression ideas;
- canonical generation-state invariants;
- provider outcome lessons;
- identity mutation cases;
- evidence of CI bottlenecks;
- small code fragments only after independent re-derivation and review.

Do not salvage wholesale:

- the 19-plan commit chain;
- the complete schema and all-family commit plan;
- the Go toolchain/process/filesystem hardening stack;
- the serialized fixture harness;
- generated baselines tied to the abandoned implementation.

## First next action

After these documents are merged to main, create a fresh branch from updated `origin/main` and execute R0 only. Do not start R1 until the readiness table has been reviewed and the proposed R1 file list fits the pull-request budgets above.
