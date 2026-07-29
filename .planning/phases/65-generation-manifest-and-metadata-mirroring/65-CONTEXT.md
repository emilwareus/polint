# Phase 65: Generation Manifest and Metadata Mirroring - Context

**Gathered:** 2026-07-29
**Status:** Ready for R4 planning; R1-R3 are accepted and the broader phase remains open

<domain>
## Phase Boundary

This planning pass covers only restart slice **R4: mirror one audited provider
family end to end**. The selected family is `polint.metrics`. Build on the
locked R1 generation lifecycle, R2 canonical run manifest, and R3 sealed
provider outcomes by:

1. auditing and narrowing the metrics provider's consumed-input and output
   identity boundary;
2. persisting its exact static manifest, sealed final outcome, success-only
   canonical output identity, and exact source/function dependency projection;
3. publishing and reopening that projection only as part of one authenticated
   complete generation; and
4. proving consumed changes invalidate, unrelated changes preserve an exact
   match, and durable tampering fails closed.

R4 persists **metadata, not metric facts**. It adds no second provider, generic
dependency-index persistence, normal-kernel publication, warm semantic-store
reuse, public CLI/config/SDK/output contract, telemetry/statistics persistence,
or CI redesign. Completing R4 must leave R5-R6, Phase 65, and
STORE-04/STORE-05/META-01/META-04 open.

</domain>

<decisions>
## Implementation Decisions

### Delivery Boundary
- **D-01:** Plan and implement R4 only. Treat `65-01-SUMMARY.md`,
  `65-02-SUMMARY.md`, `65-03-SUMMARY.md`, `65-04-SUMMARY.md`, the final clean
  `65-REVIEW.md`, and the passed `65-03-VERIFICATION.md` as locked R1-R3
  dependencies.
- **D-02:** Keep the restart stop budgets: at most three implementation tasks,
  fifteen product/test files, 2,500 handwritten added lines, one new durable
  schema family, and one persisted provider family.
- **D-03:** The only provider family in scope is `polint.metrics`. A repair
  needed solely to give that provider an honest identity/dependency contract
  belongs in R4; a repair that requires another provider migration, generic
  index redesign, fact persistence, cross-file semantic-ID repair, Go runtime
  certification, or public API expansion triggers a stop/split.
- **D-04:** R4 plans and summaries use `requirements: []`, record
  contributions separately, and do not mark Phase 65 or any mapped requirement
  complete.
- **D-05:** The user's deferred sub-five-minute CI follow-up remains deferred.
  Do not edit `.github/workflows/ci.yml`, alter timeouts, or absorb
  branch-protection/required-check work into R4.

### First Provider Family
- **D-06:** Use `polint.metrics` as the first durable provider family. It is a
  deterministic, multi-language, toolchain-free derived provider with an
  existing layer-cache seam and a small declared input vocabulary:
  `source_files` and `functions`.
- **D-07:** Do not choose `polint.source`: its workspace discovery/config
  boundary is broader, it is declared `NoCache`, and it is not the most useful
  first reusable provider proof.
- **D-08:** Do not choose Go or TypeScript syntax. R0 found syntax dependency
  coverage incomplete, while Go semantic/toolchain state has additional
  lifecycle and environment prerequisites.
- **D-09:** `polint.metrics` continues to produce all three declared metric
  families whenever any metric capability schedules it. Individual rule IDs,
  rule options, and which one of the three metric views caused scheduling do
  not alter the provider's output universe.

### Canonical Metrics Input and Output Identity
- **D-10:** Replace the current cache-mode-dependent provider identity with one
  canonical metrics output projection. The identity must be identical for cold
  computation, a validated warm layer-cache hit, and cache-disabled
  computation when the produced metrics are identical.
- **D-11:** Canonical output rows use normalized source paths and semantic
  metric fields rather than transient `FileId`, `FunctionId`, `FactRef::run_id`,
  insertion order, Rust `Debug`, or cache-file identity. Sort deterministically
  and preserve multiplicity where duplicate semantic rows affect output.
- **D-12:** The source input projection contains the complete normalized source
  membership plus the fields metrics actually reads or needs to authenticate
  those values: language, size/line semantics, and a purpose-tagged content
  digest. Reuse the R2 canonical source vocabulary where possible instead of
  inventing a competing source identity.
