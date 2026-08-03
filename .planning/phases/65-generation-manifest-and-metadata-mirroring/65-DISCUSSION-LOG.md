# Phase 65: Generation Manifest and Metadata Mirroring - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-28 through 2026-07-29 (R1, R2, and R3 entries)
**Phase:** 65-generation-manifest-and-metadata-mirroring
**Mode:** `--auto --chain`
**Areas discussed:** R1 lifecycle; R2 canonical manifest; R3 provider outcome taxonomy, dependency blocking, authoritative validation, capability revocation, and cold/warm parity

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

---

# R2 Update: Minimal Audited Run Manifest

The R1 entries above remain the audit record for Plan 01. This update records
the automatically selected decisions for the next independently reviewable
slice only.

## Canonical Manifest Projection

### Persisted inputs

| Option | Description | Selected |
|--------|-------------|----------|
| Workspace + config + sources | One versioned payload with workspace ownership, complete config identity, and canonical source membership/content rows. | ✓ |
| Workspace + config only | Omit source membership even though a source change would describe a different run. | |
| Full `InputSnapshot` v1 | Persist rule/plan, lifecycle, tool, model, extension, provider, mtime, and placeholder fields together. | |

**User's choice:** `--auto` selected the R0-approved workspace/config/source projection.
**Notes:** Source rows contain only normalized path, closed language, source digest, and byte size. Rules, plans, providers, capabilities, tools, lifecycle inputs, models, extensions, layers, queries, summaries, facts, validation events, dependency indexes, statistics, and telemetry remain out.

### Equality boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Exact typed payload | Recompute the run identity from decoded canonical fields and compare all fields exactly. | ✓ |
| Digest only | Treat a stored 64-bit fingerprint as sufficient proof of equality. | |
| Opaque JSON | Compare serialized blobs without a shared typed codec. | |

**User's choice:** `--auto` selected exact typed equality with the digest as supporting evidence.
**Notes:** Writer, reader, recomputation, and tests share one versioned codec. Rust `Debug` is never identity material.

---

## Workspace Ownership Identity

### Workspace binding

| Option | Description | Selected |
|--------|-------------|----------|
| Opaque purpose-typed root identity | Hash the canonical normalized workspace root and persist only its purpose/value. | ✓ |
| Raw absolute path | Store the machine-local root in the manifest. | |
| Cache path only | Infer workspace ownership from the configured cache location. | |

**User's choice:** `--auto` selected the opaque workspace identity.
**Notes:** This also separates workspaces when `POLINT_CACHE_DIR` is shared. Raw roots, user names, home paths, and temporary paths never persist or reach public output.

---

## Publication and Read Binding

### Publication

| Option | Description | Selected |
|--------|-------------|----------|
| One authenticated transaction | Write/decode/recompute the complete manifest, complete the reserved generation, and rotate active selection atomically. | ✓ |
| Staged independent commits | Persist manifest rows before a later completion/selection transaction. | |
| Digest-led activation | Activate after checking only the stored run digest. | |

**User's choice:** `--auto` selected one authenticated publication transaction.
**Notes:** Every injected failure leaves the candidate pending and manifest-unreadable while preserving the previous manifested active generation through reopen.

### Reuse seam

| Option | Description | Selected |
|--------|-------------|----------|
| Typed exact match/miss/refusal | Read active handle and manifest in one snapshot; return reusable only after decode, recomputation, and exact expected equality. | ✓ |
| Raw stored digest | Let later callers decide whether a digest is trustworthy. | |
| Enable warm provider reuse | Consume persisted provider results in R2. | |

**User's choice:** `--auto` selected the typed private seam without enabling reuse.
**Notes:** No-active, semantic mismatch, and malformed/refused state remain distinct.

---

## Schema-v2 Migration Posture

| Option | Description | Selected |
|--------|-------------|----------|
| Empty-only v2 migration | Migrate exact empty v2; preserve and refuse populated v2 through rebuild-needed because no manifest can be reconstructed honestly. | ✓ |
| Synthetic legacy manifest | Invent workspace/config/source provenance for existing generations. | |
| Destructive transition | Delete rows or clear the active pointer during migration. | |

