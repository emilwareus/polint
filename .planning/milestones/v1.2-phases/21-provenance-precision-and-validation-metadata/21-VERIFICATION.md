---
phase: 21-provenance-precision-and-validation-metadata
verified: 2026-05-17T08:19:35Z
status: passed
score: "13/13 must-haves verified"
overrides_applied: 0
---

# Phase 21: Provenance, Precision, and Validation Metadata Verification Report

**Phase Goal:** Add shared internal metadata for fact origin, precision, confidence, validation, stable keys, and deterministic merge behavior.
**Verified:** 2026-05-17T08:19:35Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | AnalysisDb stores crate-private sidecar metadata for source and syntax fact families without adding fields to public fact structs. | VERIFIED | `AnalysisDb` owns `fact_meta: FactMetaStore` at `core/mod.rs:543`; metadata fields exist only in `analysis_kernel::metadata`; public fact structs still only expose existing symbol/reference stable keys and precision fields. |
| 2 | Each covered fact has a run-local `FactRef` and a separate deterministic stable key. | VERIFIED | `FactRef { family, run_id }` and `FactMeta.stable_key` are separate in `metadata.rs:80-105`; `stable_key_from_parts` sorts labels, normalizes slashes, and length-prefixes parts at `metadata.rs:271-285`. |
| 3 | Fresh parse and cached restore paths attach metadata to source, Go syntax, and TS/JS syntax facts. | VERIFIED | `push_*` insertion methods record metadata immediately; `restore_file_facts` routes cached packages/functions/imports/branches/tests/coverage/TS facts through the same push methods at `core/mod.rs:1280-1325`; targeted restore test passed. |
| 4 | Module graph, symbol graph, and metrics provider outputs have internal metadata after replace boundaries run. | VERIFIED | `replace_module_graph_facts`, `replace_symbol_graph_facts`, and `replace_metric_facts` call refresh methods that record metadata at `core/mod.rs:672-817`. |
| 5 | Existing symbol, definition, and reference stable keys are reused as metadata stable keys. | VERIFIED | `symbol_fact_metadata`, `definition_fact_metadata`, and `reference_fact_metadata` pass `fact.stable_key.clone()` into `fact_meta_from_stable_key` at `core/mod.rs:1682-1742`. |
| 6 | The database can report missing metadata for every current kernel-produced fact family. | VERIFIED | `missing_fact_metadata` scans files, source/syntax facts, derived graph facts, symbol/reference facts, metrics, TS facts, tests, branches, and coverage at `core/mod.rs:927-1015`; `analysis_kernel` tests cover all-family empty and removed-row reports. |
| 7 | Duplicate identical stable keys with identical payload digests collapse idempotently. | VERIFIED | `FactMetaStore::insert` returns `FactMetaInsert::Idempotent` when owner payloads match at `metadata.rs:197-205`; unit tests passed. |
| 8 | Duplicate stable keys with conflicting payload digests produce deterministic `polint/internal` diagnostics. | VERIFIED | Conflicts are stored in a `BTreeSet` at `metadata.rs:190-218`; validation renders `Fact metadata stable key conflict detected...` diagnostics with stable evidence at `validation.rs:78-91`; tests passed. |
| 9 | Kernel metadata validation checks stable-key uniqueness, referential integrity, span bounds, provider precision ceilings, deterministic ordering, and conflict diagnostics before rules run. | VERIFIED | `AnalysisKernel::run` calls `validate_fact_metadata(&db, Self::provider_manifests())` before returning `KernelOutput` at `analysis_kernel/mod.rs:77-83`; `validation.rs` covers missing metadata, conflicts, references, spans, precision ceilings, and diagnostic sorting. |
| 10 | Crate-private/test-facing debug JSON can show provenance for files, imports, symbols, and references. | VERIFIED | `debug.rs` serializes `files`, `imports`, `symbols`, and `references` rows with family/run/stable key/provider/precision/confidence/validation metadata at `debug.rs:12-253`; metadata debug tests passed. |
| 11 | Debug JSON is deterministic and excludes timestamps, absolute machine paths, nondeterministic map order, and transient memory details. | VERIFIED | Debug rows sort by relative path/span/name/stable key/run id; tests assert byte-identical serialization and absence of temp roots/timestamp/pointer-like strings at `debug.rs:534-564`. |
| 12 | Metadata remains internal: no SDK view, crate-root export, runner contract, or public CLI output is added. | VERIFIED | `lib.rs:17` keeps `pub(crate) mod analysis_kernel;`; `rg` found no metadata/debug helper exports in `lib.rs`, `sdk`, or `runner`; public-boundary tests passed. |
| 13 | Existing `polint check`, SDK fact views, examples, diagnostics rendering, ignore handling, and rule execution remain compatible. | VERIFIED | `kernel_metadata_preserves_public_check_behavior` runs an external temp-repo rule through `polint check --format json`, asserts deterministic output, no `polint/internal`, and no metadata-only public JSON keys; targeted CLI test passed. |