- **D-13:** The function input projection contains every non-synthetic function
  as a deterministic, multiplicity-aware tuple of normalized source path,
  function name, consumed span byte/line bounds, language, and cyclomatic
  complexity. Exclude `calls`, `is_test`, `is_exported`, unused columns, and
  transient fact IDs because metrics derivation does not read them.
- **D-14:** The complete source/function sets are semantic inputs: authenticate
  counts, bounds, canonical ordering, duplicates/multiplicity, relationships,
  and every stored field before trust. A digest is supporting evidence, never
  the sole proof that two decoded projections are equal.
- **D-15:** Rule code/options, the broad config digest, plan digest, setup
  checks, toolchain, extensions, models, cache telemetry, timing, and upstream
  layer-cache identities are not metrics behavior inputs and cannot enter its
  provider identity or durable dependency projection.
- **D-16:** Audit and narrow only `LayerKey::metrics_layer_key` and the
  metrics dependency-edge builder to the same consumed-input contract. Fixed
  generic key slots use explicit purpose-typed absence; do not redesign
  `LayerKey` or migrate any other provider.
- **D-17:** The metrics layer-cache key and durable provider projection may
  share canonical projection helpers, but the provider output identity is the
  canonical produced value—not the layer-cache key. Cache location, cache
  policy outcome, and cache mode cannot change semantic identity.

### Durable Manifest and Outcome Projection
- **D-18:** Add one private, explicitly versioned relational provider-mirror
  schema family. Multiple tightly owned header/member/dependency tables may
  implement that one family, but opaque JSON/blobs, Rust debug text, and raw SQL
  access outside the store boundary are forbidden.
- **D-19:** Persist the exact static `polint.metrics` manifest projection:
  provider ID/version, closed provider kind, canonical input/output members,
  closed language scope, closed cache policy, canonical schema name/version
  members, and precision ceiling. Decode through closed codecs and compare
  exactly with the current static manifest.
- **D-20:** Every generation published through the R4 facade has exactly one
  `polint.metrics` outcome row, even when the provider is planned absent,
  blocked, unsupported, setup-missing, or failed. Missing and duplicate rows
  are invalid.
- **D-21:** Persist exactly the R3 six-state outcome vocabulary with its legal
  shape. `Succeeded` alone carries provider ID/version/schema/precision plus
  canonical output digest; non-success carries no reusable identity.
- **D-22:** Failure stage/reason remain closed typed values with the R3
  compatibility matrix. Only `DependencyBlocked` carries blockers, stored as
  exact sorted unique manifest-authenticated provider IDs.
- **D-23:** Exact source/function dependency rows exist only for a succeeded
  reusable output. Non-success records its truthful outcome but presents no
  reusable dependency set or match.
- **D-24:** Derive one canonical provider-owned forward dependency projection
  and any needed reverse lookup from it. Do not persist the generic
  `DependencyIndex`, independent forward/reverse maps, stringly cache nodes, or
  unrelated provider edges.
- **D-25:** Keep provider mirror, outcome, identity, blockers, and dependencies
  relationally owned by the same generation. No row may outlive or cross-link
  its generation.

### Publication, Migration, and Read Semantics
- **D-26:** Extend the R2 private publication facade so the run manifest and
  complete metrics projection are written, read back, boundedly decoded,
  recomputed, and exactly compared before completion and active-pointer
  rotation in the same immediate transaction.
- **D-27:** Pending, failed, unselected, malformed, future-schema, and
  provider-incomplete generations remain unreadable. Active reads obtain
  generation, run manifest, metrics manifest/outcome, identity, blockers, and
  dependencies from one validated read snapshot.
- **D-28:** Publication failure at any injected boundary rolls back candidate
  manifest/provider rows, leaves the reserved candidate pending, and preserves
  the previous active complete generation through reopen.
- **D-29:** Schema v4 migration accepts only an exact empty v3 store. A
  populated v3 store has no truthful historical provider outcome and must be
  preserved without mutation and reported through the existing typed
  rebuild-needed/refusal path. Do not synthesize `PlannedAbsent` or delete
  history.
