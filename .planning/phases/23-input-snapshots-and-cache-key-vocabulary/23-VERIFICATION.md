---
phase: 23-input-snapshots-and-cache-key-vocabulary
verified: 2026-05-18T07:25:19Z
status: gaps_found
score: 11/12 must-haves verified
overrides_applied: 0
gaps:
  - truth: "Go and TS/JS lifecycle inputs are represented with explicit present, absent, unsupported, or setup-missing identity components."
    status: partial
    reason: "Lifecycle files that exist but fail fs::read are silently skipped, so a present unreadable go.mod, go.sum, go.work, package.json, lockfile, or tsconfig can be reported as Absent with no lifecycle files present."
    artifacts:
      - path: "crates/polint/src/analysis_kernel/incremental/input_snapshot.rs"
        issue: "file_digest_component checks path.is_file() but continues on fs::read errors instead of preserving the unreadable file as SetupMissing or another explicit identity component."
    missing:
      - "Preserve unreadable lifecycle files as explicit setup-missing/read-error identity inputs with root-relative details."
      - "Add regression coverage for unreadable Go and TS/JS lifecycle files."
---

# Phase 23: Input Snapshots and Cache-Key Vocabulary Verification Report

**Phase Goal:** Add the typed snapshot and key vocabulary required for correct layered cache invalidation.  
**Verified:** 2026-05-18T07:25:19Z  
**Status:** gaps_found  
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `InputSnapshot`, `Digest`, `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey` exist internally. | VERIFIED | `digest.rs`, `input_snapshot.rs`, and `keys.rs` define crate-private typed vocabulary; `analysis_kernel` is `pub(crate)` in `lib.rs` and `incremental` is `pub(crate)` in `analysis_kernel/mod.rs`. |
| 2 | Internal Rust code has typed digest, cache-key, provider-output, and cache-stat vocabulary without public SDK, runner, crate-root, CLI, or stable JSON exposure. | VERIFIED | Public-surface grep found internal markers only in `analysis_kernel` and the no-leak CLI test; no SDK/runner/CLI exports. `input_snapshots_stay_internal` passed. |
| 3 | Digest construction is deterministic, kind-aware, length-prefixed, and canonically sorted for variable lists. | VERIFIED | `Digest::from_parts` includes the digest kind and length-prefixes labeled values before `stable_hash`; `from_unordered` sorts digests before hashing; focused tests cover determinism and serde/display. |
| 4 | `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey` encode Phase 23 identity inputs needed by later cache invalidation. | VERIFIED | `keys.rs` defines all four key types with explicit digest fields and canonical sorting; tests cover serialization and canonical digest-list order. |
| 5 | Existing `CacheKey`/config/rule/plan digest inputs bridge into typed internal identity without deleting compatibility behavior. | VERIFIED | `LayerKey::from_existing_file_cache` wraps `file_hash`, `config_hash`, `rule_hash`, `plan_hash`, `version`, and `schema`; existing `read_json_or_miss`/`write_json` still delegate to compatibility APIs. |
| 6 | `InputSnapshot` represents one coherent run input view over discovered files, config, lifecycle, rules, models, extensions, tools, provider schema versions, language scopes, and cache policies. | PARTIAL | Normal source/config/rule/model/extension/provider/lifecycle paths are represented and tested, but unreadable lifecycle files are silently omitted from `file_digest_component`. |
| 7 | Source text digests are identity inputs while mtime-like filesystem data is only a deterministic hint, and debug serialization avoids raw source, absolute paths, temp roots, timestamps, and nondeterministic order. | VERIFIED | `FileSnapshot` stores `content_hash` as `SourceText`, source length, and `mtime_hint_present`; tests assert no raw source or temp root appears and fixture JSON excludes `mtime_hint`. |
| 8 | Go and TS/JS lifecycle inputs use explicit present, absent, unsupported, or setup-missing identity components. | FAILED | Present readable files, unsupported tool invocations, absent models, and Go setup gaps are covered, but `fs::read` errors at `input_snapshot.rs:478` are `continue`d and can collapse a present unreadable lifecycle file into `Absent`. |
| 9 | Existing Go and TS/JS file-fact cache behavior is preserved while cache reads/writes produce deterministic internal stats. | VERIFIED | `CacheReadStatus`, `CacheWriteStatus`, and Go/TS `analyze_with_plan_options_and_cache_stats` wrappers record hits/misses/recomputes/writes/disabled/invalid counts while diagnostic-only wrappers still return diagnostics. |
| 10 | `KernelOutput` includes a crate-private run report with `InputSnapshot`, provider output metadata, and aggregate cache stats. | VERIFIED | `AnalysisKernel::run` builds `InputSnapshot`, collects provider output rows for all six manifests, passes Go/TS cache stats, and attaches `KernelRunReport` to `KernelOutput`. |
| 11 | Phase 22 native eval fixtures prove Phase 23 snapshot/key/provider invariants and exact current-cache counter values without Phase 24 layer-cache behavior. | VERIFIED | `cache/input-snapshots` fixture expects snapshot, layer-key, provider-output, and exact Go/TS first-run counter invariants; forbidden Phase 24/34 terms are absent. `eval_input_snapshot_fixture_passes` passed. |
| 12 | Public `polint check --format json` output remains deterministic and internal-vocabulary-free; SDK facts, runner behavior, ignore handling, and diagnostics rendering are unchanged. | VERIFIED | CLI test runs public JSON twice, asserts byte identity, parses the public report, and checks no internal snapshot/key/report markers leak. Existing compatibility test evidence was supplied by the orchestrator. |

