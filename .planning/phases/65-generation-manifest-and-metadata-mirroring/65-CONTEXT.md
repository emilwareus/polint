# Phase 65: Generation Manifest and Metadata Mirroring - Context

**Gathered:** 2026-08-03
**Status:** Ready for the first R5 increment; R1-R4 are accepted and the broader phase remains open

<domain>
## Phase Boundary

This planning pass covers one bounded increment of restart slice **R5: audit
and mirror `polint.go.syntax` metadata end to end**. It builds on the accepted
generation lifecycle, run manifest, sealed provider outcomes, and
`polint.metrics` mirror by:

1. repairing the Go syntax provider's exact input, dependency-edge, output,
   and cache-warning contracts;
2. persisting its exact static manifest, sealed outcome, success-only output
   identity, and exact Go-source/parser dependency projection;
3. publishing and reopening that metadata beside the existing metrics mirror
   only as part of one authenticated complete generation; and
4. proving that consumed Go syntax changes invalidate, unrelated changes
   preserve an exact match, and durable tampering fails closed.

This increment persists **Go syntax metadata, not syntax facts or source
text**. `polint.go.syntax` means the in-process tree-sitter parser provider,
not `polint.go.semantic` or any external Go command. It adds no TypeScript
provider, generic all-provider schema, normal-run publication, semantic-store
fact restoration, public CLI/config/SDK/output contract, or CI redesign.
Completing it leaves further R5 increments, R6, Phase 65, STORE-04, STORE-05,
META-01, and META-04 open.

</domain>

<decisions>
## Implementation Decisions

### Delivery Boundary
- **D-01:** Plan and implement only the first R5 increment. Treat Plans 01-05,
  the accepted R1-R4 summaries, the clean R4 review, and the passed R4
  verification as locked dependencies; do not reopen their behavior except for
  a narrowly required compatibility repair.
- **D-02:** Keep the restart stop budgets: at most three implementation tasks,
  fifteen product/test files, 2,500 handwritten added lines, one new durable
  schema family, and one newly mirrored provider family.
- **D-03:** The only new provider in scope is `polint.go.syntax`. If honest
  certification requires changing `polint.ts.syntax`, mirroring
  `polint.source`, persisting facts, redesigning the generic dependency index,
  or certifying an external Go runtime/environment, stop and split.
- **D-04:** Plans and summaries use `requirements: []`, record contributions
  separately, and do not mark Phase 65 or any mapped requirement complete.
- **D-05:** The user's sub-five-minute CI follow-up remains deferred. Do not
  edit `.github/workflows/ci.yml`, alter hosted-CI timeouts, or add required
  benchmark/platform work to this increment.

### Next Provider Selection
- **D-06:** Mirror `polint.go.syntax` next. It is the smallest unmirrored
  in-process parser provider, has one language and an existing layer-cache
  seam, and gives a bounded way to resolve the retained syntax-dependency
  finding before broader provider expansion.
- **D-07:** Do not combine Go and TypeScript syntax in this increment. The Go
  implementation proves the syntax-provider contract first; a later R5
  increment may apply an independently audited equivalent to
  `polint.ts.syntax`.
- **D-08:** Keep `polint.go.semantic` completely out of scope. No `go` binary,
  module/workspace loading, package patterns, build tags, environment,
  toolchain selection, process containment, or semantic frontend output may be
  certified by this work.
- **D-09:** Do not choose `polint.source`: it remains `NoCache` and its
  discovery/config/filesystem boundary is broader than this parser slice.
  Graph, solver, extension/model, summary, and query providers retain their
  own readiness gates and open correctness issues.
- **D-10:** GitHub issue #89 remains the tracking umbrella for both syntax
  adapters. This increment owns only the Go half; do not claim the issue's
  TypeScript acceptance criteria are complete.

### Canonical Go Syntax Inputs
- **D-11:** Define one canonical Go syntax input projection from the complete
  normalized Go source membership. Each row contains the repository-relative
  path, the closed Go language value, and a purpose-tagged digest of the exact
  source bytes. Persist no source body or absolute path.
- **D-12:** Source membership is set-like and path-unique. Sort by canonical
  relative path, reject duplicate/non-normalized/unsupported paths, bind the
  exact row count, and validate every scalar and relationship before trust.