- **D-30:** Fresh/v0/v1/v2 empty stores may migrate transactionally through the
  established versions to the new exact current schema. Exact current-schema
  reopen is idempotent; malformed/current, future, or colliding owned schema
  remains preserved and refused.
- **D-31:** Add a private typed read/match result that distinguishes exact
  match, semantic miss, no active generation, and malformed/refused store
  state. Callers cannot make reuse decisions from a raw digest or raw stored
  row.
- **D-32:** R4 does not wire publication or matching into normal
  `AnalysisKernel::run`, does not make each normal run durable, and does not
  consume stored provider output. Kernel behavior remains maintenance-only
  until the measured private enablement in R6.

### Invalidation, Tamper, and Parity Proof
- **D-33:** Prove at least one decisive must-invalidate/must-preserve pair:
  changing consumed source content or a consumed function metric field must
  miss; changing unrelated config/rule state or an unused function field must
  preserve an exact provider match.
- **D-34:** Cover both declared input families in the focused matrix. Source
  membership/content/line semantics and consumed function
  name/span/language/complexity/multiplicity changes invalidate; `calls`,
  `is_test`, `is_exported`, rule/config, tool, model, extension, and telemetry
  changes preserve when the canonical metrics inputs/output are unchanged.
- **D-35:** Round-trip a real sealed metrics projection through reserve,
  publish, close, reopen, active read, and exact match. Also prove a
  `PlannedAbsent` or other legal non-success outcome round-trips without
  identity/dependency rows and can never match as reusable.
- **D-36:** Tamper tests cover static manifest fields/members, status
  stage/reason shape, blocker ownership/order/count, identity fields/digest,
  dependency kind/fields/count/multiplicity/relationships, missing/extra rows,
  wrong SQLite storage classes, and catalog/index/trigger/foreign-key drift.
- **D-37:** Enforce explicit row, scalar-length, and aggregate-byte limits and
  check SQLite storage classes/counts before attacker-controlled allocation.
  Stream bounded catalog and child-row validation rather than collecting
  unbounded attacker-controlled metadata.
- **D-38:** Preserve existing workspace-ownership, singleton-active,
  complete-generation, disabled-store, busy-writer, future-schema, and
  no-mutation refusal invariants.
- **D-39:** Focused cold/warm/cache-disabled tests compare canonical metrics
  provider identity and policy semantics while allowing only cache telemetry
  to differ. Existing invalid-cache recomputation and cache-write warning
  behavior remain intact.
- **D-40:** Preserve public bytes, diagnostics, ordering, exit semantics, SDK,
  runner, CLI, generated skill, docs/examples, and crate visibility. Extend
  negative public-surface coverage for any new private store vocabulary.

### Verification and Stop Gates
- **D-41:** No required individual test may exceed sixty seconds. Ordinary
  correctness tests use isolated temp stores/repos and may not use
  process-global serialization, sleeps, environment mutation, network, or
  release/full-workspace benchmarks.
- **D-42:** Use focused unit/integration targets for codecs, canonical
  projection, migration/schema authentication, publication rollback,
  round-trip/match, mutation pairs, parity, and public-surface leakage, followed
  by formatting, strict Clippy, workspace check, and diff/scope audits.
- **D-43:** If the implementation crosses any task/file/line/schema/provider
  budget, requires a second durable family, or cannot prove the R4 exit without
  normal-kernel reuse, stop and split rather than weakening the contract.
- **D-44:** Do not persist metric facts, `FactMeta`, validation events,
  statistics, the full `InputSnapshot`, layer/query/summary rows, or generic
  dependency indexes in R4.
- **D-45:** Do not expand to source, syntax, Go semantic, extension/model,
  summary/query, graph, solver, or other provider families. Their readiness and
  environment contracts remain R5 or later work.
- **D-46:** R4 acceptance means one provider family can be written, reopened,
  exactly validated/matched, and rejected after tampering. It does not mean
  every run is persisted or any stored analysis is reused.

### the agent's Discretion
- Exact crate-private type/module/table/index names and whether canonical
  metrics projection helpers live beside `metrics.rs`, incremental keys, or a
  narrow provider-mirror module.
- The precise relational decomposition of the one provider-mirror schema
  family, provided it keeps one canonical source of truth, exact ownership,
  bounded decoding, and strict schema authentication.
