---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 05
scope: r4-only
verified: 2026-08-02T17:19:55Z
status: passed
score: "7/7 must-haves verified"
phase_65_complete: false
requirements_completed: []
decision_coverage:
  verified: 46
  total: 46
  gaps: []
security:
  standard: ASVS-L1
  high_risk_gaps: 0
verification_base: 013ff41c3b350e7918217f0e663c4c462b38ef94
verification_head: 86f20de109b5ce0a4296e61a4ae72d74c142c6d0
---

# Phase 65 Plan 05 (R4): Metrics Provider Mirror Verification Report

**Scope:** R4 only, against the locked R1-R3 base and decisions D-01 through
D-46

**Status:** passed

**Score:** 7/7 must-have truths verified; 46/46 locked decisions verified

R4 passes at the source, value, relational, transactional, migration, behavior,
privacy, quality, scope, and ASVS-L1 boundaries. A canonical `polint.metrics`
projection can be written, reopened, exactly validated and matched, and is
refused after tampering. Normal kernel execution still neither publishes nor
reuses durable provider state.

This report does **not** verify R5 or R6 and does **not** complete Phase 65.
STORE-04, STORE-05, META-01, and META-04 remain open. No state, roadmap,
requirements, source, test, branch, commit, or remote state was changed by this
verification; this report is the sole new artifact.

## Observable Truths

| # | R4 must-have truth | Status | Independent evidence |
|---:|---|---|---|
| 1 | One normalized, deterministic, multiplicity-aware source/function/output projection controls the metrics key, dependency edges, output identity, durable encoding/decoding, and matching; consumed inputs invalidate while exclusions preserve; cold/warm/disabled identity agrees | ✓ VERIFIED | `metrics_projection.rs` defines the shared typed source/function/output projection and validates exact output relationships before sealing. `metrics.rs` derives the key, dependency edges, cold output, and warm-payload validation from it; `LayerKey::metrics_layer_key` contains only its two combined input families plus typed absences. Projection, key, metrics cache, mutation-matrix, and production cold/warm/disabled tests all pass. |
| 2 | Schema v4 adds exactly one relational mirror family for only `polint.metrics`, with the exact static manifest, six legal outcomes, success-only identity/dependencies, and dependency-blocked-only actual hard blockers | ✓ VERIFIED | Migration v4 adds the five tightly owned `metrics_provider_*` tables and no opaque payload. The header fixes provider ID/kind/scope/cache policy/precision and six statuses; six exact member rows encode two inputs, three outputs, and one schema. `MetricsProviderProjection::from_durable_parts` re-closes the outcome, compares the static manifest, permits input rows only with success, and restricts blockers to the metrics hard-dependency inventory. Outcome and mirror suites pass. |
| 3 | Only an exact empty v3 store migrates to v4; populated v3, malformed/current, future, and colliding stores are preserved and refused; hostile cells/catalog data are bounded before allocation | ✓ VERIFIED | Version-3 preflight authenticates the exact manifest schema, proves all five provider-owned names absent, and proves all four existing lifecycle/manifest tables empty before the migration loop; the immediate transaction repeats that validation. Populated-v3 and future/current-malformed tests pass without logical mutation. Catalog SQL and provider header/child rows receive type, cardinality, scalar-length, row-count, and aggregate-byte preflights before String/Vec materialization. |
| 4 | Run manifest and provider mirror publish in the same immediate transaction, are read back and exactly compared before completion/selection, roll back at every injected seam, and reopen through one-snapshot typed Exact/Miss/NoActive/refusal semantics | ✓ VERIFIED | `generation::publish_transaction` writes both projections, reads and compares both, then completes and selects within one `with_immediate_transaction`. All publication failure points preserve the prior selection and leave only the deliberate pending reservation. Active manifest and provider reads occur through one read transaction; `MetricsMatch` distinguishes `Exact`, `SemanticMiss`, and `NoActive`, while malformed/future state returns typed errors. |
| 5 | A real sealed TypeScript success round-trips and matches exactly; every legal non-success is historical and non-reusable; consumed/excluded mutations and relational/catalog tampering fail with the required polarity | ✓ VERIFIED | The real TS kernel fixture survives reserve/publish/close/reopen/active read and returns `MetricsMatch::Exact`. Both legal `Failed` forms and `DependencyBlocked`, `Unsupported`, `SetupMissing`, and `PlannedAbsent` round-trip without identity/input rows and return `SemanticMiss`. The source/function mutation matrix misses every consumed change, preserves broad config/rule/cache-mode and unused function-field changes, and the table-driven header/member/outcome/blocker/identity/source/function/count/storage/catalog/index/trigger/FK tamper suite fails closed. |
| 6 | Store vocabulary remains private, supported public behavior is unchanged, and production `AnalysisKernel::run` performs only `SemanticStore::maintain` | ✓ VERIFIED | New projection and store modules/types are `pub(crate)` or narrower; SQL and `rusqlite` remain under `analysis_kernel::store`. The public leak gate scans SDK, runner, CLI, lib, README, facts docs, examples, generated skill, and JSON output. The production run-body audit found exactly one store operation: `store::SemanticStore::maintain(&store_config)`; reserve/publish/active/match calls occur only inside the private store facade/tests. Store-enabled/disabled/corrupt/future/invalid/busy modes retain byte-identical JSON and exit behavior. |
| 7 | R4 remains inside the declared diff/budget/security boundary; every focused command is below 60 seconds; strict quality gates and the clean review pass with no unresolved HIGH threat | ✓ VERIFIED | The base-to-head product/test diff is 12 allowed files and +2,281/−386, with three planned tasks, one five-table schema family, and one persisted provider family. Protected CI/planning/public paths are unchanged and forbidden persistence is absent. All 20 required commands pass separately; the slowest required command is 1.70 s and the expanded 43-test store suite is 1.89 s. Strict Clippy, workspace check, format, diff hygiene, and the clean zero-finding review pass. |