**User's choice:** `--auto` selected empty-only migration and no-mutation refusal.
**Notes:** Fresh/v0/v1 stores may migrate through to empty v3. Exact v3 reopen stays idempotent; mixed manifested/manifestless complete generations are forbidden.

---

## Tamper, Bounds, and Telemetry Proof

### Durable representation

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit bounded typed rows | Use relational header/source columns, derived path order, authenticated count, strict labels, and allocation preflight. | ✓ |
| JSON blob | Persist the payload as one extensible opaque value. | |
| Debug text | Persist Rust formatting and parse it on reopen. | |

**User's choice:** `--auto` selected explicit bounded typed rows.
**Notes:** Preflight covers SQLite storage classes, row count, per-field length, aggregate bytes, relationships, and declared child count before allocation. No persisted ordinal is trusted.

### Regression proof

| Option | Description | Selected |
|--------|-------------|----------|
| Mutation matrix + structural exclusion | Round-trip, parent/child tampering, exact mismatch, failure rollback, and proof that telemetry fields cannot enter identity. | ✓ |
| Digest equality only | Check two happy-path fingerprints. | |
| Header/count smoke | Inspect schema version and row counts only. | |

**User's choice:** `--auto` selected the mutation matrix and structural telemetry exclusion.
**Notes:** Timestamps, mtimes, durations, cache counters/outcomes, insertion order, and creation time cannot affect identity. Individual focused tests remain under 60 seconds and ordinary correctness tests remain parallel.

---

## the agent's Discretion for R2

- Private type/module/table names and the exact typed match-result vocabulary.
- Lossless platform-aware workspace-root encoding before hashing.
- Concrete count/byte limits that are explicit, preflighted, tested, and appropriate for large repositories.
- Transactional statement order and finite cfg(test)-only failure/tamper seams within the locked invariants.

## Deferred Ideas after R2

- R3 typed provider outcomes and capability revocation.
- R4 one audited provider family and exact consumed-input dependency pair.
- R5 incremental provider expansion.
- R6 private enablement and one measured cold/warm reuse pair.
- Full `InputSnapshot`, provider/capability, lifecycle/tool, layer/query/summary, dependency-index, validation-event, statistics, and `FactMeta` persistence.
- GitHub issues #86-#91 and the sub-five-minute CI follow-up.

---

# R3 Addendum: Provider Outcome Correctness

**Date:** 2026-07-29
**Mode:** `--auto --chain`

## Provider Outcome Taxonomy

| Option | Description | Selected |
|--------|-------------|----------|
| Closed six-state contract | Distinguish succeeded, failed, dependency-blocked, unsupported, setup-missing, and planned-absent. | ✓ |
| Four-state minimum | Merge unsupported/setup-missing/absence into fewer states. | |
| Digest inference | Infer outcome from optional output digests, empty facts, or diagnostics. | |

**User's choice:** `--auto` selected the closed six-state contract.
**Notes:** A provider is succeeded only after real execution, authenticated
output identity, and authoritative validation. Only success carries a reusable
output identity. Typed reason/stage data is semantic; diagnostics, cache
counters, warnings, and timing are telemetry.

## Dependency Blocking and Planned Absence

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit hard-edge blocking | Record one outcome per manifest; block only consumers of non-success producers and continue independent branches. | ✓ |
| Sentinel continuation | Pass absent digests or empty universes to downstream consumers. | |
| Global abort | Abort every provider branch on the first controlled failure. | |

**User's choice:** `--auto` selected explicit hard-edge blocking.
**Notes:** Plan gating comes first, so unrequested providers are
planned-absent. A scheduled consumer whose required producer is absent,
unsupported, setup-missing, failed, or blocked is dependency-blocked. R3 models
current actual provider-output edges only; it does not redesign or persist the
general dependency index and does not invent optional/degraded semantics.

## Authoritative Validation and Capability Revocation

| Option | Description | Selected |
|--------|-------------|----------|
| Structured validation and fail-closed revocation | Return typed issues with ownership, downgrade implicated outcomes, derive effective hard capabilities, and skip affected rules. | ✓ |
| Diagnostic-only validation | Leave provider trust unchanged after rendering validation diagnostics. | |
| Partial-fact rule execution | Run rules and ask each rule to handle missing or invalid facts. | |