**Score:** 11/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/polint/src/analysis_kernel/incremental/digest.rs` | `Digest` and `DigestKind` typed identity helpers | VERIFIED | Exists, substantive, crate-private, uses `stable_hash`, length-prefixed parts, kind separation, absent/unsupported helpers, and tests. |
| `crates/polint/src/analysis_kernel/incremental/keys.rs` | `LayerKey`, `QueryKey`, `SummaryKey`, `DiagnosticKey` | VERIFIED | Exists, substantive, canonical list sorting, compatibility bridge from `CacheKey`, and serde tests. |
| `crates/polint/src/analysis_kernel/incremental/stats.rs` | `CacheStats` and `ProviderOutputMeta` | VERIFIED | Exists, substantive, all required counters and provider metadata fields present and tested. |
| `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` | `InputSnapshot` and lifecycle snapshot construction | PARTIAL | Substantive and wired, but unreadable lifecycle files are skipped instead of represented explicitly. |
| `crates/polint/src/cache/mod.rs` | Typed cache read/write status helpers | VERIFIED | `CacheReadStatus`, `CacheReadOutcome`, and `CacheWriteStatus` exist; compatibility read/write helpers delegate to status-aware methods. |
| `crates/polint/src/go/adapter.rs` | Go syntax provider cache stats | VERIFIED | Stats-returning wrapper records per-file cache events and diagnostic wrapper preserves previous return shape. |
| `crates/polint/src/ts/adapter.rs` | TS/JS syntax provider cache stats | VERIFIED | Mirrors Go cache-event aggregation and preserves diagnostic wrapper behavior. |
| `crates/polint/src/analysis_kernel/incremental/run_report.rs` | `KernelRunReport` and provider output construction | VERIFIED | Builds provider output metadata from manifest identity and aggregates cache stats. |
| `crates/polint/src/analysis_kernel/mod.rs` | Kernel run report attachment | VERIFIED | Constructs input snapshot, provider outputs, and run report during kernel execution. |
| `crates/polint/src/analysis_kernel/provider.rs` | Manifest identity helpers | VERIFIED | Exposes provider version, schema label, language scope, and cache policy labels internally. |
| `crates/polint/src/eval/observed.rs` | Eval observed invariants from `KernelRunReport` | VERIFIED | Emits snapshot, layer-key, provider-output, and cache-counter invariants from internal report. |
| `tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml` | Native cache/snapshot fixture expectations | VERIFIED | Covers snapshot/schema, source/config/lifecycle/rule/model/tool status, provider output, and exact counters. |
| `crates/polint/tests/cli.rs` | Public no-leak proof | VERIFIED | `input_snapshots_stay_internal` asserts deterministic public JSON and absence of internal markers. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `digest.rs` | `cache/mod.rs` | `Digest::from_parts` uses `stable_hash` over length-prefixed parts | WIRED | gsd-tools key-link verification passed. |
| `keys.rs` | `cache/mod.rs` | `LayerKey` bridge includes `CacheKey` fields | WIRED | gsd-tools key-link verification passed. |
| `input_snapshot.rs` | `core/mod.rs` | Reads `SourceFile` relative path, language, hash, and source length | WIRED | gsd-tools key-link verification passed. |
| `input_snapshot.rs` | `go/lifecycle.rs` | Uses `GoAnalysisConfig::from_loaded` | WIRED | gsd-tools key-link verification passed. |
| `input_snapshot.rs` | `analysis_kernel/provider.rs` | Reads provider manifest id/schema/scope/cache policy/precision/input/output metadata | WIRED | gsd-tools key-link verification passed. |
| `go/adapter.rs` | `incremental/stats.rs` | Records `CacheStats` counters | WIRED | Manual trace confirms per-file event aggregation. |
| `ts/adapter.rs` | `cache/mod.rs` | Uses status-aware cache read/write helpers | WIRED | Manual trace confirms status-aware read/write and stat aggregation. |
| `analysis_kernel/mod.rs` | `input_snapshot.rs` | Constructs `InputSnapshot` after source loading | WIRED | `AnalysisKernel::run` constructs snapshot before provider execution. |
| `analysis_kernel/mod.rs` | Go/TS adapters | Calls stats-returning wrappers for provider metadata | WIRED | Go and TS cache stats flow into provider output rows. |
| `eval/observed.rs` | `analysis_kernel/mod.rs` | Reads `KernelOutput.run_report` and emits observed invariants | WIRED | Eval observer consumes `output.run_report` directly. |
| `expected.polint-eval.toml` | `eval/fixtures.rs` | Native fixture runner asserts snapshot/key/provider invariants | WIRED | `eval_input_snapshot_fixture_passes` passed. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `input_snapshot.rs` | `InputSnapshot.files`, config, lifecycle, rules, models, extensions, providers | `AnalysisKernel::run` passes `LoadedConfig`, `AnalysisDb`, digests, and provider manifests | Yes for normal paths; read-error lifecycle path is not preserved | PARTIAL |
| `run_report.rs` | `KernelRunReport.provider_outputs`, `cache_stats` | Provider manifests, fact metadata summaries, Go/TS adapter cache stats | Yes | FLOWING |
| `analysis_kernel/mod.rs` | `KernelOutput.run_report` | Constructed from snapshot and provider outputs during kernel execution | Yes | FLOWING |
| `eval/observed.rs` | Observed snapshot/key/provider/cache invariants | `KernelOutput.run_report` | Yes | FLOWING |
| `cli.rs` | Public no-leak assertions | Real `polint check --format json` subprocess output | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Native input-snapshot eval fixture passes | `cargo test -p polint --lib eval_input_snapshot_fixture_passes --locked` | 1 passed | PASS |
| Public JSON remains deterministic and internal-vocabulary-free | `cargo test -p polint --test cli input_snapshots_stay_internal --locked` | 1 passed | PASS |
| Lifecycle happy/setup-missing/tool status tests pass | `cargo test -p polint --lib input_snapshot::lifecycle --locked` | 4 passed | PASS |
| Workspace verification | Orchestrator evidence: `cargo test --workspace --all-features --locked` | Passed | PASS |
| Clippy/fmt | Orchestrator evidence: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`; `cargo fmt --all -- --check` | Passed | PASS |
| Schema drift | Orchestrator evidence: drift check | `drift_detected=false` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-FND-04 | 23-01 through 23-05 | polint records input snapshots, typed cache keys, provider output metadata, cache stats, and lifecycle/toolchain/rule/model digest inputs needed for correct cache invalidation. | PARTIAL | All five plans declare SAE-FND-04 and most implementation evidence exists. The unreadable lifecycle file path is a remaining correctness gap for lifecycle inputs needed by invalidation. |

