---
phase: 28-private-semantic-mir-and-place-identity
plan: 05
subsystem: static-analysis
tags: [rust, semantic-mir, cache-key, validation, debug-json]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: "Private MIR body/place schemas and Go/TS lowering outputs from plans 28-02 through 28-04"
provides:
  - "Private polint.semantic_mir provider wired into the kernel after module topology and before metrics"
  - "Semantic MIR cache identity vocabulary with source, lifecycle, config, upstream, and absent extension/model/toolchain slots"
  - "Semantic MIR validation diagnostics and test-only debug JSON rows"
affects: [semantic-mir, analysis-kernel, incremental-cache, validation]

tech-stack:
  added: []
  patterns:
    - "Provider output digests are computed from normalized private MIR rows plus upstream lifecycle/config/provider inputs"
    - "Private semantic MIR diagnostics use polint/internal with family/stable_key/field/reason evidence"

key-files:
  created:
    - crates/polint/src/analysis/provider.rs
    - crates/polint/src/analysis/cache_key.rs
    - crates/polint/src/analysis/validate.rs
  modified:
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis/store.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "Semantic MIR remains private and crate-internal; no SDK, runner, CLI, or public JSON surface was promoted."
  - "Malformed unsupported semantic rows are stored and rejected by validation so diagnostics carry stable family/stable_key/field/reason evidence."
  - "Semantic MIR cache identity includes absent extension, model, and toolchain slots even before those inputs exist."

patterns-established:
  - "TDD red/green commits for private provider wiring, cache identity, and validation/debug surfaces"
  - "Test-only metadata debug JSON can expose semantic MIR facts without raw source, absolute paths, parser ASTs, temp roots, or wall-clock data"

requirements-completed: [SAE-SEM-03]

duration: 26min
completed: 2026-05-20
---

# Phase 28 Plan 05: Semantic MIR Provider Summary

**Private semantic MIR provider with deterministic cache identity, validation diagnostics, run-report rows, and test-only debug JSON.**

## Performance

- **Duration:** 26 min
- **Started:** 2026-05-20T08:26:33Z
- **Completed:** 2026-05-20T08:52:08Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments

- Added `polint.semantic_mir` as an internal provider between `polint.module_topology` and `polint.metrics`, with normalized MIR storage and output digests.
- Added `LayerKind::SemanticMir`, `semantic_mir_layer_key`, and provider parameter digests covering source, lifecycle, config, upstream syntax/symbol/topology, and absent future input slots.
- Added semantic MIR validation for duplicate stable keys, bad spans, missing owners, dangling refs, malformed projections, unsupported evidence gaps, and precision ceiling violations.
- Extended test-only metadata debug JSON with deterministic `mir` rows for bodies, operations, places, and unsupported semantics.

## Task Commits

Each task was committed atomically using TDD red/green commits:

1. **Task 1: Add semantic MIR provider derivation and kernel wiring**
   - `9e3a2b7` test: add failing semantic MIR provider wiring tests
   - `35c3f7b` feat: wire semantic MIR provider into kernel
2. **Task 2: Add semantic MIR cache identity vocabulary**
   - `1b681b6` test: add failing semantic MIR cache key tests
   - `f60bca7` feat: add semantic MIR layer cache identity
3. **Task 3: Validate MIR/place artifacts and expose test-only debug JSON**
   - `37e6a83` test: add failing semantic MIR validation debug tests
   - `497737e` feat: validate semantic MIR artifacts and debug rows

## Files Created/Modified

- `crates/polint/src/analysis/provider.rs` - Semantic MIR provider derivation, normalization, diagnostics, and output digest generation.
- `crates/polint/src/analysis/cache_key.rs` - Provider parameter digest vocabulary for semantic MIR.
- `crates/polint/src/analysis/validate.rs` - Semantic MIR validation pass and diagnostics.
- `crates/polint/src/analysis/store.rs` - Allows malformed unsupported rows to reach validation rather than failing storage.
- `crates/polint/src/analysis_kernel/provider.rs` - Adds `polint.semantic_mir` manifest.
- `crates/polint/src/analysis_kernel/mod.rs` - Runs semantic MIR provider in the kernel and records output rows.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Adds `SemanticMir` layer key support.
- `crates/polint/src/analysis_kernel/validation.rs` - Wires semantic MIR validation and adds focused tests.
- `crates/polint/src/analysis_kernel/debug.rs` - Adds test-only semantic MIR debug JSON rows.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Updates expected provider ordering.

