# Phase 65: Generation Manifest and Metadata Mirroring - Context

**Gathered:** 2026-07-28
**Status:** Ready for R2 planning; R1 is complete and the broader phase remains open

<domain>
## Phase Boundary

This planning pass covers only restart slice **R2: the private minimal audited
run manifest**. Build on R1's schema-v2 generation lifecycle so one pending
generation can receive one canonical run/workspace/config/source projection,
have that projection decoded and authenticated, and become the active complete
generation atomically.

R2 is one independently reviewable canonical payload. It does **not** persist
`InputSnapshot` v1 wholesale, repair or mirror provider outcomes, enable warm
reuse, or add the original all-at-once Phase 65 metadata model. Completing R2
must leave R3-R6, Phase 65, and STORE-04/STORE-05/META-01/META-04 open.

</domain>

<decisions>
## Implementation Decisions

### Delivery Boundary
- **D-01:** Plan and implement R2 only. Treat `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-01-SUMMARY.md` as the completed and locked R1 dependency.
- **D-02:** Preserve the restart split/stop budgets: at most three implementation tasks, fifteen product/test files, 2,500 handwritten added lines, one new durable schema family, and zero provider families in R2.
- **D-03:** If R2 requires provider trust, capability state, tool/environment certification, a general dependency index, or an `InputSnapshot` redesign, stop that path and route it to its owning later slice or prerequisite.
- **D-04:** R2 completion must not mark Phase 65 or any mapped Phase 65 requirement complete. The plan and summary must use `requirements: []`, record contributions separately, and leave R3-R6 as the next work.

### Canonical Manifest Projection
- **D-05:** Introduce one private, versioned `RunManifest`-equivalent projection rather than serializing or migrating `InputSnapshot` v1.
- **D-06:** The complete R2 semantic payload is limited to: manifest schema version; purpose-typed workspace/cache-owner identity; complete config digest with its explicit purpose; sorted source rows containing normalized repo-relative path, a closed language value, source-content digest, and byte size; authenticated source count; and a creation-independent run identity recomputed from those canonical fields.
- **D-07:** Rule, plan, provider, capability, lifecycle, tool invocation, model, extension, layer, query, summary, fact, validation-event, dependency-index, statistics, cache outcome, duration, timestamp, and mtime metadata are structurally absent from the R2 payload.
- **D-08:** The derived run digest is supporting evidence and a lookup aid, never the sole proof of equality. Activation and reuse matching compare the fully decoded canonical fields and recompute the identity from them.
- **D-09:** Use one explicit, versioned codec shared by construction, SQL writing, SQL reading, identity recomputation, and tests. Rust `Debug`, lossy ad hoc serialization, and opaque JSON are forbidden identity inputs.

### Workspace, Config, and Source Identity
- **D-10:** Bind a store manifest to one workspace using a purpose-typed opaque identity derived deterministically from the canonical normalized workspace root. Persist only the closed purpose and digest, never the raw absolute root, user name, temp path, or home path.
- **D-11:** The workspace identity is also the ownership discriminator when `POLINT_CACHE_DIR` points multiple workspaces at a shared cache location. A different workspace is an exact non-match; it is not permission to infer ownership, overwrite an active generation, or reuse its manifest.
- **D-12:** Preserve the existing complete `config_hash` value as the config input and pair it with a fixed purpose label. Do not contaminate provider keys or invent provider-scoped config identities in R2.
- **D-13:** Derive source ordering from normalized relative paths; reject duplicate or non-canonical paths and unknown stored language labels. Do not persist or trust ordinals.
- **D-14:** Empty source membership is valid. For non-empty membership, exact insert/delete/update/path/language/digest/size changes must change the canonical payload or fail validation; insertion order must not.

### Publication and Read Semantics
- **D-15:** Add one exact schema-v3 manifest family owned by the existing generation lifecycle. A pending generation has no readable manifest; every complete generation has exactly one authenticated manifest and its owned source rows.
- **D-16:** Publication must write the manifest projection, authenticate storage classes/relationships/counts/codecs, decode it, recompute its identity, transition the same reserved handle to complete, and rotate the singleton active pointer within one `BEGIN IMMEDIATE` transaction.
- **D-17:** Any failure before commit rolls all candidate manifest and source writes back, leaves the candidate pending and unreadable, and preserves the previous active complete generation through close/reopen.
- **D-18:** Read the selected complete handle, manifest header, and all owned source rows in one read transaction/snapshot. Recompute identity from typed decoded rows before returning any trusted match.
- **D-19:** The private reuse seam must distinguish no active manifest, exact match, semantic mismatch, and malformed/refused state. Only an exact canonical-field match is reusable; R2 does not read persisted provider facts or change analysis answers.
- **D-20:** Existing handle-only active reads must not bypass manifest authentication after schema v3. A missing, extra, cross-owned, malformed, or identity-mismatched manifest makes the selected generation unreadable.