- **D-13:** Bind an explicit, versioned parser contract: provider ID/version,
  `go-facts-v2`, the Go syntax-layer payload schema, and a closed
  tree-sitter-go backend/grammar identity. Use stable labels or integers, not
  Rust `Debug`, cache paths, build paths, or ambient machine state.
- **D-14:** The in-process syntax parser does not consume the broad config
  digest, rule code/options, plan digest, TypeScript sources, Go module
  lifecycle, an external Go toolchain, extensions, models, timing, or cache
  disposition. Those values must not enter its semantic provider identity or
  durable dependency projection.
- **D-15:** If source audit finds another behavior-affecting parser input,
  represent it explicitly and mutation-test it. If that input belongs to
  another provider/runtime or cannot be canonically certified inside the
  budget, stop rather than declaring the output reusable.
- **D-16:** Narrow only the Go-specific syntax key/construction path. Preserve
  fixed generic key slots with explicit purpose-typed absence where needed;
  do not silently change TypeScript or redesign `LayerKey` repository-wide.

### Canonical Produced Value and Cache Semantics
- **D-17:** Derive `polint.go.syntax` output identity from one canonical
  produced syntax value, never from the layer key, cache file identity,
  `FactRef::run_id`, a manifest-only fact summary, or cache-hit status.
- **D-18:** The produced value covers every declared Go syntax output actually
  emitted for each path—packages, functions, imports, Go tests, and branch
  obligations—plus deterministic parser diagnostics. Validate local IDs and
  relationships, normalize path ownership, retain meaningful multiplicity,
  and sort deterministically before hashing.
- **D-19:** A validated cold computation, validated warm layer-cache hit, and
  cache-disabled computation with identical semantic results must produce the
  same provider output identity and policy semantics. Cache telemetry alone
  may differ.
- **D-20:** Parser diagnostics and fatal per-file parse results are semantic
  output and remain reproducible across cold/warm paths. Cache read/write
  warnings are run-local operational diagnostics/telemetry: preserve their
  current user-visible behavior where applicable, but never serialize or hash
  them into syntax payloads or provider identity.
- **D-21:** Reuse the current typed syntax payload only if it passes exact
  canonicality, ordering, reference, and codec tests. Otherwise introduce the
  smallest Go-owned canonical projection; do not persist opaque JSON, Rust
  debug text, or full `AnalysisDb` state in the semantic store.
- **D-22:** The layer-cache manifest must carry the exact canonical Go syntax
  dependency-edge set derived from the same input projection as the key.
  Reads require exact source, destination, kind, shape, cardinality, and value
  equality; missing, extra, replaced, duplicated, or unrelated edges reject
  the hit.
- **D-23:** Any Go-specific per-file cache key cleanup needed for identity
  parity may be included only when it stays inside the file/task budget and
  changes no public behavior. A generic file-cache migration or TypeScript
  cache change triggers a split.

### Durable Go Syntax Mirror
- **D-24:** Add one private, explicitly versioned relational Go-syntax mirror
  family beside the accepted metrics family. It may share small closed codecs
  with R4, but must not turn R5 into a generic all-provider persistence
  framework.
- **D-25:** Persist the exact static `polint.go.syntax` manifest projection:
  provider ID/version, closed provider kind, ordered input/output members,
  Go language scope, existing-file-fact-cache policy/schema, canonical schema
  name/version members, and precision ceiling. Decode and compare every field
  against the current static manifest.
- **D-26:** Every generation published through the R5 facade has exactly one
  Go syntax outcome in addition to the already-required metrics projection.
  Missing, duplicate, or cross-generation provider rows are invalid.
- **D-27:** Persist the accepted R3 six-state outcome vocabulary and legal
  stage/reason shapes. `Succeeded` alone carries provider output identity plus
  canonical Go-source/parser dependencies; all non-success outcomes remain
  truthful history and are never reusable.
- **D-28:** Only `DependencyBlocked` may carry blockers, and its exact sorted
  unique blocker set must be authenticated against the Go syntax provider's
  actual hard dependency (`polint.source`). Other provider IDs are invalid.
