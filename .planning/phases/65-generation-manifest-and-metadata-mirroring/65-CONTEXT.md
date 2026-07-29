# Phase 65: Generation Manifest and Metadata Mirroring - Context

**Gathered:** 2026-07-29
**Status:** Ready for R3 planning; R1 and R2 are complete and the broader phase remains open

<domain>
## Phase Boundary

This planning pass covers only restart slice **R3: provider outcome
correctness**. Build on the locked R1 generation lifecycle and R2 run manifest
by making the in-memory kernel describe every provider's final execution state
truthfully, block consumers whose hard producers did not succeed, and finalize
effective hard-capability support only after authoritative validation.

R3 is an independent kernel prerequisite. It makes **no SQLite, schema,
migration, generation, run-manifest, or store-publication change**. It persists
no provider family, enables no warm store reuse, redesigns no general cache key
or dependency index, and adds no public CLI/config/SDK/JSON contract. Completing
R3 must leave R4-R6, Phase 65, and STORE-04/STORE-05/META-01/META-04 open.

</domain>

<decisions>
## Implementation Decisions

### Delivery Boundary
- **D-01:** Plan and implement R3 only. Treat `65-01-SUMMARY.md`,
  `65-02-SUMMARY.md`, and the clean combined `65-REVIEW.md` as locked R1/R2
  dependencies.
- **D-02:** Preserve the restart stop budgets: at most three implementation
  tasks, fifteen product/test files, and 2,500 handwritten added lines. R3 has
  zero durable schema families and zero persisted provider families.
- **D-03:** If correctness requires a repository-wide provider-key migration,
  whole dependency-index redesign, provider persistence, Go runtime/toolchain
  certification, cross-file semantic-ID repair, or public API expansion, stop
  that path and route it to its owning prerequisite or later slice.
- **D-04:** R3 completion must not mark Phase 65 or any mapped Phase 65
  requirement complete. The plan and summary use `requirements: []`, record
  contributions separately, and leave R4-R6 as the next work.

### Closed Provider Outcome Contract
- **D-05:** Every static `ProviderManifest` receives exactly one sealed final
  outcome in deterministic manifest order for every successfully established
  kernel run report. Providers omitted by the resolved plan are represented,
  not silently absent.
- **D-06:** The closed final status vocabulary is: `Succeeded`, `Failed`,
  `DependencyBlocked`, `Unsupported`, `SetupMissing`, and `PlannedAbsent`.
  These states must never be inferred from an optional digest, empty fact set,
  rendered diagnostic, or cache counter.
- **D-07:** `Succeeded` means the provider was scheduled and completed, its
  output identity is present and authenticated, and every authoritative
  validation applicable to that output passed. Provisional execution success
  may exist internally but cannot be exposed as final trust.
- **D-08:** Only `Succeeded` carries a reusable output identity. Non-success
  outcomes carry a closed typed reason/stage and, for dependency blocking, the
  exact sorted blocking provider IDs. Open status strings are not semantic
  truth.
- **D-09:** A fatal error before a kernel run is established, or an uncontrolled
  internal panic, returns the existing kernel error rather than fabricating a
  complete outcome report. R3 does not become a general panic/process
  containment project.
- **D-10:** Diagnostics, cache hit/miss/write counters, warnings, durations,
  timestamps, worker order, and other run telemetry remain outside provider
  semantic outcome and output identity. The existing open-string
  `ProviderOutputMeta::validation` shape is not a trusted persistence contract.

### Dependency Blocking and Planned Absence
- **D-11:** R3 models only actual provider-output dependencies consumed by the
  current kernel orchestration. `ProviderManifest.inputs` prose and the
  existing broad `DependencyIndex` are not treated as an authenticated provider
  DAG, and no durable dependency relation is introduced.
- **D-12:** A scheduled provider executes only when every hard producer it
  consumes has final/provisionally usable success. Otherwise it does not run
  and becomes `DependencyBlocked`; independent provider branches continue.
- **D-13:** Plan gating is evaluated before dependency blocking. A provider not
  requested by the resolved plan is `PlannedAbsent`. If a scheduled consumer
  requires such a producer, that is blocking/inconsistent state rather than
  permission to consume an empty universe.
- **D-14:** `Digest::absent` or any equivalent sentinel cannot stand in for
  provider failure, dependency blocking, unsupported setup, setup-missing, or
  planned absence. Explicit input availability remains a separate typed input
  contract.
- **D-15:** Do not invent implicit optional or degraded dependencies in R3.
  A provider that can soundly operate in a degraded mode needs an explicit,
  separately proven contract; until then conservative blocking wins.

### Authoritative Validation and Effective Capabilities
- **D-16:** Refactor authoritative fact validation to return one closed,
  deterministic structured report with issues and provider/fact-family
  ownership. Existing internal diagnostics are rendered from that report
  instead of being the only validation result.
- **D-17:** An authoritative validation issue downgrades every implicated
  provisional provider success to `Failed` at the validation stage. If a global
  issue cannot be attributed safely, downgrade all provisional successes for
  the run rather than certify a partial universe.
