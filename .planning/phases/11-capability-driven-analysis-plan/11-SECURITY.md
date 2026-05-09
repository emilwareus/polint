---
phase: "11-capability-driven-analysis-plan"
phase_number: 11
phase_name: "capability-driven-analysis-plan"
security_audit: true
audited_at: "2026-05-09T10:03:33Z"
asvs_level: 1
block_on: "high"
threats_total: 14
threats_closed: 14
threats_open: 0
accepted_risks: 1
transferred_threats: 1
unregistered_flags: 0
status: "secured"
---

# Phase 11 Security Verification

Phase 11 threat mitigations were verified against the threat registers in:

- `.planning/phases/11-capability-driven-analysis-plan/11-01-PLAN.md`
- `.planning/phases/11-capability-driven-analysis-plan/11-02-PLAN.md`
- `.planning/phases/11-capability-driven-analysis-plan/11-03-PLAN.md`

Security config: ASVS Level 1, `block_on: high`.

## Threat Register

| Threat ID | Category | Component | Disposition | Status | Evidence |
|-----------|----------|-----------|-------------|--------|----------|
| T-11-01-01 | Tampering | `AnalysisPlan::from_rules` | mitigate | CLOSED | Deterministic planning sorts rules and aggregates capabilities with ordered collections in `crates/polint/src/analysis_plan.rs:140` and `crates/polint/src/analysis_plan.rs:501`; regression `analysis_plan_merges_enabled_rule_capabilities_deterministically` exists at `crates/polint/src/analysis_plan.rs:779`. |
| T-11-01-02 | Information Disclosure | `polint::sdk::prelude` | mitigate | CLOSED | SDK prelude exports `CapabilitySupport`, `CapabilitySupportStatus`, and `CapabilitySupportView` at `crates/polint/src/sdk/mod.rs:28`; `AnalysisPlan` has no match in `crates/polint/src/sdk/mod.rs`, and the plan module is crate-private at `crates/polint/src/lib.rs:13`. |
| T-11-01-03 | Repudiation | unsupported capability diagnostics | mitigate | CLOSED | Unsupported diagnostics use rule id `polint/capability`, path `<workspace>`, evidence key `capability`, and roadmap help in `crates/polint/src/analysis_plan.rs:188`, `crates/polint/src/analysis_plan.rs:189`, `crates/polint/src/analysis_plan.rs:197`, and `crates/polint/src/analysis_plan.rs:199`. |
| T-11-01-04 | Denial of Service | rule metadata/capability panic | transfer | CLOSED | Transfer target is documented in Plan 11-02 at `.planning/phases/11-capability-driven-analysis-plan/11-02-PLAN.md:184`; the transferred mitigation is implemented through `catch_unwind` around `meta()` and `capabilities()` in `crates/polint/src/analysis_plan.rs:353` and `crates/polint/src/analysis_plan.rs:369`, with regression coverage at `crates/polint/src/analysis_plan.rs:956`. |
| T-11-02-01 | Tampering | `CacheKey::stable_id` | mitigate | CLOSED | `plan_hash` is stored and included in cache stable IDs in `crates/polint/src/cache/mod.rs:15`, `crates/polint/src/cache/mod.rs:50`, and `crates/polint/src/cache/mod.rs:61`; regression `cache_key_changes_with_plan_hash` exists at `crates/polint/src/cache/mod.rs:194`. |
| T-11-02-02 | Denial of Service | `runner::analyze_and_run` | mitigate | CLOSED | Child check collects plan inputs, options, rule digest, and plan before source loading in `crates/polint/src/runner/mod.rs:216` through `crates/polint/src/runner/mod.rs:221`; unsupported capabilities become diagnostics in `crates/polint/src/analysis_plan.rs:188`. |
| T-11-02-03 | Repudiation | adapter outputs | mitigate | CLOSED | Go and TS/JS adapters sort per-file results by `file_id` after adding the plan input in `crates/polint/src/go/adapter.rs:76` and `crates/polint/src/ts/adapter.rs:83`. |
| T-11-02-04 | Elevation of Privilege | local rule execution | accept | ACCEPTED | Accepted risk is recorded below. Phase 11 preserves explicit Cargo invocation boundaries for local rule hosts; the accepted disposition is declared in `.planning/phases/11-capability-driven-analysis-plan/11-02-PLAN.md:280`. |
| T-11-02-05 | Denial of Service | plan-time rule metadata/capability collection | mitigate | CLOSED | `RulePlanInputs::collect` wraps rule `meta()` and `capabilities()` with `catch_unwind(AssertUnwindSafe(...))` in `crates/polint/src/analysis_plan.rs:353` and `crates/polint/src/analysis_plan.rs:369`; controlled diagnostics are built in `crates/polint/src/analysis_plan.rs:436` and `crates/polint/src/analysis_plan.rs:446`. |
| T-11-03-01 | Elevation of Privilege | `run_local_rule_host_explain_plan` | mitigate | CLOSED | Parent explain-plan delegation uses `ProcessCommand::new` plus explicit args, including `CHILD_EXPLAIN_PLAN_JSON_ARGS`, in `crates/polint/src/cli/mod.rs:24`, `crates/polint/src/cli/mod.rs:1016`, and `crates/polint/src/cli/mod.rs:1020`; temp local-host delegation proof exists at `crates/polint/tests/cli.rs:1682`. |
| T-11-03-02 | Tampering | `--format json` stdout | mitigate | CLOSED | Parent parses child JSON with `serde_json::from_str` in `crates/polint/src/cli/mod.rs:1048`; deterministic raw JSON output is tested by `explain_plan_json_is_deterministic` at `crates/polint/tests/cli.rs:1858`. |
| T-11-03-03 | Denial of Service | `polint explain plan` | mitigate | CLOSED | No-local-rule explain emits `AnalysisPlan::empty().explain_report()` without source loading in `crates/polint/src/cli/mod.rs:688`; invalid included source is covered by `explain_plan_no_rules_outputs_empty_json_without_parsing_sources` at `crates/polint/tests/cli.rs:1646`. |
| T-11-03-04 | Repudiation | unsupported capability reporting | mitigate | CLOSED | Unsupported `cfg` diagnostics and docs help are asserted in `crates/polint/tests/cli.rs:1759`, `crates/polint/tests/cli.rs:1779`, and `crates/polint/tests/cli.rs:1785`; implementation evidence is in `crates/polint/src/analysis_plan.rs:188` through `crates/polint/src/analysis_plan.rs:199`. |
| T-11-03-05 | Tampering | cache key behavior | mitigate | CLOSED | Capability-only cache invalidation is covered by `capability_change_changes_cache_entries` at `crates/polint/tests/cli.rs:1881`; adapters pass the plan digest into cache keys in `crates/polint/src/go/adapter.rs:99` through `crates/polint/src/go/adapter.rs:106` and `crates/polint/src/ts/adapter.rs:106` through `crates/polint/src/ts/adapter.rs:113`. |

## Accepted Risks Log

| Threat ID | Accepted Risk | Rationale | Guardrails |
|-----------|---------------|-----------|------------|
| T-11-02-04 | Repo-local rules execute local Rust code. | This is an intentional product boundary for repo-local policy code and is explicitly accepted in the Plan 11-02 threat register. Phase 11 does not add shell delegation for this path and preserves Cargo process boundaries. | Local rule host invocation remains argument-based via `ProcessCommand`; no shell strings were introduced for explain-plan delegation. |

## Transfer Log

| Threat ID | Transfer Target | Verification |
|-----------|-----------------|--------------|
| T-11-01-04 | Plan 11-02 runner integration and panic-contained planning snapshot. | Plan 11-02 documents closure of the transfer, and implementation wraps plan-time metadata/capability calls with `catch_unwind`. |

## Unregistered Flags

None. The Phase 11 summary files do not contain a `## Threat Flags` section.