- Purpose labels, numeric bounds, and digest helper names, provided codecs are
  closed, stable, shared by write/read/recompute, and tested.
- The finite cfg(test)-only failure/tamper seams and focused fixture layout
  needed to exercise every locked invariant without adding public or
  process-global controls.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project, Phase, and Store Foundation
- `.planning/PROJECT.md` — Defines the private semantic-store product boundary,
  reliability/performance posture, and strict public API discipline.
- `.planning/REQUIREMENTS.md` — Defines STORE-04, STORE-05, META-01, and
  META-04; R4 contributes without completing them.
- `.planning/ROADMAP.md` — Defines the broader Phase 65 outcome and Phase 66
  dependency; this context deliberately limits the next delivery unit.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-CONTEXT.md` —
  Locks the cache-owned, typed, default-disabled SQLite boundary.

### Accepted R1-R3 Slice Contracts
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-01-SUMMARY.md`
  — Accepted crash-safe generation lifecycle.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-02-SUMMARY.md`
  — Accepted canonical run-manifest identity and exact-match seam.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-03-SUMMARY.md`
  — Accepted closed provider outcomes, dependency blocking, and capability
  revocation.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-04-SUMMARY.md`
  — Accepted R3 validation-ownership and complete cold/warm proof closure.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md`
  — Final clean cumulative R1-R3 review and retained history.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-03-VERIFICATION.md`
  — Passed R3-only 7/7 and 27/27 verification; explicitly leaves R4-R6 open.

### R4 Readiness, Scope, and Lessons
- `research/local-semantic-store/RESTART-PLAN.md` — Canonical R4 scope, exit
  proof, pull-request budgets, and stop/split policy.
- `research/local-semantic-store/IDENTITY-READINESS.md` — Marks
  `ProviderManifest` conditional, broad `LayerKey` and `DependencyIndex`
  not ready, `FactMeta` deferred, and mandates one-provider audit.
- `research/local-semantic-store/REVIEW-FINDINGS-TRIAGE.md` — Owns unresolved
  correctness/security findings and prevents unrelated finding absorption.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-LEARNINGS.md`
  — Locks provider-scoped canonical dependency projection, mutation-pair,
  telemetry separation, conservative omission, and bounded-review patterns.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-PATTERNS.md`
  — Records exact R1/R2 relational, migration, identity, and transaction
  patterns that R4 must extend rather than duplicate.
- `.planning/forensics/report-20260719-phase-65-scope-collapse.md` — Mandatory
  reviewability, stop/split, and no-big-bang guardrails.

### Durable Trust and Visibility
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — Defines
  validated complete-generation trust and fail-closed reuse.
- `research/local-semantic-store/decisions/DECISIONS.md` — Keeps computation
  truth canonical and requires conservative non-reuse for uncertified inputs.
- `docs/API-VISIBILITY-PLAN.md` — Supported API boundary; provider/store
  persistence vocabulary remains private.
- `.github/workflows/ci.yml` — Remains unchanged under the user's explicit
  deferral of sub-five-minute CI work.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/provider.rs`: Static deterministic
  `ProviderManifest` inventory and closed labels. The `polint.metrics` row
  declares `source_files`/`functions`, three metric outputs,
  `InMemoryDerived`, multi-language scope, schema `metrics-facts-1`, and syntax
  precision.
- `crates/polint/src/analysis_kernel/outcome.rs`: R3's closed six-state
  `ProviderOutcome`, success-only `ProviderOutputIdentity`, typed
  stage/reason, blocker validation, and manifest-order sealing.
- `crates/polint/src/metrics.rs`: Current metric derivation, payload
  validation, layer cache, source/function digest helpers, dependency builder,
  deterministic sort helpers, and focused warm/corruption tests.
- `crates/polint/src/analysis_kernel/incremental/keys.rs`: Existing metrics
  `LayerKey` constructor and typed digest slots. Its metrics-specific broad
  config/upstream inputs are the bounded one-provider audit target.
- `crates/polint/src/analysis_kernel/incremental/run_manifest.rs`: R2's
  canonical workspace/config/source projection, exact codecs, purpose-tagged
  digests, bounds, path normalization, and equality laws.