- **D-18:** A hard requested capability is effective only when its planning and
  setup state is supported and every provider in its required closure has a
  sealed `Succeeded` outcome. Any other final provider state revokes the
  capability before rule dispatch.
- **D-19:** Planning-time `Unsupported` and `SetupMissing` states remain
  authoritative lower bounds and cannot be upgraded merely because another
  provider ran successfully.
- **D-20:** Skip every affected rule before it can access partial facts and emit
  one deterministic existing-style `polint/capability` diagnostic per affected
  rule/capability. Unaffected rules and provider branches continue.
- **D-21:** Keep final execution-state and revocation machinery crate-private.
  Do not add a public `CapabilitySupportStatus` variant, SDK type, CLI field,
  inspect field, JSON schema, diagnostic code, or exit-code contract in R3.
- **D-22:** Seal provider outcomes and effective capability blockers before
  constructing `KernelOutput`/`KernelRunReport`. Store maintenance remains
  strictly afterward and cannot change provider trust, diagnostics, capability
  support, rule dispatch, or policy answers.

### Cold/Warm Parity and Verification
- **D-23:** Cold computation and warm layer-cache execution must have identical
  final provider statuses, output identities, dependency blockers, effective
  capabilities, validation/capability diagnostics, rule-dispatch decisions,
  policy diagnostics, ordering, and exit semantics. Only telemetry may differ.
- **D-24:** A cache hit is not success by itself. Cached payloads must pass their
  existing typed decode/validation boundary; invalid cache data is rejected and
  recomputed or becomes a real provider failure. Cache read/write warnings
  after valid in-memory computation remain telemetry and do not downgrade
  semantic success.
- **D-25:** Focused proof must cover the full closed outcome codec/matrix, one
  hard dependency chain plus an independent branch, planned absence, controlled
  execution failure, authoritative validation failure, blocker ordering,
  capability diagnostic/rule skipping, and one representative cold/warm cache
  pair.
- **D-26:** Preserve byte/exit parity for unaffected public behavior and extend
  the public-surface leak gate for any new private outcome/validation names.
  Semantic-store disabled/enabled status is irrelevant to R3 outcomes.
- **D-27:** No required individual test may exceed sixty seconds, ordinary
  correctness tests may not use process-global serialization, and R3 must not
  edit `.github/workflows/ci.yml`, raise timeouts, or absorb the deferred
  sub-five-minute CI redesign.

### the agent's Discretion
- Exact crate-private type/module names and whether the outcome tracker lives
  beside `provider.rs`, `incremental::run_report`, or an equivalently central
  kernel boundary.
- The narrow in-memory representation of actual provider dependency closures,
  provided it is explicit at orchestration sites, deterministic, and does not
  pretend to complete R4's durable dependency-edge contract.
- The exact structured validation issue taxonomy and provider/fact-family
  attribution helper, provided diagnostics and trust decisions derive from the
  same report.
- Finite cfg(test)-only provider/validation failure seams and the representative
  cached provider used for cold/warm proof, provided they add no public flag,
  environment-variable protocol, global lock, or production failure mode.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project, Phase, and Completed Slice Contract
- `.planning/PROJECT.md` — Defines the private-store product boundary,
  reliability posture, performance constraints, and strict public API
  discipline.
- `.planning/REQUIREMENTS.md` — Defines STORE-04, STORE-05, META-01, and
  META-04; R3 contributes prerequisite correctness without completing them.
- `.planning/ROADMAP.md` — Defines the broader Phase 65 outcome and the Phase
  66 dependency; this context deliberately limits the next delivery unit.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-CONTEXT.md` —
  Locks the cache-owned, typed, default-disabled SQLite boundary that R3 must
  not modify.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-01-SUMMARY.md`
  — Authoritative R1 generation-lifecycle result.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-02-SUMMARY.md`
  — Authoritative R2 canonical run-manifest result and explicit R3 handoff.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md`
  — Final clean R1/R2 review and invariants that R3 must preserve.

### R3 Readiness and Finding Authority
- `research/local-semantic-store/RESTART-PLAN.md` — Canonical R3 scope, exit
  proof, review policy, test strategy, and pull-request budgets.
- `research/local-semantic-store/IDENTITY-READINESS.md` — Records
  `ProviderOutputMeta`, provider execution outcome, effective capability state,
  and validation result as R3-blocked contracts.
- `research/local-semantic-store/REVIEW-FINDINGS-TRIAGE.md` — Owns WR-08,
  WR-10, WR-14, WR-15, WR-16, WR-21, WR-22, and WR-26 provider-trust
  regressions.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-LEARNINGS.md`
  — Locks typed provider outcomes, telemetry separation, conservative omission,
  provider-scoped dependencies, and bounded-review lessons.
- `.planning/forensics/report-20260719-phase-65-scope-collapse.md` — Mandatory
  stop/split and reviewability guardrails after the abandoned oversized
  implementation.

### Store, Visibility, and Deferred-CI Boundary
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — Defines
  validated complete-generation trust; R3 supplies a prerequisite without
  changing the store.
- `research/local-semantic-store/decisions/DECISIONS.md` — Keeps the provider
  DAG as computation truth and requires conservative non-reuse for uncertified
  inputs.
- `docs/API-VISIBILITY-PLAN.md` — Supported API boundary; provider outcomes,
  validation internals, raw identities, and store vocabulary remain private.
- `.github/workflows/ci.yml` — Existing workflow remains unchanged; CI-duration
  optimization is a separately deferred follow-up.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/provider.rs`: Static deterministic provider
  manifest order and closed provider metadata are the inventory anchor. Its
  descriptive `inputs` strings are not yet an authenticated dependency DAG.