## Canonical Identity and Invalidation Proof

The shared projection in
`crates/polint/src/analysis_kernel/metrics_projection.rs` is the single
provider-owned semantic vocabulary:

- Each source row contains normalized repo-relative path, closed language,
  purpose-tagged source-content digest, byte count, line count, and non-empty
  line count.
- Each non-synthetic function row contains normalized source path, name,
  byte/line bounds, language, and cyclomatic complexity.
- Source membership is unique by canonical path. Function rows are sorted but
  not deduplicated, so semantically duplicate functions retain multiplicity.
- Canonical output authenticates exactly one file-metric row per source and
  exactly one function-metric plus complexity-metric row per projected
  function, including all source/function relationships and produced fields.
- Transient `FileId`/`FunctionId`, insertion order, cache file/policy result,
  broad config, rules/options, plan/setup, toolchain, models, extensions,
  telemetry, `calls`, `is_test`, and `is_exported` do not enter the projection.

`metrics.rs` obtains `CanonicalMetricsInputs` before either cache read or
computation. The same source/function digests build both the metrics-only layer
key and its exact forward dependency edges. Cold and cache-disabled computation
seal `CanonicalMetricsOutput::digest`; warm reuse first reconstructs and
relationship-validates that same output and compares it to the stored output
digest. The provider output is therefore the canonical produced value, not the
layer key or cache payload identity.

The focused evidence establishes both polarities:

- **Must invalidate:** source membership, content digest, language, byte/line
  semantics, function membership/multiplicity, name, span bounds, language, or
  complexity changes yield `SemanticMiss` and change the metrics key where
  applicable.
- **Must preserve:** input order, transient IDs, `calls`, `is_test`,
  `is_exported`, rule/config selection, the scheduling metric view, cache mode,
  and absent tool/model/extension state preserve the canonical projection and
  exact provider match when produced metrics are unchanged.

## Relational Mirror, Migration, and Publication Proof