No orphaned Phase 23 requirements were found in `.planning/REQUIREMENTS.md`; `SAE-FND-04` is the only requirement mapped to Phase 23 and is claimed by every plan.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` | 478 | `let Ok(contents) = fs::read(&path) else { continue; };` | Warning / Gap | Present unreadable lifecycle files are silently omitted and may be reported as absent, weakening cache identity correctness. |
| `crates/polint/tests/cli.rs` | multiple | `TODO` / placeholder literals | Info | Test fixture strings for rules that detect TODO/placeholder literals; not implementation stubs. |
| `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` | 871, 1116 | `return null` inside fixture source strings | Info | TS fixture payload, not Rust implementation behavior. |

### Human Verification Required

None. This phase is internal Rust/cache/eval behavior and was verifiable through source inspection and automated tests.

### Gaps Summary

Phase 23 mostly delivers the intended internal vocabulary and instrumentation: typed digests/keys, input snapshots, cache stats, provider output metadata, kernel run reports, eval fixture proof, and public no-leak checks are present and wired.

The blocking gap is the lifecycle read-error path. `file_digest_component` distinguishes missing files from readable present files, but not present unreadable files. For cache invalidation vocabulary, that means an important lifecycle input can disappear from the snapshot instead of becoming an explicit setup-missing/read-error identity component. This matches the advisory review finding `WR-01` and should be closed before treating SAE-FND-04 as fully achieved.

---

_Verified: 2026-05-18T07:25:19Z_  
_Verifier: Claude (gsd-verifier)_
