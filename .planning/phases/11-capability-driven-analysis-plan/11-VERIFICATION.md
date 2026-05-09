---
phase: "11-capability-driven-analysis-plan"
verified: "2026-05-09T09:10:09Z"
status: passed
score: "16/16 must-haves verified"
overrides_applied: 0
human_verification:
  - test: "Review unsupported capability diagnostic and explain-plan human output for rule-author clarity"
    expected: "`polint/capability` diagnostics and `polint explain plan` human output are understandable and actionable for a rule author."
    why_human: "Error message clarity is subjective even though structured fields, help text, docs paths, and exact output shape were verified."
---

# Phase 11: Capability-Driven Analysis Plan Verification Report

**Phase Goal:** Make `Capabilities` drive analysis, setup checks, and cache semantics.
**Verified:** 2026-05-09T09:10:09Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

Phase 11's automated implementation checks pass. The one subjective diagnostic-clarity
item was manually reviewed and recorded in `11-HUMAN-UAT.md`.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Enabled rules are merged into a deterministic `AnalysisPlan`. | VERIFIED | `AnalysisPlan::from_inputs` builds planned rules from `RulePlanInputs`, sorts them deterministically, aggregates capabilities through `BTreeMap`/`BTreeSet`, and hashes length-prefixed parts in `crates/polint/src/analysis_plan.rs:118`, `crates/polint/src/analysis_plan.rs:500`, and `crates/polint/src/analysis_plan.rs:581`. Unit test `analysis_plan_merges_enabled_rule_capabilities_deterministically` passed. |
| 2 | The full `AnalysisPlan` type is not exported through `polint::sdk` or the crate root public API. | VERIFIED | `AnalysisPlan` is `pub(crate)` in `crates/polint/src/analysis_plan.rs:17`; `analysis_plan` module is `pub(crate)` in `crates/polint/src/lib.rs:13`; SDK prelude exports only `CapabilitySupport*` types at `crates/polint/src/sdk/mod.rs:27`. |
| 3 | `RuleCtx` exposes only a narrow read-only capability support view. | VERIFIED | `CapabilitySupportView.entries` is private, exposing only `empty`, `new`, `entries`, and `status_for` in `crates/polint/src/core/mod.rs:728`; `RuleCtx::capability_support()` returns `&CapabilitySupportView` at `crates/polint/src/core/mod.rs:837`. |
| 4 | Reserved capabilities such as `cfg` and `test_suite_metrics` are reported as unsupported, not silently accepted. | VERIFIED | `support_for` marks `cfg`, `call_graph`, `coverage_facts`, and `test_suite_metrics` unsupported, with the required `go_tests` hint for metrics in `crates/polint/src/analysis_plan.rs:525`; diagnostics use `polint/capability`, `<workspace>`, rule evidence, capability evidence, and roadmap help in `crates/polint/src/analysis_plan.rs:181`. |
| 5 | The child local-rule host builds the real plan from its registered `Vec<Arc<dyn Rule>>` before file loading. | VERIFIED | `runner::analyze_and_run` calls `RulePlanInputs::collect`, derives options/rule digest, and builds `AnalysisPlan::from_inputs` before `load_analysis_files` in `crates/polint/src/runner/mod.rs:212`. |
| 6 | Parent no-local-rule analysis uses an empty valid plan. | VERIFIED | Parent analysis uses empty rules/options and `AnalysisPlan::empty()` before Go/TS adapter calls in `crates/polint/src/cli/mod.rs:627` and `crates/polint/src/cli/mod.rs:631`. |
| 7 | Go and TS/JS adapters receive the plan before harvesting facts. | VERIFIED | Child check passes `&plan` to both `crate::go::analyze_with_plan_options` and `crate::ts::analyze_with_plan_options` before rule execution in `crates/polint/src/runner/mod.rs:224`; adapters accept `plan: &AnalysisPlan` in `crates/polint/src/go/adapter.rs:51` and `crates/polint/src/ts/adapter.rs:58`. |
| 8 | Adapter cache keys change when the plan digest changes. | VERIFIED | `CacheKey` stores `plan_hash` and includes it in `stable_id` in `crates/polint/src/cache/mod.rs:11` and `crates/polint/src/cache/mod.rs:56`; Go and TS adapters pass `plan.digest()` into file cache keys at `crates/polint/src/go/adapter.rs:68` and `crates/polint/src/ts/adapter.rs:75`. Unit and CLI cache-change spot checks passed. |
| 9 | Currently harvested facts remain available for compatibility. | VERIFIED | Go still extracts package, imports, string literals, and functions without plan gating in `crates/polint/src/go/adapter.rs:195`; TS/JS still extracts imports/exports, declarations, literals, and JSX in `crates/polint/src/ts/adapter.rs:257`. |
| 10 | Plan-time rule `meta()` and `capabilities()` panics are contained as diagnostics or controlled internal errors. | VERIFIED | `RulePlanInputs::collect` wraps both calls with `catch_unwind` in `crates/polint/src/analysis_plan.rs:347`; child `explain plan` fails closed on plan-input diagnostics in `crates/polint/src/runner/mod.rs:136`. Regression tests for check and explain panic behavior are present and targeted spot checks passed. |
| 11 | `polint explain plan` emits human output by default. | VERIFIED | Parent and child `ExplainPlanFormat` default to `Human` in `crates/polint/src/cli/mod.rs:137` and `crates/polint/src/runner/mod.rs:44`; `to_human` renders `Analysis plan`, `Rules`, `Capabilities`, `Setup checks`, and `Digest:` in `crates/polint/src/analysis_plan.rs:277`. |
| 12 | `polint explain plan --format json` emits deterministic parseable JSON. | VERIFIED | `ExplainPlanReport` is a typed serde report with stable root fields in `crates/polint/src/analysis_plan.rs:68`; status strings are lowercase in `crates/polint/src/analysis_plan.rs:677`; deterministic raw stdout is tested in `crates/polint/tests/cli.rs:1858`. |
| 13 | Parent `polint explain plan` delegates to `polint-local-rules explain plan` when a local rule host exists. | VERIFIED | Parent discovers manifests, calls `run_local_rule_host_explain_plan`, invokes `cargo run --quiet --manifest-path ... -- explain plan --format json` through `ProcessCommand` args, and parses JSON in `crates/polint/src/cli/mod.rs:683` and `crates/polint/src/cli/mod.rs:1005`. |
| 14 | `polint explain plan` with no local rules emits an empty valid plan. | VERIFIED | Parent no-host explain builds `AnalysisPlan::empty().explain_report()` in `crates/polint/src/cli/mod.rs:684`; CLI regression with an invalid Go file verifies empty parseable JSON without source parsing in `crates/polint/tests/cli.rs:1646`. |
| 15 | `polint explain plan` does not parse source files by default. | VERIFIED | Child explain-plan path loads config/rules and builds the plan without `load_analysis_files` in `crates/polint/src/runner/mod.rs:133`; parent no-host explain does the same in `crates/polint/src/cli/mod.rs:683`. The invalid-source CLI spot check passed. |
| 16 | `polint explain plan` shows requested capabilities and setup checks. | VERIFIED | JSON root includes `rules`, `capabilities`, and `setup_checks` in `crates/polint/src/analysis_plan.rs:68`; human output renders `Capabilities` and `Setup checks` in `crates/polint/src/analysis_plan.rs:300`; docs describe the shape in `docs/facts/capability-plans.md:13`. Current setup checks are empty because no Phase 11 supported capability requires setup. |