Schema version 4 adds one relational family, implemented by exactly five
generation-owned tables:

1. `metrics_provider_mirror`
2. `metrics_provider_members`
3. `metrics_provider_blockers`
4. `metrics_provider_sources`
5. `metrics_provider_functions`

The family stores no metric fact payload, `FactMeta`, validation event,
telemetry/statistic, full `InputSnapshot`, generic dependency index,
layer/query/summary row, JSON, or opaque blob. Its static manifest comparison
authenticates provider ID/version, kind, ordered input/output/schema members,
language scope, cache policy, schema versions, and precision ceiling against
the current in-process `polint.metrics` manifest.

Outcome reconstruction goes through `ProviderOutcome::from_closed_parts`:

- `Succeeded` alone has output identity and canonical source/function inputs.
- `Failed` permits only execution/execution-failed or
  validation/validation-rejected.
- `DependencyBlocked` alone has blockers; they must be sorted, unique, known,
  and members of the actual `polint.metrics` hard-dependency set
  (`polint.go.syntax`, `polint.ts.syntax`).
- `Unsupported`, `SetupMissing`, and `PlannedAbsent` retain their exact closed
  setup/planning shapes.
- Every non-success has neither reusable identity nor dependency rows and can
  never return `Exact`.

The version-3 guard is evaluated before writer policy changes and again inside
the immediate migration transaction. An authenticated but populated v3 store
is returned through the existing invalid-schema/rebuild-needed path without
inventing historical provider outcomes. Empty v0/v1/v2 stores migrate through
the same ordered chain to v4; exact v4 reopen is idempotent; future and
malformed/current stores are preserved and refused.

Publication uses the established generation reservation and a single immediate
transaction. Workspace ownership and pending-candidate status are checked
before child writes. Manifest header/sources and all five provider sections are
then written; both typed projections are boundedly read back and compared
exactly before the generation can become complete or selected. The 23-point
failure-injection matrix proves rollback before/after each mutation/readback
seam, preserving the old complete selection or no active truth on a first
failed publication.

## Required Artifact and Link Verification

| Artifact / link | Status | Verification |
|---|---|---|
| `metrics_projection.rs` | ✓ VERIFIED | Canonical source/function/output projection, normalization, multiplicity, relationship validation, sealed identity, and hard-blocker validation are substantive and tested. |
| `incremental/keys.rs` -> canonical inputs | ✓ WIRED | Metrics-only key accepts canonical source/function digests and uses typed absence for config/lifecycle/tool/model/extension slots; no upstream layer digests remain. |
| `metrics.rs` -> key/edges/output/cache validation | ✓ WIRED | One projection feeds key, exact dependency edges, cold output identity, and validated warm reuse; any metric capability still derives all three metric families. |
| `outcome.rs` -> durable projection | ✓ WIRED | Closed six-state/status-stage-reason codecs and actual hard dependencies are revalidated on durable reconstruction. |
| `migrations.rs` | ✓ VERIFIED | Current version 4, exact five-table family, schema/catalog/FK authentication, empty-v3-only guard, and bounded catalog decode. |
| `provider_mirror.rs` | ✓ VERIFIED | Split relational writer, bounded preflight/decoder, dense ordinals, row-derived witness, exact manifest comparison, and success-only dependency reconstruction. |
| `generation.rs` -> manifest and provider mirror | ✓ WIRED | Same immediate transaction, exact readback before completion/selection, one-snapshot active read, and typed match/refusal. |
| `store/mod.rs` -> private facade | ✓ WIRED | Publication/read/match types and methods remain crate-private and disabled mode returns before path/SQLite work. |
| `AnalysisKernel::run` -> store | ✓ VERIFIED | Sole production operation is `SemanticStore::maintain`; no publication, active read, match, or reuse is wired. |
| `runner/mod.rs` and `public_surface_leak.rs` | ✓ VERIFIED | Production cold/warm/disabled policy parity and private-vocabulary non-leakage pass without a supported API/CLI change. |