- **D-29:** Store one provider-owned forward dependency projection and derive
  any lookup from it. Do not persist the generic `DependencyIndex`, redundant
  forward/reverse maps, stringly cache nodes, syntax facts, raw source, or
  unrelated provider edges.

### Publication, Migration, and Matching
- **D-30:** Extend the accepted immediate publication transaction so the run
  manifest, metrics mirror, and complete Go syntax projection are written,
  boundedly decoded, recomputed, and compared before the generation becomes
  complete and selected.
- **D-31:** A publication failure at any new write/readback seam rolls back all
  candidate child rows, leaves the reserved candidate pending, and preserves
  the previous selected complete generation across reopen.
- **D-32:** Migrate only an exact empty schema-v4 store to schema v5. A
  populated v4 store has no truthful historical Go syntax outcome and must be
  preserved without mutation through the existing rebuild/refusal path; do
  not synthesize absence or delete history.
- **D-33:** Fresh and exact empty older schemas may migrate transactionally
  through the established chain. Exact v5 reopen is idempotent; malformed
  current, future, colliding, or populated-v4 state remains preserved and
  refused.
- **D-34:** Active reads authenticate lifecycle, workspace ownership, run
  manifest, metrics projection, Go syntax static manifest/outcome/identity,
  blockers, and exact dependency rows from one read snapshot before returning
  a typed exact/miss/no-active/refusal result.
- **D-35:** Go syntax matching is narrower than full run equality: changing a
  Go source/path/parser contract must miss; changing only config/rules/plan,
  cache telemetry, or TypeScript files must preserve an exact Go syntax match
  when the canonical Go inputs/output remain unchanged.
- **D-36:** Do not wire publication, reads, or matching into normal
  `AnalysisKernel::run`. Normal runs still do not become durable and do not
  consume semantic-store provider output; R6 owns private enablement and
  measured reuse.

### Proof and Stop Gates
- **D-37:** The decisive real fixture uses a tiny Go repository and proves
  reserve → atomic publish → close → reopen → authenticated active read →
  exact Go syntax match while retaining the accepted metrics projection.
- **D-38:** The mutation matrix covers Go source content, membership, path,
  parser/schema/provider identity, and exact dependency edges as
  must-invalidate inputs. Config/rule/plan, TypeScript-only source changes,
  cache telemetry, and cache mode are must-preserve cases when produced Go
  syntax is unchanged.
- **D-39:** Round-trip at least one legal non-success Go syntax outcome with no
  identity/dependency rows and prove it cannot match. Tamper tests cover
  manifest members, outcome shape, blocker ownership, identity fields,
  dependency rows/counts/order, SQLite storage classes, catalog objects,
  foreign keys, and aggregate bounds.
- **D-40:** Apply the R4 untrusted-storage posture: exact cardinality, dense or
  derivable ordering, storage-class and byte preflight before Rust allocation,
  bounded aggregate decoding, shared writer/reader codecs, and recomputed
  witnesses rather than digest-only trust.
- **D-41:** Preserve disabled-store zero I/O, busy/future/corrupt refusal,
  complete-generation selection, public JSON/diagnostics/order/exit behavior,
  SDK/runner/CLI signatures, docs/examples, generated skill text, and private
  visibility. Extend negative leak coverage for new vocabulary.
- **D-42:** Required correctness tests use isolated local temp repos/stores,
  no network, sleeps, environment mutation, process-global serialization, or
  external Go command. No required individual test may exceed sixty seconds.
- **D-43:** Run focused projection/adapter/layer-cache/store tests, format,
  strict workspace Clippy, workspace check, and diff/scope audits. If any
  task/file/line/schema/provider budget is crossed, or the exit proof requires
  TypeScript, facts, normal-run reuse, or CI changes, stop and split.
- **D-44:** Acceptance means exactly one additional provider family can be
  written, reopened, exactly matched, and rejected after tampering. It does
  not mean all providers are mirrored, GitHub issue #89 is fully closed, each
  run is persisted, or stored syntax facts are reused.

### the agent's Discretion
- Private type/module/table/index names and the exact relational decomposition
  of the one Go-syntax mirror family.
