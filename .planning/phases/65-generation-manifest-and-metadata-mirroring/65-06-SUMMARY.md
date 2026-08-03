---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 06
subsystem: analysis-kernel
tags: [go-syntax-identity, semantic-store, provider-mirror, sqlite, tamper-refusal]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    provides: R1-R4 generation, manifest, outcome, and metrics-mirror contracts
provides:
  - Canonical Go source/parser inputs and six-family produced syntax identity
  - Present-language `string_literals` ownership and failure gating
  - Schema-v5 relational metadata mirror for `polint.go.syntax`
  - Atomic manifest-plus-metrics-plus-Go publication with authenticated reopen and private exact matching
  - First R5 Go-only invalidation, rollback, bounds, tamper, parity, and privacy proof
affects: [phase-65-r5-typescript, phase-65-r5-other-providers, phase-65-r6, semantic-store]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Canonical source and parser projections drive keys, exact dependency edges, durable metadata, and matching"
    - "Produced identities hash relationship-validated semantic rows rather than transient fact IDs or serialized payload bytes"
    - "Operational cache warnings stay run-local and never enter reusable syntax payloads or identities"

key-files:
  created:
    - crates/polint/src/analysis_kernel/go_syntax_projection.rs
    - crates/polint/src/analysis_kernel/store/go_syntax_mirror.rs
  modified:
    - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/store/generation.rs
    - crates/polint/src/analysis_kernel/store/migrations.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
    - crates/polint/src/go/adapter.rs
    - crates/polint/src/go/tests.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/tests/public_surface_leak.rs

key-decisions:
  - "The private Go manifest declares packages, functions, imports, go_tests, branch_obligations, and string_literals in exact order"
  - "Go source path/content digests and the closed parser contract are consumed; config, rules, plan-only state, TypeScript state, cache mode, telemetry, and warnings are excluded"
  - "Only successful Go outcomes carry identity, source rows, and one parser row; legal non-success remains exact but never reusable"
  - "Only exact empty schema v4 migrates to v5; populated v4 history is preserved and refused rather than assigned invented Go history"
  - "Normal AnalysisKernel execution remains maintenance-only; private publication/read/match seams are not wired into production reuse"

patterns-established:
  - "Present-language ownership: Go-only uses Go syntax, TS-family-only uses TS syntax, and mixed repositories require both in stable order"
  - "Exact dependency vectors are authenticated before cache reuse; missing, extra, replaced, duplicate, or reordered edges recompute"
  - "Go mirror source rows must exactly correspond to the Go subset of the retained run manifest"

requirements-completed: []

# Metrics
duration: 1h 25m
completed: 2026-08-03
---

# Phase 65 Plan 06: R5 Go Syntax Metadata Mirror Summary

**`polint.go.syntax` now has canonical Go-only identity and a private schema-v5 metadata mirror that can be atomically written beside metrics, reopened, exactly matched, and refused after dependency or SQLite tampering.**

This accepts only the first, Go-only R5 increment. The TypeScript syntax
increment, other readiness-gated R5 providers, R6 private enablement/measured
reuse, Phase 65, STORE-04, STORE-05, META-01, and META-04 remain open.

## Performance

- **Duration:** 1h 25m
- **Started:** 2026-08-03T13:51:33+02:00
- **Task implementation completed:** 2026-08-03T15:16:27+02:00
- **Tasks:** 3
- **Product/test files modified:** 13 of 13 allowed
- **Cumulative product/test delta from `4b925a088`:** 2,307 additions, 127 deletions
- **Task 3 delta:** 548 additions, 4 deletions
- **Task 3 `store/tests.rs` additions:** 333 of 350 allowed
- **New durable schema families:** 1 relational Go-syntax provider mirror family
- **Tables in that family:** 5
- **Newly mirrored providers:** 1 (`polint.go.syntax`)
- **Total retained mirrored providers:** 2 (`polint.metrics`, `polint.go.syntax`)

## Accomplishments

- Added canonical, path-unique Go source inputs and a closed parser contract
  covering provider version, `go-facts-v2`, payload schema, tree-sitter backend,
  and grammar.
- Added one relationship-validated produced value over packages, functions,
  imports, Go tests, branch obligations, string literals, and deterministic
  `parser/go` diagnostics. String-literal path, value, span, Go language, and
  multiplicity all participate in identity.
- Repaired the private Go provider manifest to include the already-public
  `StringLiterals<'_>` fact family without changing SDK, CLI, config, output,
  docs, examples, or generated skill contracts.