- `crates/polint/src/analysis_kernel/store/{mod.rs,generation.rs,migrations.rs,connection.rs,tests.rs}`:
  Private typed facade, immediate publication transaction, active read
  snapshot, strict catalog/content authentication, empty-only migration
  posture, failure injection, and tamper fixtures.
- `crates/polint/src/analysis_kernel/mod.rs`: Metrics executes last, receives
  sealed prerequisite identities, records a provisional output identity,
  undergoes authoritative validation, seals outcomes, and only then performs
  maintenance-only store work.

### Established Patterns
- Durable truth belongs to one explicitly selected complete generation.
  Reservation is durable but publication of authenticated children,
  completion, and selection are atomic.
- R2 writers and readers share a typed canonical codec and recompute identity
  from decoded fields; digests do not replace exact field comparison.
- Current metrics derivation always materializes all three metric families
  when scheduled, so the rule/plan details beyond the scheduling boolean are
  not behavior inputs.
- Current metrics cache identity includes broad config and upstream syntax
  output digests and its uncached provider-output digest follows a different
  path. R4 must repair this before persisting trust.
- Current function digests include `calls`, `is_test`, and `is_exported`, even
  though metrics computation reads only function membership, name, span,
  language, and complexity. This is the decisive false-invalidation seam.
- Generic `DependencyIndex` materializes independent forward/reverse stringly
  maps and is not a persistence contract. R4 needs one metrics-owned canonical
  projection only.

### Integration Points
- Introduce one canonical metrics input/output projection used by the metrics
  provider identity, metrics-only layer-key construction, durable writer,
  durable reader/recomputation, and mutation tests.
- Extend R2 publication inputs with one sealed metrics provider projection and
  validate it before completing/selecting the generation.
- Extend schema migration/authentication by exactly one provider-mirror family
  and one empty-v3-to-v4 transition; preserve all existing R1/R2 invariants.
- Add typed active-provider read/match operations behind `SemanticStore`
  without exposing rusqlite or wiring them into production reuse.
- Add focused tests near metrics/key, outcome codec projection, store
  migrations/publication, and public-surface leak gates; do not touch CI.

</code_context>

<specifics>
## Specific Ideas

- The decisive success fixture runs a small TypeScript repository requesting
  metrics, captures the sealed `polint.metrics` outcome and canonical
  source/function projection, publishes it with the R2 run manifest, closes
  and reopens the store, and obtains one exact typed match.
- The decisive input pair changes a consumed source or
  name/span/language/complexity field and gets a miss, then changes only an
  unrelated rule/config value or `calls`/`is_test`/`is_exported` and retains an
  exact match.
- The decisive identity parity fixture computes the same metrics cold, from a
  validated layer-cache hit, and with caching disabled. All three provider
  output identities and semantic results match; telemetry alone differs.
- A legal planned-absent record has the exact static manifest and typed
  planning-stage reason, but no identity, blockers, or dependencies. It is
  readable as outcome history and never reusable.
- A publication failure after any provider header/member/dependency write
  leaves the candidate pending and provider-incomplete while the old active
  generation remains the only readable truth after reopen.

</specifics>

<deferred>
## Deferred Ideas

- **R5:** Mirror additional providers one audited family at a time; keep syntax,
  Go semantic, extensions/models, and summary/query providers separate when
  their trust/environment inputs differ.
- **R6:** Wire private normal-run publication, measure one representative
  cold/warm semantic-store reuse pair, and decide whether any default
  enablement is justified.
- Metric fact payload persistence/restoration, `FactMeta`, validation-event
  persistence, store statistics, full `InputSnapshot`, generic
  `DependencyIndex`, and layer/query/summary persistence.
- Cross-file semantic-ID/cache-schema issues and other independently tracked
  product bugs in GitHub issues #86-#91.
- Go process/toolchain/filesystem security, monorepo lifecycle certification,
  optional/degraded provider semantics, and general panic containment.
- The separately deferred sub-five-minute required CI path, exhaustive
  scale/crash/platform matrices, branch protection, and ruleset
  administration.

</deferred>

---

*Phase: 65-generation-manifest-and-metadata-mirroring*
*Context gathered: 2026-07-29*