**Score:** 13/13 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/polint/src/analysis_kernel/metadata.rs` | Crate-private metadata vocabulary, store, stable keys, merge tracking, missing metadata model | VERIFIED | Exists, substantive; contains `FactFamily`, `FactRef`, `FactMeta`, `FactMetaStore`, `FactMetaInsert`, `MissingFactMeta`, deterministic stable-key helper, and tests. |
| `crates/polint/src/analysis_kernel/validation.rs` | Crate-private metadata validation and diagnostics | VERIFIED | Exists, substantive; validates missing rows, conflicts, refs, spans, precision ceilings, and deterministic diagnostic order. |
| `crates/polint/src/analysis_kernel/debug.rs` | Test-only deterministic metadata debug JSON | VERIFIED | Exists under `#![cfg(test)]`; serializes files/imports/symbols/references and has determinism/path-hygiene tests. |
| `crates/polint/src/analysis_kernel/mod.rs` | Module registration and kernel validation/debug hooks | VERIFIED | Registers `metadata`, `validation`, and cfg-test `debug`; calls validation before returning kernel output; exposes test-only helpers crate-privately. |
| `crates/polint/src/core/mod.rs` | `AnalysisDb` metadata sidecar and attachment points | VERIFIED | Owns `FactMetaStore`; records metadata for source/syntax/derived/metric families; reports missing metadata. |
| `crates/polint/src/module_graph/mod.rs` | Module graph metadata tests and replace-boundary usage | VERIFIED | Calls `db.replace_module_graph_facts`; metadata tests assert provider and precision/status mapping. |
| `crates/polint/src/symbol_graph/mod.rs` | Symbol graph metadata tests and replace-boundary usage | VERIFIED | Calls `db.replace_symbol_graph_facts`; tests assert symbol/definition/reference metadata stable keys. |
| `crates/polint/src/metrics.rs` | Metrics metadata trigger and default tests | VERIFIED | `derive_requested_metrics` still gates metrics by requested capabilities and calls `db.replace_metric_facts`; metadata tests passed. |
| `crates/polint/tests/cli.rs` | Public behavior compatibility proof | VERIFIED | Contains `kernel_metadata_preserves_public_check_behavior` and existing kernel delegation compatibility coverage. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `core/mod.rs` | `analysis_kernel/metadata.rs` | `AnalysisDb` owns `FactMetaStore` and records `FactRef` rows | WIRED | gsd key-link verification passed. |
| `go/adapter.rs`, `ts/adapter.rs` | `core/mod.rs` | cached facts restored through `restore_file_facts` push paths | WIRED | gsd key-link verification passed; restore test passed. |
| `module_graph/mod.rs` | `core/mod.rs` | `db.replace_module_graph_facts` | WIRED | gsd key-link verification passed; actual derivation calls replace at `module_graph/mod.rs:178`. |
| `symbol_graph/mod.rs` | `core/mod.rs` | `db.replace_symbol_graph_facts` | WIRED | gsd key-link verification passed. |
| `metrics.rs` | `core/mod.rs` | `db.replace_metric_facts` | WIRED | gsd key-link verification passed; `derive_requested_metrics` calls replace at `metrics.rs:61`. |
| `analysis_kernel/mod.rs` | `analysis_kernel/validation.rs` | `AnalysisKernel::run` extends diagnostics with validation diagnostics | WIRED | gsd key-link verification passed; call exists before `Ok(KernelOutput)`. |
| `analysis_kernel/validation.rs` | `core/mod.rs` and provider manifests | Reads `AnalysisDb` facts/metadata and `ProviderManifest` ceilings | WIRED | gsd key-link verification passed. |
| `analysis_kernel/debug.rs` | `core/mod.rs` and `serde_json` | Reads facts/metadata and serializes deterministic report | WIRED | gsd key-link verification passed for data/serialization paths. |
| `lib.rs` | public API boundary | absence of public metadata exports | WIRED | gsd pattern check missed escaped text, but manual `rg` verified `pub(crate) mod analysis_kernel;` and no metadata/debug helper exports. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `core/mod.rs` | `fact_meta` rows for source/syntax facts | `add_source_file`, `push_package`, `push_function`, `push_import`, `push_*` syntax methods, and `restore_file_facts` | Yes - stable keys and payload digests are computed from normalized fact fields | FLOWING |
| `core/mod.rs` | derived provider metadata rows | `replace_module_graph_facts`, `replace_symbol_graph_facts`, `replace_metric_facts` | Yes - metadata is built from current module/symbol/metric vectors | FLOWING |
| `validation.rs` | validation diagnostics | `db.fact_meta()`, `db.missing_fact_metadata()`, `AnalysisDb` fact vectors, `ProviderManifest` ceilings | Yes - diagnostics derive from actual in-memory fact/metadata state | FLOWING |
| `debug.rs` | debug JSON report rows | `AnalysisDb` files/imports/symbols/references plus matching `FactMeta` rows | Yes - tests run a real kernel fixture and assert non-empty arrays | FLOWING |
| `cli.rs` | public JSON behavior | temp-repo external SDK rule via `polint check --format json --fail-on none` | Yes - public report is parsed as `PolintReport` and asserted metadata-free | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Kernel metadata, validation, debug, and provider tests pass | `cargo test -p polint --lib analysis_kernel --locked` | 25 passed | PASS |
| Debug JSON helper coverage passes | `cargo test -p polint --lib metadata_debug --locked` | 5 passed | PASS |
| Public CLI behavior remains deterministic and metadata-free | `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked` | 1 passed | PASS |
| Library lint is warning-clean | `cargo clippy -p polint --lib --all-features --locked -- -D warnings` | Passed | PASS |
| Formatting is stable | `cargo fmt --all -- --check` | Passed | PASS |
| Full workspace test signal | `cargo test --workspace --all-features --locked` | Reported passed after execution; not rerun during this verification | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-FND-02 | 21-01, 21-02, 21-03, 21-04 | Existing fact families carry internal provenance, precision, confidence, validation status, stable-key metadata, and deterministic merge validation. | SATISFIED | All four plans declare `SAE-FND-02`; roadmap maps `SAE-FND-02` to Phase 21; source/syntax/derived/metric metadata, validation, debug JSON, and compatibility tests are present and passing. |