**Score:** 16/16 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint/src/analysis_plan.rs` | Internal plan construction, support rows, diagnostics, digest, explain report. | VERIFIED | Exists, substantive, crate-private; contains schema, `AnalysisPlan`, `RulePlanInputs`, unsupported capability logic, digest, and tests. |
| `crates/polint/src/lib.rs` | Crate-private analysis plan module registration. | VERIFIED | `pub(crate) mod analysis_plan;` at line 13. |
| `crates/polint/src/core/mod.rs` | SDK-facing support view and plan-aware rule runner plumbing. | VERIFIED | `CapabilitySupport*`, `RuleCtx::capability_support`, and `run_rules_with_capability_support` are present. |
| `crates/polint/src/sdk/mod.rs` | SDK prelude exports for support view only. | VERIFIED | `CapabilitySupport`, `CapabilitySupportStatus`, and `CapabilitySupportView` are exported; `AnalysisPlan` is not. |
| `crates/polint/src/runner/mod.rs` | Child host plan construction, check wiring, explain-plan command. | VERIFIED | Builds plan before file loading, passes support view to rules, and implements child explain plan. |
| `crates/polint/src/cli/mod.rs` | Parent empty-plan path and local-host explain delegation. | VERIFIED | No-host check/explain use `AnalysisPlan::empty`; local-host explain uses `ProcessCommand` args and typed JSON. |
| `crates/polint/src/go/adapter.rs` | Go adapter plan parameter and plan digest cache input. | VERIFIED | `analyze_with_plan_options` accepts `&AnalysisPlan` and passes `plan.digest()` to `CacheKey::for_file`. |
| `crates/polint/src/go/mod.rs` | Crate-internal Go plan-aware re-export. | VERIFIED | `pub(crate) use adapter::analyze_with_plan_options;` at line 11. |
| `crates/polint/src/ts/adapter.rs` | TS/JS adapter plan parameter and plan digest cache input. | VERIFIED | `analyze_with_plan_options` accepts `&AnalysisPlan` and passes `plan.digest()` to `CacheKey::for_file`. |
| `crates/polint/src/ts/mod.rs` | Crate-internal TS/JS plan-aware re-export. | VERIFIED | `pub(crate) use adapter::analyze_with_plan_options;` at line 12. |
| `crates/polint/src/cache/mod.rs` | Plan hash in stable cache identity. | VERIFIED | `plan_hash` participates in `CacheKey::stable_id`; regression test passed. |
| `crates/polint/tests/cli.rs` | External temp-repo proof for plan behavior. | VERIFIED | Tests generate `.polint/rules`, import only `polint::sdk::prelude::*`, register through `polint::runner::run_cli`, and assert explain/check/cache behavior. |
| `docs/facts/capability-plans.md` | Public docs for capability declarations and explain plan. | VERIFIED | Documents JSON fields, supported Phase 11 capabilities, unsupported reserved names, and the `go_tests`/`test_suite_metrics` boundary. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `analysis_plan.rs` | `core/mod.rs` | Imports `Rule`, `RuleOptions`, `Capabilities`, and `CapabilitySupportView`. | VERIFIED | `AnalysisPlan` builds `CapabilitySupportView` rows from planned capabilities. |
| `core/mod.rs` | `sdk/mod.rs` | SDK prelude re-export. | VERIFIED | Support-view types are exported through `polint::sdk::prelude::*`. |
| `runner/mod.rs` | `analysis_plan.rs` | `RulePlanInputs::collect` -> `AnalysisPlan::from_inputs`. | VERIFIED | Child check and child explain plan both use the panic-contained planning path. |
| `runner/mod.rs` | `go/adapter.rs` | `crate::go::analyze_with_plan_options(..., &plan, ...)`. | VERIFIED | Plan is passed before Go fact harvesting. |
| `runner/mod.rs` | `ts/adapter.rs` | `crate::ts::analyze_with_plan_options(..., &plan, ...)`. | VERIFIED | Plan is passed before TS/JS fact harvesting. |
| `go/mod.rs` | `go/adapter.rs` | `pub(crate) use adapter::analyze_with_plan_options`. | VERIFIED | Manual check confirms re-export; automated regex missed spacing/context. |
| `ts/mod.rs` | `ts/adapter.rs` | `pub(crate) use adapter::analyze_with_plan_options`. | VERIFIED | Manual check confirms re-export; automated regex missed spacing/context. |
| `go/adapter.rs` | `cache/mod.rs` | `plan.digest()` -> `CacheKey::for_file(..., plan_hash, ...)`. | VERIFIED | Go adapter cache identity includes the plan digest. |
| `ts/adapter.rs` | `cache/mod.rs` | `plan.digest()` -> `CacheKey::for_file(..., plan_hash, ...)`. | VERIFIED | TS/JS adapter cache identity includes the plan digest. |
| `cli/mod.rs` | `runner/mod.rs` | Parent invokes child `explain plan --format json`. | VERIFIED | Uses `ProcessCommand::new` and explicit args; no shell string. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `analysis_plan.rs` | `RulePlanInputs.rules` | Registered `Vec<Arc<dyn Rule>>` via `meta()` and `capabilities()` with `catch_unwind`. | Yes | FLOWING |
| `runner/mod.rs` | `plan` | `RulePlanInputs::collect` and resolved config-derived `RuleOptions`. | Yes | FLOWING |
| `go/adapter.rs` and `ts/adapter.rs` | `plan_hash` | `plan.digest()` from resolved `AnalysisPlan`. | Yes | FLOWING |
| `cache/mod.rs` | `CacheKey.plan_hash` | Adapter-supplied plan digest. | Yes | FLOWING |
| `analysis_plan.rs` | `ExplainPlanReport` | Planned rules, requested capabilities, setup checks, and digest. | Yes | FLOWING |
| `analysis_plan.rs` | `Diagnostic` for unsupported capabilities | Unsupported `PlannedCapability` rows. | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Analysis plan construction, diagnostics, support view, explain report. | `cargo test -p polint --lib analysis_plan --locked` | 8 passed. | PASS |
| Cache key changes when plan hash changes. | `cargo test -p polint --lib cache_key_changes_with_plan_hash --locked` | 1 passed. | PASS |
| No-rules explain plan emits empty JSON and does not parse invalid source. | `cargo test -p polint --test cli explain_plan_no_rules_outputs_empty_json_without_parsing_sources --locked` | 1 passed. | PASS |
| Parent delegates explain plan to local rule host JSON. | `cargo test -p polint --test cli explain_plan_delegates_to_local_rule_host_json --locked` | 1 passed. | PASS |
| Unsupported reserved capability appears in check diagnostics. | `cargo test -p polint --test cli check_reports_unsupported_reserved_capability --locked` | 1 passed. | PASS |
| Phase gate after review fixes. | Provided by orchestrator: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo test --workspace --all-features --locked`. | Passed. | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| PLAN-01 | 11-01, 11-03 | Rule authors can declare capabilities and see an explicit analysis plan derived from enabled rules. | SATISFIED | `Capabilities` declarations are captured by `RulePlanInputs`, `AnalysisPlan` exposes report rows, and temp-repo tests use public SDK imports and `polint::runner::run_cli`. |
| PLAN-02 | 11-02 | The runner passes the resolved analysis plan to Go and TS/JS adapters before fact harvesting. | SATISFIED | Child runner builds `plan` before `load_analysis_files` and passes `&plan` to both adapter entrypoints. |
| PLAN-03 | 11-02, 11-03 | Cache keys change when requested capabilities or setup-sensitive analysis inputs change. | SATISFIED | `CacheKey.plan_hash` participates in `stable_id`; adapters supply `plan.digest()`; unit and CLI cache-change tests pass. |
| PLAN-04 | 11-01, 11-02, 11-03 | Missing or unsupported setup for requested capabilities becomes a clear diagnostic or structured warning. | SATISFIED | Unsupported reserved capabilities generate structured explain rows and `polint/capability` check diagnostics with rule/capability evidence and docs help. Human review remains for subjective clarity. |