**User's choice:** `--auto` selected structured validation and fail-closed
revocation.
**Notes:** An unowned authoritative issue downgrades all provisional successes
rather than certifying a partial universe. Each affected rule is skipped before
fact access and receives one deterministic existing-style capability
diagnostic; independent rules continue. The machinery stays crate-private and
does not add a public capability-status variant or output schema.

## Cold/Warm Parity and Verification

| Option | Description | Selected |
|--------|-------------|----------|
| Semantic parity, telemetry-only differences | Require identical outcomes, identities, blockers, capabilities, diagnostics, and policy results across cold/warm runs. | ✓ |
| Warm-specific trust | Treat a cache hit itself as provider success. | |
| Exhaustive benchmark matrix | Pull the full cross-platform/scale suite into R3. | |

**User's choice:** `--auto` selected semantic parity with focused proof.
**Notes:** Invalid cached payloads are rejected and recomputed or become a real
provider failure; cache I/O warnings after valid computation remain telemetry.
Tests cover the outcome matrix, dependency chain plus independent branch,
planned absence, execution and validation failures, rule skipping, and one
representative cold/warm pair. No test may exceed sixty seconds or require
global serialization.

## the agent's Discretion for R3

- Private module/type names and the narrow outcome tracker representation.
- The explicit in-memory representation of current hard provider-output edges.
- Structured validation issue details and deterministic attribution helpers.
- Finite cfg(test)-only failure seams and the representative cached provider.

## Deferred Ideas after R3

- R4 persistence for one audited provider family and exact dependency pair.
- R5 incremental provider expansion and R6 private store-reuse enablement.
- Whole dependency-index, key-family, snapshot, fact, validation-event, query,
  summary, and store-stat persistence.
- Provider-specific cache-key audits, optional/degraded semantics, Go runtime
  hardening, general panic containment, GitHub issues #86-#91, and the
  sub-five-minute CI follow-up.

---

# R4 Addendum: Mirror One Audited Provider Family

**Date:** 2026-07-29
**Mode:** `--auto --chain`
**Areas discussed:** first provider family; durable provider projection;
canonical identity and exact dependency edges; publication and read semantics;
mutation and tamper proof; R4 scope and stop gates

## First Provider Family

| Option | Description | Selected |
|--------|-------------|----------|
| `polint.metrics` | Audit the deterministic, multi-language, toolchain-free derived provider with an existing cache seam. | ✓ |
| `polint.source` | Start with workspace discovery/config despite `NoCache` and a broader input boundary. | |
| Go or TypeScript syntax | Start with a parser provider whose dependency-edge readiness is not yet certified. | |

**User's choice:** `--auto` selected `polint.metrics`.
**Notes:** R0 blocks generic syntax persistence and Go environment
certification. Metrics has only two declared fact inputs and produces a useful
reusable family, but its current cache identity must first be narrowed.

## Durable Provider Projection

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit typed relational projection | Store the exact static metrics manifest and sealed R3 outcome in one versioned provider-mirror family. | ✓ |
| Opaque serialized blob | Store `ProviderManifest`/`ProviderOutcome` as JSON or another open serialized payload. | |
| Digest only | Persist only an output fingerprint and leave interpretation to callers. | |

**User's choice:** `--auto` selected explicit typed relational projection.
**Notes:** Every R4-published generation carries exactly one metrics outcome for
the closed six-state vocabulary. Only success may carry reusable identity and
dependency rows.

## Canonical Identity and Exact Dependency Edges

| Option | Description | Selected |
|--------|-------------|----------|
| Canonical metrics value and input projections | Use normalized path-based output rows and complete source/function consumed-input sets. | ✓ |
| Current layer-cache identity unchanged | Persist the current broad config/upstream-dependent digest, including its cache-mode split. | |
| FactMeta summary | Derive durable identity from transient fact/run IDs and metadata summaries. | |