- Made `StringLiterals<'_>` runtime ownership follow present languages exactly:
  Go-only requires Go syntax, TS/JS-only requires TS syntax, and mixed inputs
  require both. Injected owner failures prove the corresponding gating polarity.
- Unified Go layer keys and dependency vectors on the canonical source/parser
  projection. Cold, warm, cache-disabled, corrupt-cache recovery, write-warning,
  warning-recovery, and later warm-hit paths preserve the same sealed identity.
- Added schema v5 with one five-table relational Go syntax family. Empty exact
  v4 migrates transactionally; populated v4 is preserved and refused.
- Extended the immediate generation transaction to publish the run manifest,
  retained metrics projection, and Go projection, with exact readback before
  completion and selection. All 35 publication failure seams preserve prior
  selected truth and leave no candidate child rows.
- Proved a real repository with two Go files and one unrelated TypeScript file
  survives reserve, publish, close, reopen, active reads, exact run-manifest
  matching, exact metrics equality, and `GoSyntaxMatch::Exact`.
- Proved real Go content, membership, and path changes miss, while independent
  config, rule, plan-only, cache-mode, and TypeScript byte/path/membership
  changes preserve exact Go matching.
- Added five isolated raw layer-edge adversaries and 76 table-driven SQLite
  tamper cases plus a direct relationship-valid aggregate overflow. Malformed
  status shapes, identities, blockers, sources, parser rows, storage classes,
  counts, ordinals, relationships, ownership, catalog declarations, FKs, and
  hostile sizes fail closed without becoming active or Exact.
- Kept source bodies and fact payloads out of SQLite, all new vocabulary
  crate-private, and `SemanticStore::maintain` as the sole production kernel
  store operation.

## Implementation Commits

The three planned tasks were committed atomically with the repository's strict
formatting and Clippy hook:

