---
phase: 22-internal-evaluation-harness-mvp
plan: "04"
subsystem: evaluation-harness
tags: [rust, eval, fixtures, provenance, cache, deterministic-json, internal-api]

requires:
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Deterministic metadata debug JSON with producer_id rows for current fact families
  - phase: 22-internal-evaluation-harness-mvp
    provides: Native fixture runner, observed kernel items, matcher, metrics, and deterministic report hashing from Plans 22-01 through 22-03
provides:
  - Native provenance fixture asserting Phase 21 SourceFile and Import metadata through producer_id
  - Native current cache determinism fixture deriving an observed invariant from cold, warm, and no-cache output equality
  - Strict fixture manifest parsing for expected fact producer fields
affects: [22-05-extension-fixtures, 22-06-fixture-coverage, phase-23-cache-snapshots, evaluation-harness]

tech-stack:
  added: []
  patterns:
    - native provenance fixtures use partial stable-key matching for content-hash-bearing metadata keys while still checking producer_id, precision, and status
    - current cache determinism is represented as an observed invariant only after normalized JSON and output_hash equality are proven across three cache states
    - fixture manifests reject unknown expected item fields so producer_id remains the only accepted provenance producer field

key-files:
  created:
    - tests/eval-fixtures/provenance/metadata/expected.polint-eval.toml
    - tests/eval-fixtures/provenance/metadata/repo/.polint.toml
    - tests/eval-fixtures/provenance/metadata/repo/src/app.ts
    - tests/eval-fixtures/cache/current-determinism/expected.polint-eval.toml
    - tests/eval-fixtures/cache/current-determinism/repo/.polint.toml
    - tests/eval-fixtures/cache/current-determinism/repo/component.tsx
    - tests/eval-fixtures/cache/current-determinism/repo/payment.go
    - .planning/phases/22-internal-evaluation-harness-mvp/22-04-SUMMARY.md
  modified:
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/model.rs

key-decisions:
  - "Keep provenance and cache fixtures crate-private/test-facing under eval with no public CLI, SDK, runner, or crate-root surface."
  - "Use producer_id, precision, and status as expected fact match constraints when the manifest specifies them."
  - "Derive cache.current_determinism only after cold, warm, and no-cache fixture runs have matching normalized JSON and output_hash values."
  - "Expose only test-only observed helpers for shared fixture repo execution; production behavior remains unchanged."

patterns-established:
  - "Expected fact mode = partial matches stable-key substrings while exact mode retains full stable-key equality."
  - "Cache-state fixture comparisons clear transient runtime durations before deterministic JSON comparison, matching output_hash semantics."

requirements-completed: [SAE-FND-03]

duration: 11 min
completed: 2026-05-17
---

# Phase 22 Plan 04: Provenance and Cache Fixture Summary

**Native provenance and cache determinism fixtures over real kernel output with strict producer metadata matching**

## Performance

- **Duration:** 11 min
- **Started:** 2026-05-17T17:18:35Z
- **Completed:** 2026-05-17T17:29:25Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments

- Added `tests/eval-fixtures/provenance/metadata`, asserting real SourceFile and TS Import metadata facts with `producer_id = "polint.source"` and `producer_id = "polint.ts.syntax"`.
- Added `tests/eval-fixtures/cache/current-determinism`, asserting `cache.current_determinism = "cold_warm_no_cache_equal"` and a `5000ms` runtime budget without adding future typed cache semantics.
- Tightened expected fact matching so provenance fields specified by manifests are enforced, and added partial stable-key matching for content-hash-bearing metadata rows.
- Added a cache fixture runner path that uses one copied repo across cold, warm, and disabled-cache kernel runs before emitting the cache determinism invariant.

## Task Commits

Each TDD step was committed atomically:

