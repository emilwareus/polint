---
phase: 11-capability-driven-analysis-plan
reviewed: 2026-05-09T08:46:13Z
depth: standard
files_reviewed: 15
files_reviewed_list:
  - crates/polint/src/analysis_plan.rs
  - crates/polint/src/cache/mod.rs
  - crates/polint/src/cli/mod.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/go/adapter.rs
  - crates/polint/src/go/mod.rs
  - crates/polint/src/lib.rs
  - crates/polint/src/runner/mod.rs
  - crates/polint/src/sdk/mod.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/src/ts/mod.rs
  - crates/polint/tests/cli.rs
  - docs/facts/README.md
  - docs/facts/capability-plans.md
  - examples/go-test-quality/.polint/rules/src/go_test_quality.rs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 11: Code Review Report

**Reviewed:** 2026-05-09T08:46:13Z
**Depth:** standard
**Files Reviewed:** 15
**Status:** issues_found

## Summary

Reviewed the capability-driven analysis plan implementation, cache-key integration, CLI/runner behavior, SDK exposure, tests, and docs. The main implementation shape is sound and targeted checks pass, but there are three behavioral issues around explain-plan truthfulness and diagnostic filtering.

Verification run during review:

- `cargo check -p polint --locked`
- `cargo test -p polint analysis_plan --lib --locked`
- `cargo test -p polint --test cli explain_plan --locked`

## Warnings

### WR-01: Explain Plan Ignores Severity Overrides

**File:** `crates/polint/src/analysis_plan.rs:135`

**Issue:** `AnalysisPlan::from_inputs` reports each planned rule's severity from `input.meta.severity`, even though `options` already contains the resolved config override. `polint check` applies `RuleOptions::severity` when diagnostics are reported, so `polint explain plan --format json` can claim a rule is `warn` while the same resolved profile will emit `error` or `info`.

**Fix:**

```rust
let rule_options = options
    .get(&input.meta.id)
    .cloned()
    .unwrap_or_default();
let options_digest = deterministic_rule_options(&rule_options);

PlannedRule {
    id: input.meta.id.clone(),
    description: input.meta.description.clone(),
    severity: rule_options.severity.unwrap_or(input.meta.severity),
    requested_capabilities: capabilities,
    options_digest,
}
```

Add a CLI regression test where `[[rules.config]] severity = "error"` and `explain plan --format json` reports `"severity": "error"`.

### WR-02: `--only-rule` Can Hide Unsupported Capability Errors

**File:** `crates/polint/src/analysis_plan.rs:188`

**Issue:** Unsupported capability diagnostics use the synthetic rule id `polint/capability`, while the selected local rule id only appears in the message. The runner then applies `--only-rule` filtering after analysis. A command such as `polint check --only-rule local/needs-cfg --fail-on error` can filter out the selected rule's own unsupported-capability error and incorrectly succeed.

**Fix:** Preserve the owning rule id structurally and make report filtering keep capability diagnostics for matching owners.

```rust
Diagnostic::error(
    "polint/capability",
    "<workspace>",
    TextRange::point(1, 1),
    format!("Rule `{rule_id}` requested unsupported capability `{}`.", capability.capability),
)
.with_evidence("rule", rule_id.clone())
.with_evidence("capability", capability.capability.clone())
```

Then update the report filter to match either `diagnostic.rule_id` or a `rule` evidence value for `polint/capability`. Add an integration test for `--only-rule local/needs-cfg` that still returns the capability diagnostic and fails at the default/error threshold.

### WR-03: `explain plan` Drops Plan-Time Panic Diagnostics

**File:** `crates/polint/src/runner/mod.rs:136`

**Issue:** `RulePlanInputs::collect` converts rule metadata and capability panics into diagnostics, but `runner::explain_plan` ignores `plan_inputs.diagnostics()` and emits a successful plan anyway. A rule whose `capabilities()` panics can therefore produce an incomplete JSON plan with no indication that capability collection failed; `check` reports the problem, but `explain plan` silently loses it.

**Fix:** Fail closed or extend the explain-plan schema to carry diagnostics. With the current schema, the narrowest fix is to return a controlled error before printing the plan:

```rust
let plan_inputs = RulePlanInputs::collect(rules, enabled.as_ref());
let plan_diagnostics = plan_inputs.diagnostics();
if !plan_diagnostics.is_empty() {
    anyhow::bail!(
        "failed to collect rule plan inputs: {}",
        plan_diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    );
}
```

Add a CLI regression test for `polint explain plan --format json` with a rule whose `capabilities()` panics, asserting a non-zero exit and clear error.

---

_Reviewed: 2026-05-09T08:46:13Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