1. **Task 1: Canonicalize Go syntax identity** - `63cc093b` (`feat`)
2. **Task 2: Add schema-v5 Go syntax mirror and atomic publication** - `3bcb2a99` (`feat`)
3. **Task 3: Prove exact matching, invalidation, tamper refusal, and privacy** - `4a255b51` (`test`)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/go_syntax_projection.rs` - Canonical Go
  sources, parser contract, six-family produced output, diagnostic identity,
  durable projection, and relationship validation.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Raw duplicate
  dependency rejection before canonical repair.
- `crates/polint/src/analysis_kernel/mod.rs` - Present-language string-literal
  ownership/failure gating and fallible Go projection handling.
- `crates/polint/src/analysis_kernel/provider.rs` - Exact six-output private Go
  provider manifest including `string_literals`.
- `crates/polint/src/go/adapter.rs` - Canonical Go key/dependency construction,
  semantic payload validation, and separation of operational warnings.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Exact schema-v5
  catalog, empty-v4-only migration, current-row validation, and source/manifest
  relationship checks.
- `crates/polint/src/analysis_kernel/store/go_syntax_mirror.rs` - Bounded
  relational writer/reader, exact manifest/outcome/parser/source reconstruction,
  dense ordinals, witnesses, and relationship preflight.
- `crates/polint/src/analysis_kernel/store/generation.rs` - Atomic
  three-projection publication, Go readback seams, authenticated active read,
  and exact Go matching.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Private typed Go
  publication, active projection, and match facade.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Real success/non-success,
  preserve/invalidate matrices, 35 rollback seams, bounded aggregate proof, and
  76-case SQLite/catalog tamper suite.
- `crates/polint/src/go/tests.rs` - Isolated missing/extra/replaced/duplicate/
  reordered layer-edge repair and later verified-warm proof.
- `crates/polint/src/runner/mod.rs` - Cold/warm/disabled identity parity plus
  corrupt-cache and transient write-warning recovery.
- `crates/polint/tests/public_surface_leak.rs` - Private Go projection, parser,
  match, schema, table, and dependency vocabulary leak markers.

## Decisions Made

- Persisted only canonical repo-relative source metadata and parser metadata;
  source bodies, facts, payloads, cache statistics, full snapshots, dependency
  indexes, queries, and summaries remain absent.
- Replaced transient file/function IDs with path-owned semantic relationships
  before hashing. Meaningful multiplicity is retained and full tuples are
  sorted deterministically.
- Kept parser diagnostics semantic but rejected `internal/cache` warnings from
  canonical output so operational failures cannot poison later reuse.
- Required exact source/parser dependency cardinality and order at both layer
  and store boundaries. Digests corroborate exact decoded values rather than
  replacing them.
- Authenticated Go mirror sources against the exact Go subset of the same run
  manifest before decoding or trusting the projection.
- Preserved provider-scoped matching: workspace ownership and the complete Go
  projection determine Exact, while unrelated full-manifest changes do not
  create false misses.

## Deviations from Plan

No scope deviation. The implementation used exactly three task commits, all
thirteen declared product/test paths, 2,307 cumulative additions, one new
durable family, and one newly mirrored provider. It made no CI, state, roadmap,
requirements, public-surface, TypeScript-parser, Go-semantic/toolchain, fact-
persistence, or normal-run persistence/reuse change.

The test-only run-manifest aggregate ceiling was adjusted from 256 to 384 bytes
because the mandated three-source real fixture has 321 bytes of encoded row
metadata. The production 512 MiB bound is unchanged, and the hostile aggregate
fixture was enlarged so the same pre-allocation refusal remains covered.

During implementation review, three evidence gaps were closed before the Task
3 commit: Go mirror sources were related directly to run-manifest Go rows; the
duplicate layer-edge case was sorted so it exercised duplication independently
from reordering; and real Go content, membership, and rename cases replaced
synthetic-only source mutations. No separate remediation commit was needed.

## Verification

All required commands were run separately against the final implementation;
every focused target completed below sixty seconds:

- Go provider manifest: 1/1 passed; 0.24s wall.
- Present-language string-literal ownership/gating: 1/1 passed; 0.30s wall.
- Canonical Go projection: 5/5 passed; 0.35s wall.
- Go syntax layer, including five edge adversaries: 6/6 passed; 24.59s wall
  including compilation.
- Raw duplicate layer dependency guard: 1/1 passed; 0.41s wall.
- Closed outcomes: 6/6 passed; 0.65s wall.
- Schema migrations: 24/24 passed; 0.64s wall.
- Go schema/storage round trip: 1/1 passed; 1.01s wall.
- Go provider mirror exit suite: 5/5 passed; 36.57s wall including compilation.
- Generation lifecycle and 35 rollback seams: 13/13 passed; 44.92s wall.
- Cold/warm/cache-disabled/warning-recovery parity: 1/1 passed; 0.42s wall.
- Store-mode JSON/exit parity: 1/1 passed; 1.20s wall.
- Public-surface leak proof: 1/1 passed; 46.10s wall including compilation.
- Full semantic-store module: 48/48 passed; 5.88s wall.
- Public marker negative controls: 1/1 passed.
- `cargo fmt --all -- --check`: passed.
- Final strict workspace/all-target/all-feature Clippy: passed; 17.11s wall.
- Final workspace/all-target/all-feature check: passed; 7.99s wall.
- Final `make lint`: passed; 2.77s wall.
- Diff, exact allowed-file, Task 3/cumulative addition caps, protected-file,
  forbidden-persistence, private-visibility, one-family/one-provider, and
  maintenance-only wiring audits: passed.

Tests use isolated local temp repositories/stores and in-process tree-sitter
parsing. No network, external Go process, sleep, process environment mutation,
global serialization, or timeout increase was added.

## Gate Status

- **Executor cumulative bounded review:** CLEAN at `4a255b51`; no unresolved
  Critical, Warning, informational finding, or ASVS-L1 HIGH within the declared
  Go-only scope.
- **Independent cumulative review:** pending parent/orchestrator gate. This
  summary does not pre-certify that independent result.
- **Fresh R5-Go-only verifier:** pending parent/orchestrator gate. Verification
  must cover D-01 through D-44 only and must not certify the TypeScript half of
  issue #89, another R5 increment, R6, Phase 65, STORE-04, STORE-05, META-01,
  or META-04.

## User Setup Required

None. The mirror, publication, reading, and matching seams remain crate-private
and are not enabled for normal-run persistence or reuse.

## Next Phase Readiness

- Implement the separate TypeScript syntax R5 increment with its own canonical
  inputs/output, metadata mirror, invalidation matrix, tamper proof, and review.
- Consider other R5 providers only after their readiness gates and exact
  consumed-input contracts are satisfied.
- Keep R6 private until enablement can be paired with measured reuse, invalidation,
  recovery, and no-regression evidence.
- Keep Phase 65 and STORE-04, STORE-05, META-01, and META-04 open until those
  remaining increments are independently accepted.