- Whether a small closed manifest/outcome header codec is factored from the
  metrics mirror, provided the R4 behavior/schema remains exact and the work
  does not become a generic provider framework.
- Stable parser-contract labels, numeric bounds, and digest helper names,
  provided they are explicit, versioned, shared by key/edge/output/store
  paths, and mutation-tested.
- Finite cfg(test)-only failure/tamper seams and focused fixture organization.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and Phase Contracts
- `.planning/PROJECT.md` — Defines the private semantic-store product boundary,
  reliability/performance posture, and strict supported-API discipline.
- `.planning/REQUIREMENTS.md` — Defines STORE-04, STORE-05, META-01, and
  META-04; this increment contributes without completing them.
- `.planning/ROADMAP.md` — Defines the broader Phase 65 outcome and Phase 66
  boundary; this context deliberately limits one R5 delivery unit.
- `.planning/phases/64-store-foundation-and-boundary-proof/64-CONTEXT.md` —
  Locks the cache-owned, typed, default-disabled SQLite boundary.

### Accepted R1-R4 Contracts
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-01-SUMMARY.md`
  — Accepted crash-safe generation state machine.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-02-SUMMARY.md`
  — Accepted canonical run manifest and exact-match seam.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-03-SUMMARY.md`
  — Accepted sealed provider outcomes and dependency/capability closure.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-04-SUMMARY.md`
  — Accepted validation ownership and production cold/warm parity closure.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-05-SUMMARY.md`
  — Accepted R4 canonical metrics identity and schema-v4 provider mirror.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-05-REVIEW.md`
  — Clean independent R4 review after bounded trust-boundary remediation.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-05-VERIFICATION.md`
  — Passed R4-only verification; explicitly leaves R5/R6 and Phase 65 open.

### R5 Readiness and Scope Control
- `research/local-semantic-store/RESTART-PLAN.md` — Defines incremental R5
  expansion, hard PR budgets, review disposition, and R6 separation.
- `research/local-semantic-store/IDENTITY-READINESS.md` — Marks syntax
  dependency edges not ready and requires one syntax provider to be repaired
  and proven before mirroring.
- `research/local-semantic-store/REVIEW-FINDINGS-TRIAGE.md` — Assigns exact
  syntax dependency authentication and cache-warning separation to a focused
  prerequisite/provider slice while keeping Go runtime hardening separate.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-LEARNINGS.md`
  — Locks provider-scoped projections, mutation pairs, telemetry separation,
  conservative omission, and bounded review.
- `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-PATTERNS.md`
  — Records the relational, migration, transaction, and identity patterns this
  increment extends rather than duplicates.
- `.planning/forensics/report-20260719-phase-65-scope-collapse.md` — Mandatory
  stop/split and no-big-bang guardrails.
- `https://github.com/emilwareus/polint/issues/89` — Current umbrella issue for
  exact Go and TypeScript syntax-layer dependency manifests; this increment
  owns only the Go half.

### Durable Trust and Visibility
- `research/local-semantic-store/implementation/STORE-CONTRACT.md` — Defines
  validated complete-generation trust, rebuildability, and privacy.
- `research/local-semantic-store/decisions/DECISIONS.md` — Requires explicit
  readiness, one kernel identity vocabulary, and conservative non-reuse.
- `docs/API-VISIBILITY-PLAN.md` — Keeps provider/store vocabulary private.
- `.github/workflows/ci.yml` — Must remain unchanged under the user's explicit
  deferral of sub-five-minute CI work.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/go/adapter.rs`: Go-only source selection, tree-sitter
  parse path, per-file and layer payloads, cold/warm restore, output digest,
  and focused cache tests.
- `crates/polint/src/analysis_kernel/incremental/keys.rs`:
  `LayerKey::syntax_layer_key` and typed digest slots used by both syntax
  providers; the Go call site can be narrowed without changing TypeScript.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs`: Versioned
  layer manifest, payload digest verification, dependency edge/index helpers,
  invalidation planning, and corruption eviction.
- `crates/polint/src/analysis_kernel/provider.rs`: Exact static
  `polint.go.syntax` manifest: source-file input, five syntax outputs, Go scope,
  existing file-fact cache, `go-facts-v2`, and syntax precision.