## Decisions Made

- Kept all new semantic MIR provider, cache, validation, and debug APIs private to preserve the public API boundary.
- Used `polint/internal` validation diagnostics with `family`, `stable_key`, `field`, and `reason` evidence for malformed semantic MIR rows.
- Preserved unsupported-row storage so the validation layer, not store normalization, owns user-visible internal diagnostics.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated internal eval provider-order expectations**
- **Found during:** Task 1 provider order verification.
- **Issue:** Existing eval provider-order fixtures still expected `polint.metrics` immediately after `polint.module_topology`.
- **Fix:** Added `polint.semantic_mir` to the eval observed/provider-order expectations.
- **Files modified:** `crates/polint/src/eval/observed.rs`, `crates/polint/src/eval/fixtures.rs`, `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml`
- **Verification:** `cargo test -p polint --lib provider_order --locked`
- **Committed in:** `35c3f7b`

**2. [Rule 2 - Missing Critical] Let malformed unsupported rows reach validation**
- **Found during:** Task 3 validation implementation.
- **Issue:** Store normalization rejected incomplete unsupported rows before semantic MIR validation could emit diagnostics with required evidence labels.
- **Fix:** Removed the completeness rejection from store normalization while retaining dangling-reference checks; validation now reports incomplete unsupported rows deterministically.
- **Files modified:** `crates/polint/src/analysis/store.rs`, `crates/polint/src/analysis/validate.rs`
- **Verification:** `cargo test -p polint --lib analysis_kernel::validation::semantic_mir --locked`
- **Committed in:** `497737e`

**3. [Rule 3 - Blocking] Applied formatter-only cleanup to earlier plan files**
- **Found during:** Task 3 final verification.
- **Issue:** `cargo fmt --all` normalized formatting in provider/cache files touched by earlier tasks in this plan.
- **Fix:** Included formatter-only changes with the Task 3 GREEN commit.
- **Files modified:** `crates/polint/src/analysis/provider.rs`, `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/src/analysis/mod.rs`
- **Verification:** `cargo fmt --all -- --check`
- **Committed in:** `497737e`

**Total deviations:** 3 auto-fixed (Rule 2: 1, Rule 3: 2)
**Impact on plan:** All deviations were required to keep provider ordering, validation diagnostics, and formatting verification coherent.

## Issues Encountered

Task 3 initially referenced per-id semantic MIR lookup helpers that `AnalysisDb` does not expose. The implementation was adjusted to use existing internal slice accessors, preserving the narrow API surface.

## Known Stubs

None. Stub scan found only intentional format strings, test fixture `exclude = []`, and an existing placeholder test name unrelated to stubbed behavior.

## Threat Flags

None. The new trust-boundary surface was the planned private provider, cache identity, validation, and test-only debug surface from the plan threat model.

## Verification

- `cargo test -p polint --lib semantic_mir_provider --locked`
- `cargo test -p polint --lib provider_order --locked`
- `cargo test -p polint --lib kernel_run_report_semantic_mir_row_carries_output_digest --locked`
- `cargo test -p polint --lib semantic_mir_layer_key --locked`
- `cargo test -p polint --lib analysis_kernel::validation::semantic_mir --locked`
- `cargo test -p polint --lib analysis_kernel::debug::semantic_mir_debug_json --locked`
- `cargo fmt --all -- --check`

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Downstream phases can now consume private semantic MIR artifacts through the kernel with deterministic provider identity, validation, and debug visibility.

## Self-Check: PASSED

- Summary file exists: `.planning/phases/28-private-semantic-mir-and-place-identity/28-05-SUMMARY.md`
- Key files exist: provider, cache key, validation, kernel wiring, debug JSON, and provider-order fixture files were found.
- Task commits exist: `9e3a2b7`, `35c3f7b`, `1b681b6`, `f60bca7`, `37e6a83`, `497737e`

---
*Phase: 28-private-semantic-mir-and-place-identity*
*Completed: 2026-05-20*
