---
phase: 11
slug: capability-driven-analysis-plan
status: ready
nyquist_compliant: true
wave_0_complete: false
created: 2026-05-09
---

# Phase 11 - Validation Strategy

Per-phase validation contract for feedback sampling during execution.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` with `assert_cmd`, `predicates`, `tempfile`, `serde_json`, `proptest`, and `insta` |
| **Config file** | Root `Cargo.toml` and `Makefile` |
| **Quick run command** | `cargo test -p polint --lib analysis_plan --locked` |
| **Full suite command** | `cargo test --workspace --all-features --locked` |
| **Estimated runtime** | ~120 seconds |

## Sampling Rate

- **After every task commit:** Run the task's `<automated>` command from the active plan.
- **After every plan wave:** Run `cargo test -p polint --lib --locked` and `cargo test -p polint --test cli --locked`.
- **Before `/gsd-verify-work`:** Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo test --workspace --all-features --locked`.
- **Max feedback latency:** 120 seconds for the targeted command; full-suite latency is allowed at the phase gate.

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | PLAN-01, PLAN-04 | T-11-01-01, T-11-01-03 | Rule capability support is exposed through a narrow, read-only SDK view. | unit | `cargo test -p polint --lib capability_support --locked` | W0 | pending |
| 11-01-02 | 01 | 1 | PLAN-01, PLAN-04 | T-11-01-01, T-11-01-02, T-11-01-03 | Unsupported reserved capabilities become deterministic plan diagnostics. | unit | `cargo test -p polint --lib analysis_plan --locked` | W0 | pending |
| 11-02-01 | 02 | 2 | PLAN-02, PLAN-04 | T-11-02-02 | The child rule host builds the real plan before source loading and contains plan-time rule panics. | unit | `cargo test -p polint --lib analysis_plan --locked` | W0 | pending |
| 11-02-02 | 02 | 2 | PLAN-02, PLAN-03 | T-11-02-01, T-11-02-03 | Go and TS/JS cache keys change when the plan digest changes. | unit | `cargo test -p polint --lib cache_key_changes_with_plan_hash --locked` | W0 | pending |
| 11-03-01 | 03 | 3 | PLAN-01, PLAN-04 | T-11-03-02, T-11-03-03 | Child explain-plan output is deterministic JSON or human text and does not parse source files. | unit | `cargo test -p polint --lib analysis_plan_explain_report --locked` | W0 | pending |
| 11-03-02 | 03 | 3 | PLAN-01, PLAN-04 | T-11-03-01, T-11-03-02, T-11-03-03 | Parent explain-plan delegates through explicit process args and emits an empty valid plan without local rules. | CLI integration | `cargo test -p polint --test cli explain_plan_no_rules_outputs_empty_json_without_parsing_sources --locked` | W0 | pending |
| 11-03-03 | 03 | 3 | PLAN-01, PLAN-03, PLAN-04 | T-11-03-04, T-11-03-05 | External local-rule tests prove unsupported diagnostics and capability-sensitive cache entries. | CLI integration | `cargo test -p polint --test cli explain_plan --locked` | W0 | pending |

## Wave 0 Requirements

- [ ] `crates/polint/src/analysis_plan.rs` or equivalent unit tests for deterministic plan merge, capability support statuses, unsupported diagnostics, and digest stability.
- [ ] `crates/polint/src/cache/mod.rs` or `crates/polint/src/cache/keys.rs` tests proving plan digest changes cache stable IDs.
- [ ] `crates/polint/tests/cli.rs` tests for explain-plan JSON, parent-to-child delegation, no source parsing by default, unsupported capability diagnostics, and capability-sensitive cache invalidation.
- [ ] Adapter tests or integration assertions proving Go and TS/JS adapters receive plan-aware cache inputs.

## Manual-Only Verifications

All phase behaviors have automated verification.

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies.
- [x] Sampling continuity: no 3 consecutive tasks without automated verify.
- [x] Wave 0 covers all missing references.
- [x] No watch-mode flags.
- [x] Feedback latency target is documented.
- [x] `nyquist_compliant: true` set in frontmatter.

**Approval:** approved 2026-05-09