## Locked Decision Coverage

| Decision | Status | Evidence |
|---|---|---|
| D-01 | ✓ | Verification is R4-only on the locked R1-R3 artifacts; it does not certify later slices. |
| D-02 | ✓ | Three tasks, 12 product/test files, 2,281 additions, one schema family, one provider family. |
| D-03 | ✓ | Only the metrics-specific identity/mirror path changed; no generic index, fact persistence, semantic-ID, Go certification, or public-API expansion. |
| D-04 | ✓ | Plan/summary/report retain empty requirement completion; Phase 65 and all mapped requirements remain open. |
| D-05 | ✓ | CI workflow, timeouts, required checks, and deferred sub-five-minute redesign are untouched. |
| D-06 | ✓ | Sole mirrored provider is deterministic, multi-language, toolchain-free `polint.metrics`. |
| D-07 | ✓ | `polint.source` is not mirrored or promoted as reusable state. |
| D-08 | ✓ | Go/TS syntax and Go semantic providers are not mirrored or migrated. |
| D-09 | ✓ | Any scheduled metric capability derives/authenticates file, function, and complexity metrics together. |
| D-10 | ✓ | Canonical produced-output identity is equal across cold, validated warm, and disabled-cache execution. |
| D-11 | ✓ | Output uses normalized semantic rows, deterministic order, no transient IDs/cache identity, and preserves duplicate semantic rows. |
| D-12 | ✓ | Complete source membership, language, content digest, and size/line semantics are authenticated. |
| D-13 | ✓ | All non-synthetic functions use the exact consumed tuple; calls/test/export flags and transient IDs are excluded. |
| D-14 | ✓ | Counts, bounds, ordering, multiplicity, relationships, and exact decoded fields are validated before trust. |
| D-15 | ✓ | Rules/options, broad config, plan/setup, tool/model/extension, telemetry/timing, and upstream cache identity are excluded. |
| D-16 | ✓ | Only the metrics key and metrics dependency-edge builder were narrowed; generic key design and other providers are unchanged. |
| D-17 | ✓ | Cache and durable paths share canonical helpers, while output identity remains the produced value rather than the LayerKey. |
| D-18 | ✓ | One private versioned relational family; no JSON/blob/debug persistence or SQL outside the store boundary. |
| D-19 | ✓ | Every static metrics manifest scalar/member/version is closed-decoded and exactly compared. |
| D-20 | ✓ | Every published generation owns exactly one metrics outcome row; missing/duplicate headers are invalid. |
| D-21 | ✓ | Exact six-state vocabulary; only success carries authenticated output identity. |
| D-22 | ✓ | Closed stage/reason matrix; only dependency-blocked has sorted unique actual hard blockers. |
| D-23 | ✓ | Source/function dependency rows exist only with succeeded reusable output; all non-success is non-reusable. |
| D-24 | ✓ | One canonical forward source/function projection is stored; no generic or independently duplicated dependency index. |
| D-25 | ✓ | Mirror/header/member/blocker/source/function rows are generation-owned by exact foreign keys and cannot cross-link/outlive it. |
| D-26 | ✓ | Manifest and complete metrics projection write/read/recompute/compare before selection in one immediate transaction. |
| D-27 | ✓ | Only selected complete, fully authenticated state is returned from one read snapshot; incomplete/refused states are unreadable. |
| D-28 | ✓ | Every provider-aware failure seam rolls back child rows, retains pending reservation, and preserves prior active truth. |
| D-29 | ✓ | Exact empty v3 alone migrates; populated v3 is preserved/refused without synthesized history. |
| D-30 | ✓ | Fresh/empty v0-v2 migrate to v4; exact v4 is idempotent; malformed, future, and owned-name collisions refuse. |
| D-31 | ✓ | Private `MetricsMatch` distinguishes Exact, SemanticMiss, and NoActive; typed errors represent malformed/refused state. |
| D-32 | ✓ | Production kernel remains maintenance-only and consumes no stored provider output. |
| D-33 | ✓ | Consumed source/function changes miss; broad config/rule/cache mode and unused function-field changes preserve. |
| D-34 | ✓ | Both declared input families and every named consumed/excluded field are covered by source inspection plus mutation/key tests. |
| D-35 | ✓ | Real sealed TS success and every legal non-success shape survive reopen with exact reusable/non-reusable polarity. |
| D-36 | ✓ | Static manifest, outcome, blocker, identity, dependency, cardinality, ownership, type, catalog/index/trigger/FK tampering fails closed. |
| D-37 | ✓ | Explicit row/scalar/aggregate limits and storage-class/count preflight precede attacker-sized decoding/allocation. |
| D-38 | ✓ | Ownership, singleton/complete selection, disabled early return, bounded contention, future refusal, rollback, and no-mutation guarantees pass. |
| D-39 | ✓ | Cold/warm/disabled semantic projections match; corrupt cache recomputes and write failures remain honest warnings/telemetry. |
| D-40 | ✓ | JSON/diagnostics/order/exit, SDK, runner signature, CLI/config, docs/examples, generated skill, and visibility remain stable. |
| D-41 | ✓ | Isolated, network-free focused commands all finish far below 60 seconds; no sleeps/global serialization/release benchmark added. |
| D-42 | ✓ | Focused codecs/projection/migration/publication/match/mutation/parity/leak targets plus fmt/Clippy/check/diff audits all pass. |
| D-43 | ✓ | No task/file/line/schema/provider budget is crossed and no split-triggering expansion was needed. |
| D-44 | ✓ | No metric facts, FactMeta, validation events, stats, full snapshot, layer/query/summary rows, or generic dependency index persisted. |
| D-45 | ✓ | No source/syntax/Go semantic/extension/model/summary/query/graph/solver/other provider mirror was added. |
| D-46 | ✓ | Acceptance is limited to metrics metadata write/reopen/exact-match/tamper refusal; no claim of normal-run persistence or reuse. |