**User's choice:** `--auto` selected canonical metrics projections.
**Notes:** The metrics-only `LayerKey` is narrowed to the same audited
source/function boundary. Broad config, rule/plan, toolchain, extension/model,
telemetry, and upstream cache identities are excluded. The generic
`DependencyIndex` is not persisted.

## Publication and Read Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Complete-generation atomic publication | Write, decode, recompute, and compare manifest plus provider metadata before completing and selecting. | ✓ |
| Post-activation provider write | Activate the run manifest first and fill provider metadata afterward. | |
| Pending-row reads | Let readers consume provider rows before generation completion. | |

**User's choice:** `--auto` selected atomic complete-generation publication.
**Notes:** Active reads use one validated snapshot. Exact empty v3 stores may
migrate; populated v3 stores are preserved and refused because no honest
historical outcome can be reconstructed. The private facade stays unwired from
normal kernel publication and reuse until R6.

## Mutation and Tamper Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Consumed-field miss plus unrelated-field hit matrix | Change real source/function inputs to miss and unrelated/unused fields to preserve exact match. | ✓ |
| Digest smoke test | Assert only that one aggregate digest changes. | |
| Broad conservative invalidation | Keep config and unused fact fields as dependencies. | |

**User's choice:** `--auto` selected the consumed/unrelated mutation matrix.
**Notes:** Cold, warm layer-cache, and cache-disabled identities must agree.
Focused proof also covers strict catalog/content authentication, bounded
decoding, row/shape/relationship tampering, reopen rejection, and publication
rollback preserving the old active generation.

## R4 Scope and Stop Gates

| Option | Description | Selected |
|--------|-------------|----------|
| Restart hard budgets | At most three tasks, fifteen product/test files, 2,500 additions, one schema family, and one provider family. | ✓ |
| Advisory budgets | Continue if coherent even after crossing the restart limits. | |
| Opportunistic expansion | Add adjacent providers, facts, or reuse while touching the store. | |

**User's choice:** `--auto` kept the restart hard budgets.
**Notes:** R4 uses `requirements: []` and leaves Phase 65 plus all mapped
requirements open. Facts, other providers, generic indexes, normal-run
publication, warm store reuse, public surfaces, and the user-deferred
sub-five-minute CI redesign remain out of scope.

## the agent's Discretion for R4

- Private projection/module/table/index names and relational decomposition
  within the one provider-mirror family.
- Closed purpose labels, bounded limits, and digest helper names.
- Finite cfg(test)-only tamper/failure seams and focused fixture organization.

## Deferred Ideas after R4

- R5 provider-by-provider expansion.
- R6 private normal-run enablement and one measured cold/warm store-reuse pair.
- Fact payloads/`FactMeta`, validation events, statistics, full snapshots,
  generic dependency indexes, and layer/query/summary persistence.
- Syntax/Go environment readiness, optional/degraded provider semantics,
  GitHub issues #86-#91, and the sub-five-minute CI follow-up.

---

# R5 Addendum: First Increment — Go Syntax Provider Mirror

> **Audit trail only.** Do not use as input to planning, research, or
> execution agents. Decisions are captured in `65-CONTEXT.md`; this section
> preserves the alternatives considered.

**Date:** 2026-08-03
**Phase:** 65-generation-manifest-and-metadata-mirroring
**Mode:** `--auto`
**Areas discussed:** next provider, exact syntax identity, cache-warning
separation, durable row shape, publication and matching, scope and stop gates

## Next Provider

| Option | Description | Selected |
|--------|-------------|----------|
| `polint.go.syntax` only | Prove the smallest in-process single-language syntax contract first. | ✓ |
| `polint.ts.syntax` only | Start with Oxc and its four TS/JS source modes. | |
| Both syntax providers | Repair and mirror the whole syntax family in one increment. | |
| `polint.source` | Mirror discovery despite `NoCache` and its broader filesystem/config boundary. | |

**User's choice:** `--auto` selected `polint.go.syntax` only.
**Notes:** This is the in-process tree-sitter provider, not Go semantic
analysis or an external Go command. One provider stays inside the restart
budget and proves the shared syntax pattern before TypeScript is attempted.
GitHub issue #89 remains only partially addressed.