- `crates/polint/src/analysis_kernel/incremental/stats.rs`: Existing
  `ProviderOutputMeta` and `CacheStats` show the current semantic/telemetry
  mixture and the open `validation` string that R3 must stop treating as trust.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs`: The private
  run-report seam can carry sealed outcomes without introducing a public
  contract.
- `crates/polint/src/analysis_kernel/mod.rs`: The explicit provider sequence,
  plan gates, output-digest handoffs, early capability-support construction,
  late validation call, and final store-maintenance boundary are the central
  orchestration points.
- `crates/polint/src/analysis_kernel/validation.rs`: Existing comprehensive
  validators and deterministic diagnostic ordering are reusable; the missing
  piece is a structured result/ownership boundary.
- `crates/polint/src/analysis_plan.rs`: Planning/setup support remains the
  pre-execution lower bound from which effective support is derived.
- `crates/polint/src/core/mod.rs`: Existing capability view, blocking check,
  rule isolation, and diagnostic behavior provide the dispatch seam, but
  runtime revocation should remain a private sidecar rather than widen the
  public status enum.

### Established Patterns
- Provider execution order is explicit and deterministic inside
  `AnalysisKernel::run`; provider outputs are collected into a crate-private
  `KernelRunReport`.
- Current optional output digests serve several meanings: real output,
  planned default, and controlled failure. R3 must replace that inference at
  the orchestration/trust boundary without persisting new rows.
- Module/symbol providers can refine planning-time support, but current support
  is finalized before most late providers and global validation execute.
- `validate_fact_metadata` already aggregates and deterministically sorts
  authoritative issues, but returns only rendered diagnostics.
- Rules are already skipped before execution when their support view contains
  a blocking state; R3 extends the private blocker source rather than allowing
  partial fact access.
- Store maintenance runs after validation and can remain behaviorally
  irrelevant to provider outcomes.

### Integration Points
- Introduce one private closed provider-outcome/reason contract and make
  run-report construction require one sealed row per manifest.
- Track actual hard producer outcomes alongside the explicit provider calls;
  remove failure-to-absent substitution at every consumed provider-output
  handoff in the bounded R3 scope.
- Make authoritative validation return structured issues, then seal provisional
  outcomes and derive effective capability blockers before `KernelOutput`.
- Pass the final private blocker set through runner dispatch while preserving
  existing public `RuleCtx`/SDK/CLI schemas and unaffected diagnostics.
- Add focused deterministic tests near the outcome tracker/kernel validation
  seams plus one outside-user parity/leak assertion; do not touch store schema
  or CI.

</code_context>

<specifics>
## Specific Ideas

- The decisive dependency case is: provider A fails; scheduled B depends on A;
  scheduled C depends on B; independent D succeeds; unrequested E is
  planned-absent. Final outcomes are failed, dependency-blocked,
  dependency-blocked, succeeded, and planned-absent in manifest order.
- The decisive validation case first records a provisional provider success,
  injects an owned authoritative validation issue, seals that provider as
  failed-at-validation, revokes the consuming hard capability, skips its rule,
  and preserves an unrelated rule.
- The decisive warm case runs the same representative provider cold and from a
  validated layer-cache hit. Semantic outcomes, output identities,
  capabilities, diagnostics, and policy results are byte-identical while cache
  counters differ.
- A cache write warning after valid computation is explicitly not a provider
  failure; an invalid cached payload is never a trusted success.

</specifics>

<deferred>
## Deferred Ideas

- **R4:** Persist one audited provider family's manifest, final outcome, output
  identity, and exact consumed-input edges with one invalidate/preserve pair.
- **R5:** Expand provider mirroring one proven family at a time.
- **R6:** Private enablement and one measured cold/warm store-reuse pair before
  any default promotion.
- Provider-specific cache-key/dependency readiness repairs, whole
  `DependencyIndex` persistence, `InputSnapshot` redesign, `FactMeta`,
  validation-event persistence, layer/query/summary keys, and store statistics.
- Cross-file semantic-ID/cache-schema and other product bugs tracked in GitHub
  issues #86-#91.
- Go toolchain/process/filesystem security work, optional/degraded provider
  semantics, general panic containment, and platform runtime redesign.
- Establishing a sub-five-minute required CI path and any branch-protection or
  ruleset administration.

</deferred>

---

*Phase: 65-generation-manifest-and-metadata-mirroring*
*Context gathered: 2026-07-29*