Decision coverage is **46/46** with no gaps.

## Behavioral Verification

Every required command was run separately from the repository root at
`86f20de109b5ce0a4296e61a4ae72d74c142c6d0`. Times are wall-clock `real`
measurements from `/usr/bin/time -p`.

| # | Required command / focus | Result | Time |
|---:|---|---:|---:|
| 1 | `analysis_kernel::metrics_projection::tests` | 2 passed | 0.48 s |
| 2 | `metrics::tests` | 29 passed | 0.28 s |
| 3 | Metrics-only incremental key test | 1 passed | 0.22 s |
| 4 | `analysis_kernel::outcome::tests` | 6 passed | 0.11 s |
| 5 | `analysis_kernel::store::migrations::tests` | 22 passed | 0.14 s |
| 6 | Provider non-success storage round-trip | 1 passed | 0.43 s |
| 7 | Provider mirror/mutation/tamper suite | 6 passed | 1.19 s |
| 8 | Generation lifecycle/rollback suite | 13 passed | 0.87 s |
| 9 | Cold/warm/cache-disabled production projection | 1 passed | 0.14 s |
| 10 | Store-mode JSON/exit parity | 1 passed | 0.48 s |
| 11 | Supported public-surface store leak gate | 1 passed | 1.70 s |
| 12 | `cargo fmt --all -- --check` | passed | 1.30 s |
| 13 | Strict workspace/all-target/all-feature Clippy | passed | 0.38 s |
| 14 | Workspace/all-target/all-feature check | passed | 0.15 s |
| 15 | Base-to-HEAD `git diff --check` | passed | 0.03 s |
| 16 | Exact allowed-file subset audit | passed | 0.01 s |
| 17 | Addition and file-cap audit | passed | 0.03 s |
| 18 | Protected CI/planning/docs/examples/CLI/SDK audit | passed | 0.01 s |
| 19 | Forbidden persistence audit | passed | 0.02 s |
| 20 | Production normal-kernel wiring audit | passed; sole operation is `maintain` | 0.03 s |