No orphaned Phase 11 requirements were found in `.planning/REQUIREMENTS.md`; PLAN-01 through PLAN-04 are all declared by the Phase 11 plans.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| n/a | n/a | No blocker stubs found. Matches were digest encoder strings, intentional empty-plan collections, and test fixture literals such as `TODO` and `rules = []`. | Info | No impact on goal achievement. |

### Human Verification Completed

#### 1. Diagnostic And Human Output Clarity

**Test:** Run or inspect `polint explain plan` and a `polint/capability` unsupported-capability diagnostic from a local rule that declares `.cfg()`.
**Expected:** A rule author can understand which rule requested the unsupported capability, why it is unsupported, where to read more, and that `go_tests` is the current supported path for Go test evidence.
**Result:** PASS. A throwaway local rule pack declaring `Capabilities::new().cfg()` produced human output that named `local/needs-cfg`, marked `cfg` unsupported, explained that it is reserved for a later phase, linked `docs/roadmap/00_ROADMAP.md`, preserved `polint/capability` under `--only-rule local/needs-cfg`, and exited nonzero.
**Evidence:** `.planning/phases/11-capability-driven-analysis-plan/11-HUMAN-UAT.md`

### Gaps Summary

No automated gaps were found. The phase goal is implemented in code: enabled rule capabilities produce a deterministic internal plan, the child runner builds it before analysis, adapters receive it, cache identity includes the digest, unsupported reserved capabilities are surfaced, and `polint explain plan` exposes deterministic human/JSON output.

Status is `passed`; the subjective diagnostic clarity review above is complete.

---

_Verified: 2026-05-09T09:10:09Z_
_Verifier: Claude (gsd-verifier)_
