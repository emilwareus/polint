---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 01
subsystem: analysis-kernel
tags: [digests, identities, metadata, codecs, provider-output, determinism]

# Dependency graph
requires:
  - phase: 64-store-foundation-and-boundary-proof
    provides: Private semantic-store boundary and public no-leak guarantees
provides:
  - Purpose-separated workspace, config, run, generation, and semantic aggregate identities
  - Exhaustive canonical codecs for durable kernel enum labels
  - Run-ID-free stable fact metadata rows and deterministic provider output digests
affects: [phase-65-plans-02-19, generation-manifest, metadata-mirroring, semantic-store]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Opaque identities validate DigestKind before composing canonical digests"
    - "Durable fact identity projects owned semantic rows and excludes transient run handles"

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "ConfigIdentity accepts only the complete Config digest; provider-scoped settings retain a separate digest purpose"
  - "Stable fact conflicts are keyed by family plus stable key and reject any differing semantic metadata"
  - "Provider-output identity hashes explicit canonical row fields and payload_digest, never run IDs, Debug text, or payload bytes"

patterns-established:
  - "Every durable enum codec has an exhaustive label/parse_label pair with a typed unknown-label error"
  - "Order-insensitive identity inputs are sorted before hashing, with no alternate hash path"
  - "Workspace roots are normalized, hashed into an opaque identity, and discarded before serialization"

requirements-completed: [STORE-04, META-01, META-04]

# Metrics
duration: 37min
completed: 2026-07-12
---

# Phase 65 Plan 01: Canonical Identity and Metadata Foundation Summary

**Purpose-checked kernel identities, exhaustive stable codecs, and run-ID-free semantic rows now provide the canonical input vocabulary for generation and metadata mirroring.**

## Performance

- **Duration:** ~37 min
- **Started:** 2026-07-12T18:03:18Z
- **Completed:** 2026-07-12T18:39:58Z
- **Tasks:** 1
- **Files modified:** 10

## Accomplishments

- Added opaque crate-private `WorkspaceIdentity`, `ConfigIdentity`, `RunIdentity`, and `GenerationIdentity` values backed exclusively by purpose-separated `DigestKind` values and the existing canonical hash implementation.
- Added exhaustive label parsers with typed failures for digest, layer, precision, input-status, provider-manifest, fact-metadata, cache-policy, and language vocabulary; schema-bearing cache-policy parsing borrows its input.
- Added `StableFactMetaRow` projection that removes transient `FactRef::run_id`, fully sorts and deduplicates semantic rows, and rejects conflicting metadata for the same family/stable-key identity.
- Migrated provider output and eval fixture digests from opaque strings to explicit stable rows that include family, stable key, producer, layer, precision, confidence, validation, and `payload_digest`.
- Kept the entire addition crate-private and verified that no SDK, runner, CLI, config, generated-skill, or public prelude surface widened.

## Task Commits

1. **Task 1: Define canonical identities, codecs, and stable semantic rows** - `092aac10` (feat)

## Files Created/Modified

- `crates/polint/src/core/mod.rs` - Canonical crate-private `Language` labels and typed parsing.
- `crates/polint/src/analysis_kernel/mod.rs` - Curated private re-exports and stable-row provider output projection with controlled conflict fallback.
- `crates/polint/src/analysis_kernel/metadata.rs` - Exhaustive fact codecs, owned stable rows, deterministic deduplication, and conflict errors.
- `crates/polint/src/analysis_kernel/provider.rs` - Canonical provider, language-scope, cache-policy, and precision codecs including a borrowed cache-policy view.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated crate-private identity re-exports.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Purpose-separated digest kinds, opaque identity constructors, path normalization, and permutation/wrong-purpose tests.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Canonical layer/precision codecs and the single precision-ceiling conversion.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Canonical input-status codec and complete provider-manifest digest purpose.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Explicit stable-row provider output hashing without transient identifiers or compatibility overloads.
- `crates/polint/src/eval/performance.rs` - Complete stable-row fixtures and semantic/permutation digest assertions.

## Decisions Made

- Preserved existing digest-kind ordering by appending the new durable aggregate purposes, while making the new labels explicit and exhaustively parsed.
- Included provider version and kind in the provider-manifest digest alongside scope, cache policy, precision, schemas, inputs, and outputs so run identity can consume a complete manifest aggregate.
- Projected stable rows globally before provider filtering. This makes duplicate family/stable-key conflicts visible even when conflicting rows name different producers, then turns the conflict into a controlled unsupported provider-output digest instead of a panic.
- Kept `payload_digest` as required semantic metadata while deliberately excluding payload contents, source bodies, AST/MIR/CFG blobs, and graph adjacency.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - AGENTS compliance] Replaced delivery-history comments with enduring kernel invariants**

- **Found during:** Task 1 metadata codec implementation.
- **Issue:** Existing comments and lint reasons in the touched fact-family section referenced roadmap identifiers and future plan chronology, which the repository's shipped-code comment policy forbids.
- **Fix:** Reworded them to describe durable solver and reserved-vocabulary behavior only.
- **Files modified:** `crates/polint/src/analysis_kernel/metadata.rs`
- **Verification:** Added-line scan found no phase, plan, milestone, or plan-ID references in shipped Rust code; formatting and workspace Clippy gates passed.
- **Committed in:** `092aac10`

---

**Total deviations:** 1 auto-fixed (AGENTS policy compliance)
**Impact on plan:** Comment-only cleanup in an already modified file; no product or API scope changed.

## Issues Encountered

- The first strict Clippy pass identified a large error value, a manual boolean `filter_map`, and a redundant test clone. The error stores conflicting rows behind boxes, the iterator was simplified, and the clone was removed; the full workspace Clippy rerun passed with `-D warnings`.

## User Setup Required

None - all changes are private kernel vocabulary with no external service, configuration, or migration action.

## Verification

- Plan-focused identity, metadata, run-report, language, and eval filters: 35 passed.
- Additional provider, cache-policy, layer/precision, and input-status codec filters: 6 passed.
- Analysis-kernel regression filter: 32 passed, 1 intentional synthetic benchmark ignored.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets --all-features --locked -- -D warnings`: passed across the workspace and external example rule crates.
- Acceptance scans: no transient `run_id` contribution in provider identity, no string-summary compatibility overload or synthetic `format!("provider=...")` input, no new payload/body/blob fields, no delivery-history comments, and no public visibility widening.
- Threat review: normalized workspace paths are irreversibly reduced to an opaque digest and discarded; stable rows retain only payload digests, with no new network, authentication, SQL, or file-write surface.

## Next Phase Readiness

- Plans 65-02 onward can project generation manifests and relational metadata using canonical identities and symmetric codecs without inventing persistence-specific hashes or labels.
- Stable fact/provider rows are deterministic under insertion and run-ID permutations, and semantic conflicts have an explicit controlled rejection path.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All ten planned source files and this summary exist, task commit `092aac10` is present in history, all plan-focused and codec tests pass, formatting and all-feature compilation are clean, and the strict all-target/all-feature workspace Clippy gate completes with zero warnings.