Expanded independent coverage:

| Focus | Result | Time |
|---|---:|---:|
| Full `analysis_kernel::store::tests` suite | 43 passed | 1.89 s |

No required target approached the 60-second cap. No full-workspace test,
network/toolchain fixture, release benchmark, CI redesign, timeout change, or
cross-platform expansion was added to this R4 verification path.

## Diff, Budget, and Protected-Scope Audit

The exact verified range is
`013ff41c3b350e7918217f0e663c4c462b38ef94..86f20de109b5ce0a4296e61a4ae72d74c142c6d0`.

- Product/test delta: **12 files, +2,281/−386**.
- Allowed cap: at most 13 product/test files and 2,500 additions.
- Planned implementation tasks: **3**.
- New durable schema families: **1**, the five-table relational metrics mirror.
- Persisted provider families: **1**, `polint.metrics`.
- The 13th allowed path, `analysis_kernel/store/connection.rs`, is unchanged;
  the allowed-file audit correctly accepts a subset rather than requiring
  padding.
- `.github/workflows/ci.yml`, `.planning/STATE.md`,
  `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md`, `docs/`, `examples/`,
  `crates/polint/src/cli/`, and `crates/polint/src/sdk/` are unchanged.
- The current head differs from the clean reviewed product head `4b925a08`
  only in `65-05-REVIEW.md` and `65-05-SUMMARY.md`; there is no later product
  drift.
- The working tree was clean before this report was created.

## ASVS-L1 Security Verification

| Threat | Status | Independent disposition |
|---|---|---|
| T-65-05-01: cache-mode/partial payload forges produced output | ✓ CLOSED | Complete output relationship validation and cold/warm/disabled/corrupt/write-warning tests bind success to canonical produced values. |
| T-65-05-02: omitted consumed input or poisoned identity | ✓ CLOSED | One shared projection plus key/dependency/mutation tests proves exact consumed and excluded sets. |
| T-65-05-03: forged manifest/outcome/identity/blocker/dependency yields reusable success | ✓ CLOSED | Closed codecs, static-manifest equality, success-only identity/inputs, actual-hard-blocker checks, and relational tamper refusal prevent it. |
| T-65-05-04: hostile SQLite values cause unbounded allocation/partial trust | ✓ CLOSED | Type/count/length/aggregate/catalog preflights and explicit maxima precede decoding/materialization; oversized fixtures refuse. |
| T-65-05-05: partial publication becomes active or destroys prior truth | ✓ CLOSED | Same immediate transaction, exact readback before completion/selection, and all failure-seam reopen tests preserve prior truth. |
| T-65-05-06: populated v3 receives false history or is erased | ✓ CLOSED | Exact schema/emptiness preflight occurs before mutation and repeats transactionally; populated v3 is preserved/refused. |
| T-65-05-07: cross-generation/workspace/incomplete state is returned active | ✓ CLOSED | Generation FKs, workspace authentication, selected-complete join, exact child ownership/cardinality, and one-snapshot read close the route. |
| T-65-05-08: private seams enable normal reuse or public API expansion | ✓ CLOSED | Kernel has only maintenance wiring; private visibility, protected-diff audit, leak scan, and public parity tests pass. |

ASVS-L1 HIGH-risk gaps: **0**.

The cumulative code review is clean with zero Critical, Warning, or
informational findings. Its reviewed product head is `4b925a08`; the current
head adds review/summary documentation only. This verifier independently
inspected the full base-to-current-head source/test diff and reran the required
gates rather than relying on that review verdict.

## Final R4 Disposition

**Passed.** `polint.metrics` is the sole durable provider metadata family and
meets the R4 exit: atomic write, bounded authenticated reopen, exact private
match, honest non-success history, and fail-closed tamper refusal.

`phase_65_complete` remains `false`, `requirements_completed` remains empty,
and R5/R6 plus STORE-04, STORE-05, META-01, and META-04 remain explicitly open.