### Migration and Recovery
- **D-21:** Migrate exact schema v2 to v3 only when it contains no generation or active-selection rows. A populated v2 store has no truthful manifest to reconstruct, so refuse without mutation through the existing private rebuild-needed outcome.
- **D-22:** Do not synthesize a legacy manifest, clear or rotate an old active pointer, delete prior rows, or support mixed manifestless and manifested complete generations.
- **D-23:** Fresh/v0/v1 stores may migrate through the existing ordered versions to an empty v3 store; exact current-v3 reopen is idempotent; future and malformed schemas remain typed private refusals without mutation.

### Verification and Stop Gates
- **D-24:** Use explicit relational header/source columns and strict closed labels. Preflight SQLite storage classes, row counts, per-field lengths, aggregate bytes, ownership, and declared child count before allocating or decoding attacker-controlled values.
- **D-25:** Focused tests must round-trip identical input through close/reopen; prove exact config/workspace/source changes become normal mismatches; tamper every parent scalar plus representative source insertion, deletion, and update cases; and require typed refusal rather than partial trust.
- **D-26:** Prove timestamps, creation time, mtimes, durations, cache counters/outcomes, worker order, and insertion order cannot affect the identity because they are absent from the constructor and durable schema.
- **D-27:** Repeat the publication failure matrix with a prior manifested active generation and seams around manifest write, source write, identity validation, completion, selection, and commit. The old active manifest must remain the sole readable truth after reopen.
- **D-28:** Preserve the default-disabled short circuit before path validation, workspace canonicalization, manifest construction, or store I/O. Add no public CLI, config, SDK, output, diagnostic, ordering, or exit-code contract.
- **D-29:** No required individual test may exceed 60 seconds, and ordinary correctness tests may not be globally serialized. The user explicitly deferred the sub-five-minute CI target; R2 must not redesign CI, raise timeouts, or weaken tests.

### the agent's Discretion
- Exact private Rust type/module names and the narrow match/result enum, provided the canonical projection lives at the analysis-kernel boundary and SQL remains inside `analysis_kernel::store`.
- Exact table/column names, constraints, indexes, and transactional write order within the single manifest schema family, provided pending/complete ownership and one-manifest-per-complete-generation invariants are authenticated.
- The lossless platform-aware encoding used to hash the normalized workspace root and the precise digest-builder helpers, provided raw absolute paths never persist and `Debug` formatting is not used.
- Concrete row-count and byte limits, provided they are explicit, tested before allocation, large-repository compatible, and do not silently truncate.
- Deterministic cfg(test)-only tamper and failure seams, provided they add no environment variable, feature, global lock, or production-visible failure mode.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and Phase Contract
- `.planning/PROJECT.md` — Defines polint's private-store product boundary, performance posture, reliability constraints, and public API discipline.
- `.planning/REQUIREMENTS.md` — Defines STORE-04, STORE-05, META-01, and META-04; R2 contributes to but does not complete them.
- `.planning/ROADMAP.md` — Defines the broader Phase 65 outcome and the Phase 66 dependency; this context deliberately limits the next delivery unit.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-CONTEXT.md` — Locks the cache-owned, default-disabled, typed private SQLite boundary inherited by R2.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-01-SUMMARY.md` — Authoritative R1 lifecycle result, schema-v2 behavior, review remediation, and incomplete-phase guard.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md` — Final clean R1 review and the exact lifecycle guarantees R2 must preserve.

### Restart and Identity Authority
- `research/local-semantic-store/RESTART-PLAN.md` — Canonical R2 scope, exit proof, PR budgets, and rejected all-at-once approaches.
- `research/local-semantic-store/IDENTITY-READINESS.md` — R0 allowlist and explicit prohibition on persisting `InputSnapshot` v1 or trusted provider metadata.
- `research/local-semantic-store/REVIEW-FINDINGS-TRIAGE.md` — Store-core regressions requiring typed decode, identity recomputation, authenticated child counts, bounded allocation, single-snapshot reads, and scalar validation.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-LEARNINGS.md` — Retained semantic/telemetry split, canonical-edge, readiness, and bounded-delivery lessons from the abandoned implementation.
- `.planning/forensics/report-20260719-phase-65-scope-collapse.md` — Mandatory scope and reviewability guardrails after the abandoned 85,000-line delivery.

