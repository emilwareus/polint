# Phase 65: Generation Manifest and Metadata Mirroring - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-28
**Phase:** 65-generation-manifest-and-metadata-mirroring
**Mode:** `--auto --chain`
**Areas discussed:** Delivery boundary, generation lifecycle semantics, persistence and API boundary, verification and stop gates

---

## Delivery Boundary

### Next executable unit

| Option | Description | Selected |
|--------|-------------|----------|
| Original Phase 65 | Implement manifests, snapshots, providers, layers, indexes, validation events, statistics, and metadata together. | |
| R1 only | Implement the independently mergeable private generation state machine approved by the readiness audit. | ✓ |
| Rewrite roadmap first | Pause all implementation until the roadmap is reorganized. | |

**User's choice:** `--auto` selected the recommended R1-only boundary.
**Notes:** `IDENTITY-READINESS.md` marks R1 GO and the wider identity families NO-GO or deferred. The forensic report and restart plan explicitly reject another all-at-once delivery.

### Delivery budgets

| Option | Description | Selected |
|--------|-------------|----------|
| Advisory budgets | Continue when the implementation exceeds restart limits if the code appears coherent. | |
| Hard split/stop gates | Pause at more than three tasks, fifteen files, 2,500 handwritten lines, one schema family, or one provider family. | ✓ |
| Historical salvage | Reuse the abandoned branch to reduce new implementation work. | |

**User's choice:** `--auto` selected the restart plan’s hard gates.
**Notes:** Historical material remains useful as evidence and regression ideas, but wholesale cherry-picking would recreate the failed delivery shape.

---

## Generation Lifecycle Semantics

### Readable truth

| Option | Description | Selected |
|--------|-------------|----------|
| Newest row | Read the newest generation regardless of explicit state. | |
| Active complete only | Read only the single explicitly selected complete generation. | ✓ |
| Caller selection | Return all complete generations and make each consumer choose. | |

**User's choice:** `--auto` selected one active complete generation.
**Notes:** Pending, failed, unselected, malformed, and future-schema state stays unreadable. Recency is not activation.

### Publication boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Activate on reservation | Make the candidate readable as soon as a handle exists. | |
| Complete and select atomically | After validation, mark the reserved handle complete and rotate the singleton active pointer in one protected publication transaction. | ✓ |
| Best-effort writes | Complete and select in independent writes with recovery based on newest state. | |

**User's choice:** `--auto` selected atomic completion and active-pointer rotation.
**Notes:** A pending handle can never be selected. Publication failures preserve the previous active complete generation through reopen.

---

## Persistence and API Boundary

### Durable schema

| Option | Description | Selected |
|--------|-------------|----------|
| Future-complete schema | Precreate the broad manifests, providers, facts, dependencies, and metadata model. | |
| Minimal generation schema | Add only the handle, minimal lifecycle status, and singleton active selection needed by R1. | ✓ |
| Memory only | Avoid durable lifecycle state until all later identities are ready. | |

**User's choice:** `--auto` selected one exact minimal generation-state migration.
**Notes:** The handle is an opaque store-local relational ID, not a semantic identity or provider generation key.

### Product surface

| Option | Description | Selected |
|--------|-------------|----------|
| Experimental CLI | Add a hidden or experimental way for users to enable/inspect the store. | |
| Private typed facade | Keep lifecycle state crate-private, default-disabled, and behind typed operations. | ✓ |
| Raw internal SQL | Let internal callers access SQLite connections or statements directly. | |

**User's choice:** `--auto` preserved the Phase 64 private boundary.
**Notes:** No public CLI, config, SDK, output, diagnostic, ordering, or exit-code change is allowed.

---

## Verification and Stop Gates

### Correctness evidence

| Option | Description | Selected |
|--------|-------------|----------|
| Broad suite only | Depend on the existing workspace regression suite. | |
| Focused lifecycle fixtures | Test every transition, invalid selection, reopen, migration, and injected publication-failure seam. | ✓ |
| Happy path only | Prove reservation and one successful publication. | |

**User's choice:** `--auto` selected focused invariant and failure-injection tests.
**Notes:** Tests must prove the previous active generation remains the sole readable truth, not just check row counts.

### CI economics

| Option | Description | Selected |
|--------|-------------|----------|
| Ignore baseline | Implement R1 even when the required pull-request path exceeds five minutes. | |
| Plan, then gate execution | Produce a bounded R1 plan but stop before implementation unless a truthful sub-five-minute required path exists. | ✓ |
| Bundle CI redesign | Expand R1 to redesign the repository’s complete test workflow. | |

**User's choice:** `--auto` selected the hard execution gate.
**Notes:** The latest `main` CI run inspected during discussion, run `29752687999`, was green but had an approximately 9 minute 52 second critical path. The restart plan requires a human split decision when the five-minute trigger is crossed; `--auto --chain` does not silently waive that project constraint.

**Later human decision (2026-07-28):** The user explicitly directed, “Let's skip the sub 5 minute CI, we can follow up with that later.” This satisfies the required human decision and removes CI latency as an R1 execution blocker. The measured baseline and follow-up remain recorded; CI redesign is still outside R1.

### Test architecture

| Option | Description | Selected |
|--------|-------------|----------|
| Exhaustive required matrix | Run crash, scale, performance, and every platform scenario on every pull request. | |
| Focused required path | Keep individual required tests below 60 seconds and move exhaustive matrices to scheduled or merge-queue workflows. | ✓ |
| Raise timeouts | Preserve the current shape by increasing job/test timeouts. | |

**User's choice:** `--auto` selected the focused required path.
**Notes:** Ordinary correctness tests may not be globally serialized. Any repository-wide CI restructuring is a separate prerequisite, not R1 scope.

---

## the agent's Discretion

- Private Rust type and module names within the locked file/visibility budget.
- Exact private relational names and constraints that implement only the R1 state machine.
- Deterministic failure-injection and fixture mechanics.
- The narrow typed facade shape used to hold the writer/publication transaction.

## Deferred Ideas

- R2 minimal run manifest.
- R3 typed provider outcomes and capability revocation.
- R4 one audited provider family.
- R5 incremental provider expansion.
- R6 private enablement and measured reuse.
- `InputSnapshot`, provider/capability, layer/query/summary, dependency-index, validation-event, statistics, `FactMeta`, and Go/tool metadata persistence.
- Independently tracked correctness issues #86-#91.
- Repository-wide CI architecture and branch-protection administration.