## Exact Syntax Identity

| Option | Description | Selected |
|--------|-------------|----------|
| Exact consumed-input and produced-value projections | Bind normalized Go sources and an explicit parser contract to a canonical syntax result. | ✓ |
| Current broad `LayerKey` | Keep config and rendered/debug parser inputs even when parsing does not consume them. | |
| Full run-manifest equality | Invalidate Go syntax for every config or non-Go workspace change. | |

**User's choice:** `--auto` selected exact consumed-input and produced-value
projections.
**Notes:** Go paths/source bytes and the explicit provider/schema/parser
contract participate. Config, rules, plan, TypeScript sources, Go module
lifecycle, external toolchains, extensions/models, cache disposition, and
telemetry do not. Unknown behavior inputs must be represented or trigger a
split.

## Cache-Warning Separation

| Option | Description | Selected |
|--------|-------------|----------|
| Separate semantic diagnostics from cache telemetry | Hash parser diagnostics/facts, but keep cache I/O warnings outside payload identity. | ✓ |
| Hash every diagnostic | Let cache failures rotate semantic provider identity. | |
| Drop all diagnostics | Exclude both parser and cache diagnostics from returned behavior. | |

**User's choice:** `--auto` selected semantic/operational separation.
**Notes:** Cold, warm, and cache-disabled runs with the same parser result must
seal the same provider identity. Existing user-visible cache warnings remain
run-local where applicable and must not be replayed as cached parser output.

## Durable Row Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Provider-owned relational family | Add one exact versioned Go-syntax mirror beside the accepted metrics family. | ✓ |
| Generic all-provider redesign | Replace the metrics-specific schema with a framework for every provider. | |
| Opaque payload or digest-only row | Store JSON/blob/debug state or trust a fingerprint without exact dependencies. | |

**User's choice:** `--auto` selected one provider-owned relational family.
**Notes:** The mirror contains static manifest, sealed outcome, success-only
identity, blockers, and exact Go-source/parser dependencies—never source text
or syntax facts. Small closed codec sharing is allowed; framework
generalization is not.

## Publication and Matching

| Option | Description | Selected |
|--------|-------------|----------|
| Atomic complete-generation publication | Validate run manifest, metrics mirror, and Go syntax mirror before completion/selection. | ✓ |
| Independent post-activation write | Select the generation before Go syntax metadata is complete. | |
| Pending-generation reads | Let readers consume uncommitted provider rows. | |

**User's choice:** `--auto` selected atomic complete-generation publication.
**Notes:** Populated schema-v4 stores are preserved/refused because historical
Go outcomes cannot be invented. Active reads use one authenticated snapshot;
normal kernel runs remain maintenance-only until R6.

## Scope and Stop Gates

| Option | Description | Selected |
|--------|-------------|----------|
| Restart hard budgets | At most three tasks, fifteen product/test files, 2,500 additions, one schema family, and one new provider. | ✓ |
| Advisory budgets | Continue after crossing the reviewability limits. | |
| Opportunistic expansion | Add TypeScript, facts, generic indexes, normal reuse, or CI work while nearby. | |

**User's choice:** `--auto` kept the restart hard budgets.
**Notes:** The plan uses `requirements: []` and leaves R5 expansion, R6, Phase
65, and all mapped requirements open. The explicitly deferred sub-five-minute
CI follow-up remains untouched.

## the agent's Discretion for This R5 Increment

- Private module/type/table/index names and relational decomposition.
- Small closed manifest/outcome codec reuse that preserves R4 exactly.
- Stable parser-contract labels, bounded limits, digest helper names, and
  finite test-only failure/tamper seams.

## Deferred Ideas after This R5 Increment

- Mirror `polint.ts.syntax` in a separate R5 increment and finish the
  TypeScript half of GitHub issue #89.
- Audit remaining provider families independently, with Go semantic,
  extensions/models, summaries, and queries kept separate.
- R6 private normal-run publication and one measured cold/warm store pair.
- Phase 66 fact/`FactMeta` persistence and restoration.
- GitHub issues #86, #90, and #91, external Go runtime hardening, and the
  separately deferred sub-five-minute CI follow-up.