### Store and Visibility Design
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — Private rebuildable-store contract, complete-generation publication protocol, invalidation posture, and path/privacy rules.
- `research/local-semantic-store/RECOMMENDED_IMPLEMENTATION.md` — Internal store placement, explicit-table preference, migration policy, and bounded restart sequencing.
- `research/local-semantic-store/decisions/DECISIONS.md` — Durable single-active-truth, SQLite, relational-handle, and conservative reuse decisions.
- `docs/API-VISIBILITY-PLAN.md` — Supported API boundary; store, SQL, raw identity, and provider internals remain crate-private.
- `.github/workflows/ci.yml` — Existing required workflow; the sub-five-minute optimization is a separate deferred follow-up, not R2 scope.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/store/generation.rs`: R1's opaque handle, same-handle publication, active-pointer validation, immediate transaction, and finite rollback seams are the publication primitive to extend.
- `crates/polint/src/analysis_kernel/store/migrations.rs`: Exact schema-v2 catalog/content validation and ordered transactional migration runner provide the schema-v3 boundary and populated-v2 refusal point.
- `crates/polint/src/analysis_kernel/store/connection.rs`: Existing writer/read-only wrappers, bounded busy policy, schema preflight, typed fixture snapshots, and same-connection callbacks keep manifest reads and writes isolated.
- `crates/polint/src/analysis_kernel/store/mod.rs`: `SemanticStore`, `StoreConfig`, typed recovery mapping, owned cache-path checks, and disabled-before-I/O guards remain the only facade.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs`: Existing sorted file rows and complete-config source are projection inputs, but mtime, rule/plan, provider, lifecycle, tool, model, and extension fields are explicit exclusion witnesses.
- `crates/polint/src/analysis_kernel/incremental/digest.rs`: Existing length-prefixed deterministic digest construction and closed digest kinds can support purpose-typed workspace/run wrappers; `debug_part` is forbidden for durable identity.
- `crates/polint/src/cache/keys.rs`: `config_hash` is the mutation-tested complete configuration digest R2 retains with an explicit purpose.

### Established Patterns
- SQL, table names, scalar row IDs, transactions, and `rusqlite` values do not cross `analysis_kernel::store`; callers exchange narrow typed values.
- Current-schema validation authenticates exact table/index/foreign-key/trigger shape and row invariants before trust; R2 extends this policy rather than adding opportunistic repair.
- Store maintenance runs after in-memory computation/validation and cannot affect diagnostics, capabilities, or policy answers while the store remains private and disabled.
- Durable identity uses deterministic normalized repo-relative paths and explicit closed labels. Telemetry remains reportable but separate from semantic identity.
- Tests use independent temporary stores and typed snapshots, avoiding process-global serialization and raw production connection exposure.

### Integration Points
- Add one private manifest identity/projection module under `analysis_kernel::incremental` or an equivalently central kernel boundary; do not define a store-only competing identity vocabulary.
- Extend the store migration and generation publication/read paths with the one manifest family; do not connect providers, layer cache, rules, SDK, runner, CLI output, or public docs.
- Adjust schema-relative test fixtures from v2 to v3 and add an exact populated-v2 no-mutation refusal fixture.
- Keep normal production activation unchanged. R2 may expose a private unwired exact-match seam for R4/R6, but must not turn semantic-store persistence or reuse on.

</code_context>

<specifics>
## Specific Ideas

- A canonical source row is conceptually `(normalized relative path, closed language, source-content digest purpose/value, byte size)`. Row order is derived from path and never persisted as trusted meaning.
- The decisive happy path is: reserve B; publish B with manifest M; close; reopen; read B and reconstruct exactly M; compare every canonical field and the recomputed run identity.
- The decisive rollback path is: A is active with manifest M1; reserve B; fail at each manifest/publication seam; close; reopen; A/M1 remains the sole readable active truth and B has no readable manifest.
- The decisive tamper path changes one stored header scalar or one source relationship/value while leaving the stored run digest untouched; typed reopen must recompute and refuse it.
- The decisive migration path preserves a populated schema-v2 database byte-for-byte/catalog-for-catalog on refusal because no truthful R2 manifest can be synthesized for its existing generation.

</specifics>

<deferred>
## Deferred Ideas

- **R3:** Closed provider execution outcomes, dependency blocking, planned absence, failure, and post-validation capability revocation without SQLite changes.
- **R4:** Persist one audited provider family, its manifest/outcome/output identity, and exact consumed-input edges with one invalidate/preserve mutation pair.
- **R5:** Expand provider mirroring one proven family at a time.
- **R6:** Private enablement and one measured cold/warm reuse pair before any default promotion.
- `InputSnapshot` v2 redesign or wholesale persistence; provider/capability, lifecycle/tool, layer/query/summary, `FactMeta`, validation-event, statistics, and whole dependency-index mirroring.
- Cross-file semantic-ID/cache-schema, syntax dependency, semantic-graph cache-input, Go RTA-root, and related work tracked separately in GitHub issues #86-#91.
- Establishing a sub-five-minute required CI path and any branch-protection/ruleset administration.

</deferred>

---

*Phase: 65-generation-manifest-and-metadata-mirroring*
*Context gathered: 2026-07-28*