1. **Task 1 RED: Add failing provenance fixture coverage** - `335c99f` (test)
2. **Task 1 GREEN: Implement provenance fixture matching** - `2099b21` (feat)
3. **Task 2 RED: Add failing cache determinism fixture** - `66786b5` (test)
4. **Task 2 GREEN: Derive cache determinism invariant** - `594e228` (feat)
5. **Formatting: Format eval fixture changes** - `2e754f6` (style)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/eval/fixtures.rs` - Provenance/cache fixture tests, shared evaluation-run builder, cache determinism comparison, and invariant derivation.
- `crates/polint/src/eval/observed.rs` - Test-only helper path for running a copied fixture repo with cache enabled or disabled.
- `crates/polint/src/eval/matcher.rs` - Expected fact matching now checks partial stable keys plus specified producer, precision, and status fields.
- `crates/polint/src/eval/model.rs` - Fixture-related serde structs reject unknown fields.
- `tests/eval-fixtures/provenance/metadata/*` - Native provenance fixture repo and expected manifest.
- `tests/eval-fixtures/cache/current-determinism/*` - Native cache determinism fixture repo and expected manifest.

## Decisions Made

- Kept all new fixture support internal/test-facing; no public eval command or SDK surface was introduced.
- Used `producer_id` as the only accepted provenance producer field in expected fact manifests and made unknown manifest fields a load error.
- Compared normalized cache fixture outputs with transient runtime durations removed, preserving pass/fail runtime budget semantics without making deterministic comparisons machine-speed-dependent.
- Reused the existing current cache behavior only; the fixture does not introduce typed cache-key vocabulary or persistent cache behavior.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Enforced producer_id manifest fields**

- **Found during:** Task 1
- **Issue:** Expected fact matching ignored `producer_id`, precision, and status, and fixture manifests accepted unknown fields such as an alternate producer field name.
- **Fix:** Added strict serde field checks and made fact matching honor specified provenance fields, with partial stable-key support for metadata rows containing content hashes.
- **Files modified:** `crates/polint/src/eval/model.rs`, `crates/polint/src/eval/matcher.rs`, `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --lib eval_provenance --locked`
- **Committed in:** `2099b21`

**2. [Rule 3 - Blocking] Added shared-repo observed fixture execution**

- **Found during:** Task 2
- **Issue:** The existing native observer always copied fixtures into a fresh temp repo, so it could not prove cold, warm, and disabled-cache behavior over the same repo state.
- **Fix:** Added test-only helpers to copy a fixture once and run kernel observation against that repo with cache enabled or disabled.
- **Files modified:** `crates/polint/src/eval/observed.rs`, `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --lib eval_cache_current_determinism --locked`
- **Committed in:** `594e228`

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 blocking)
**Impact on plan:** Both fixes were required to satisfy the threat model and cache determinism proof without adding public API or future cache semantics.

## Issues Encountered

- `cargo fmt --all -- --check` found rustfmt-only changes after Task 2. Fixed with `cargo fmt --all` and committed as `2e754f6`.

## Verification

- `cargo test -p polint --lib eval_provenance_fixture_passes --locked`
- `cargo test -p polint --lib eval_provenance --locked`
- `cargo test -p polint --lib eval_cache_current_determinism_fixture_passes --locked`
- `cargo test -p polint --lib eval_cache_current_determinism --locked`
- `rg -n "eval_provenance_fixture_passes" crates/polint/src/eval/fixtures.rs`
- `rg -n "producer_id|polint.source|polint.ts.syntax|relative_path" tests/eval-fixtures/provenance/metadata/expected.polint-eval.toml`
- `rg -n "eval_cache_current_determinism_fixture_passes" crates/polint/src/eval/fixtures.rs`
- `rg -n "cache.current_determinism|cold_warm_no_cache_equal|max_runtime_ms = 5000" tests/eval-fixtures/cache/current-determinism/expected.polint-eval.toml crates/polint/src/eval/fixtures.rs`
- `rg -n "InputSnapshot|LayerKey|QueryKey|SummaryKey|DiagnosticKey|provider output metadata|layer cache|hit/miss" crates/polint/src/eval tests/eval-fixtures/cache/current-determinism` - no matches
- `cargo test -p polint --lib eval --locked`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None. Stub scan hits were intentional fixture syntax: `exclude = []` in test TOML and `return user != ""` in the cache fixture Go sample.

## Threat Flags

None. The new file-system activity is test-only native fixture execution, stays under the existing crate-private eval harness, copies fixture repos into temp directories, and reuses the existing symlink rejection and relative path normalization.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 22-05 can build synthetic extension rejection and delta fixtures on top of strict expected fact matching, real provenance fixture evidence, and current cache determinism invariant derivation.

## Self-Check: PASSED

- Found summary file: `.planning/phases/22-internal-evaluation-harness-mvp/22-04-SUMMARY.md`
- Found created fixture files under `tests/eval-fixtures/provenance/metadata` and `tests/eval-fixtures/cache/current-determinism`.
- Found task commits: `335c99f`, `2099b21`, `66786b5`, `594e228`, and `2e754f6`.

---
*Phase: 22-internal-evaluation-harness-mvp*
*Completed: 2026-05-17*