No orphaned Phase 21 requirements were found in `.planning/REQUIREMENTS.md`.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/polint/src/core/mod.rs` | 686 | Review WR-01: `replace_module_graph_facts` rewrites primary graph IDs without remapping dependent references | Warning | Residual internal caller-contract risk. It does not block Phase 21 because the current module graph provider already emits dense IDs (`ResolvedImportId(index)` and builder-assigned node IDs), and validation checks reference integrity before output. |
| `crates/polint/src/analysis_kernel/mod.rs` | 77 | Review WR-02: debug assertion runs before validation diagnostics | Warning | Real defensive-code risk for future metadata gaps in debug/test builds. It does not block current goal achievement because all current kernel-produced families have metadata and clean kernel/CLI runs passed without `polint/internal` diagnostics. |

Stub scan found only false positives in test fixture strings, TOML empty arrays, and formatter strings. No blocker stubs, placeholders, or orphaned implementation artifacts were found.

### Human Verification Required

None.

### Gaps Summary

No blocking gaps found. The phase goal is achieved: metadata is internal, attached across current fact families, validated deterministically, debuggable through crate-private/test-facing JSON, and public behavior remains compatible. The two advisory review warnings are valid residual risks but do not compromise the verified Phase 21 must-haves.

---

_Verified: 2026-05-17T08:19:35Z_
_Verifier: Claude (gsd-verifier)_