- `crates/polint/src/analysis_kernel/outcome.rs`: Closed six-state outcomes,
  success-only identities, and the authenticated `polint.source` hard
  dependency for Go syntax.
- `crates/polint/src/analysis_kernel/store/{provider_mirror.rs,generation.rs,migrations.rs,mod.rs,tests.rs}`:
  Accepted metrics mirror, bounded readback, atomic publication, exact schema
  authentication, active-snapshot reads, matching, rollback, and tamper seams.

### Established Patterns
- The current Go syntax layer key hashes source rows plus broad config, an
  explicit absent lifecycle, a package-version tool label, and parser
  parameters rendered with Rust `Debug`; the parser itself reads Go source
  bytes/path/language but not config, rules, or plan.
- Both syntax adapters currently write an empty layer dependency list, and the
  generic reader requires non-empty dependencies only for module graph,
  symbol graph, and metrics. A current Go syntax manifest can therefore omit
  the inputs needed to authenticate reuse.
- Cache-disabled Go syntax returns no adapter output digest, so the kernel
  falls back to a fact-metadata summary containing transient run IDs; this is
  not a durable produced-value identity.
- Per-file cache-write warnings are appended to the result before the outer
  syntax payload is built, allowing operational cache state to alter payload
  identity. Outer layer-cache write warnings are already kept outside the
  payload.
- Schema v4 deliberately owns exactly one metrics projection per manifested
  complete generation. A second provider must preserve that projection and
  cannot fabricate metadata for populated historical stores.

### Integration Points
- Introduce or prove one canonical Go syntax input/output projection at the
  Go adapter/kernel boundary and use it for Go-only key construction,
  dependency edges, sealed provider identity, durable matching, and mutations.
- Make the Go syntax layer writer emit the exact expected edge set and make
  Go syntax reads compare it exactly before accepting a hit.
- Add one schema-v5 Go-syntax mirror family and extend the existing immediate
  publication/read snapshot beside the run manifest and metrics mirror.
- Add focused Go adapter, layer-cache, store migration/publication/match,
  tamper, parity, and public-surface tests without touching normal production
  store wiring or CI.

</code_context>

<specifics>
## Specific Ideas

- The decisive repository contains one or two small `.go` files plus an
  unrelated `.ts` file. It proves a Go edit/path/membership change misses while
  a TypeScript-only, config, rule, plan, or telemetry change preserves the Go
  provider match when the Go produced value is unchanged.
- Run the same Go syntax result cold, from a validated warm layer-cache hit,
  and with caching disabled. Compare sealed provider identity, parser
  diagnostics, restored facts, policy answers, ordering, and exit semantics;
  permit only cache telemetry/warnings to differ where intentionally injected.
- Delete, replace, duplicate, reorder, or add a Go syntax dependency edge in a
  current-schema layer manifest and require invalidation/recomputation rather
  than a hit.
- Publish a real sealed Go syntax projection beside the accepted metrics
  projection, close/reopen the store, and obtain one exact typed match. Inject
  failure after each new write/readback seam and prove the old active
  generation remains the only readable truth.

</specifics>

<deferred>
## Deferred Ideas

- A later R5 increment for `polint.ts.syntax`, including the TypeScript half of
  GitHub issue #89 and its source-type/parser-mode identity.
- Further R5 provider expansion for source, module/symbol/graph, Go semantic,
  extension/model, summary, and query providers only after separate readiness
  audits and open correctness prerequisites.
- R6 private normal-run publication, one measured cold/warm semantic-store
  reuse pair, and any decision about default enablement.
- Syntax/metric fact payload persistence and restoration, `FactMeta`,
  validation events, store statistics, full `InputSnapshot`, generic
  dependency indexes, and layer/query/summary persistence; these remain Phase
  66 or later concerns unless the roadmap is explicitly revised.
- GitHub issues #86, #90, and #91; external Go process/toolchain/environment
  hardening; general panic containment; exhaustive crash/scale/platform
  matrices; and branch-protection administration.
- The separately deferred sub-five-minute required CI path.

</deferred>

---

*Phase: 65-generation-manifest-and-metadata-mirroring*
*Context gathered: 2026-08-03*
