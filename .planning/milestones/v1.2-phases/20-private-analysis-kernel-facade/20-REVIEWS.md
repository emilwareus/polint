---
phase: 20
reviewers: [claude, coderabbit]
reviewed_at: 2026-05-16T20:54:10Z
plans_reviewed: [20-01-PLAN.md, 20-02-PLAN.md]
---

# Cross-AI Plan Review - Phase 20

## Claude Review

# Cross-AI Plan Review: Phase 20 - Private Analysis Kernel Facade

## Plan 20-01: Private Kernel Facade and Runner/CLI Delegation

### Summary
A tightly scoped refactor that extracts the existing six-provider orchestration sequence into a crate-private `AnalysisKernel` facade and routes both the local-rule-host runner and the parent CLI through it, while leaving rule selection, options, ignores, filtering, rendering, and exit behavior outside the kernel. The plan correctly frames itself as "deliberately boring" - establishing an ownership boundary without changing observable behavior - and backs that up with a temp-repo external-consumer test that asserts the same derived fact values rules can read today. Strong API-discipline guardrails and an `<interfaces>` block save the executor from re-discovering existing signatures.

### Strengths
- Crisp scope. The kernel takes ownership of only provider execution and support-overlay merging; everything CLI/reporting stays put. This matches D-03 exactly.
- The temp-repo TDD test (`kernel_delegation_preserves_existing_rule_facts`) is the right shape: it consumes `polint::sdk::prelude::*` like an outside user, requests five typed fact views, and asserts numeric/string values rather than just "no panic."
- Acceptance criteria are mechanically checkable via `rg`, such as the negative grep for `run_rules|apply_ignores|apply_report_filters` inside `analysis_kernel/mod.rs`.
- `<interfaces>` block pre-extracts the contracts the executor must preserve. This is the right answer for a pure-refactor task where re-reading the call sites would be wasted context.
- Threat model entry T-20-01 correctly flags that error propagation must use `anyhow::Result<KernelOutput>` rather than panicking.
- Empty-repo unit test plus error-propagation unit test together cover the two structural failure modes a thin facade can have.

### Concerns
- **MEDIUM - Structural test masquerades as behavioral test.** The acceptance criteria for `kernel_delegation_preserves_existing_rule_facts` require the test to assert specific JSON evidence values and the plan's own threat model accepts the structural grep `rg "AnalysisKernel::run"` in runner/CLI sources. The completed summary admits the integration test would already pass against the old inline orchestration, and the executor backfilled structural assertions to make RED meaningful. If the test passes pre- and post-refactor with the same fixture, it does not actually verify delegation correctness; it verifies that some code path produces the right facts.
- **MEDIUM - Parent CLI path uses `AnalysisPlan::empty()` unconditionally.** Plan 20-01 routes the parent/no-local-rule-host path through the kernel with an empty plan, which means module-graph, symbol-graph, and metrics derivation are no-ops there. This was the prior behavior, but it is now an inherited quirk that the kernel supports rather than a deliberate design choice. The plan should call out that parent-CLI semantics intentionally skip derived providers, so a future refactor does not try to fix it by passing a real plan.
- **LOW - Provider error containment.** If `crate::fs::load_analysis_files` succeeds but a downstream provider panics, the kernel propagates the panic because none of the calls are wrapped in `catch_unwind`. This matches existing behavior, but the plan could note explicitly that the kernel does not introduce new error containment.
- **LOW - Support-overlay ordering is not asserted.** The kernel's contract requires module graph overlays plan support, then symbol graph overlays module support. There is no unit test that exercises a case where both providers produce a setup-missing row for the same capability and asserts the symbol-graph row wins.
- **LOW - Plan diagnostics ordering.** Runner code appends `plan_inputs.diagnostics()`, then `plan.diagnostics()`, then `output.diagnostics`, then rule diagnostics. The kernel does not own this concatenation, but the plan does not say so explicitly.

### Suggestions
- Add one snapshot/golden test of `KernelOutput.capability_support` and `KernelOutput.diagnostics` on a small mixed-language fixture.
- Make the parent-CLI `AnalysisPlan::empty()` decision explicit with a named helper or comment.
- Add an acceptance grep that the kernel does not `catch_unwind`, so error-containment expansion is deliberate in a later phase.

### Risk Assessment
**LOW.** This is a contained refactor with strong scope discipline, explicit deferral of adjacent concerns, and clear before/after surface. The main residual risk is that the behavior-preservation test is weaker than its acceptance criteria suggest, but prior integration tests provide a wide regression net.

---

## Plan 20-02: Internal Provider Manifests and Deterministic Provider-Order Inspection

### Summary
Adds six concrete `ProviderManifest` rows describing the existing providers' inputs, outputs, language scope, cache policy, schema versions, and precision ceiling, plus `#[cfg(test)]` helpers (`provider_order_for_test`, `provider_order_report_for_test`) for deterministic order inspection. The plan recognizes that future-shaped metadata without production use can trip `unreachable_pub`/`dead_code` lints, and forces production consumption of every manifest field through a behavior-preserving consistency path. This is defensive under the workspace lint policy, but creates a code smell the plan should acknowledge more honestly.

### Strengths
- Explicit schema-version names (`go-facts-v2`, `ts-facts-v1`, etc.) tie manifest metadata to real cache schemas already in the codebase.
- The negative grep against cache/query/scheduler/MIR/CFG/dataflow terms is a useful future-creep tripwire.
- Visibility-isolation checks against `crates/polint/src/sdk`, `runner/mod.rs`, and `cli/mod.rs` enforce that provider manifests do not become a rule-author contract.
- `#[cfg(test)]`-gated `ProviderOrderRow` means the test inspection surface does not exist in release builds.
- Naming the helper `provider_order_for_test` prevents accidental promotion of the helper as a public/internal production contract.

### Concerns
- **HIGH - Production manifest consumption is forced make-work.** The behavior-preserving consistency path requiring `AnalysisKernel::run` to read every manifest field solves a lint problem, not a design problem. The completed `provider_manifest_metadata_token()` sums string lengths and enum weights into a dropped `usize`. The reviewer argues this is dead computation used to satisfy clippy, and suggests either adding a documented `#[expect(dead_code)]`, deferring manifests until Phase 23, or gating all manifest types under `#[cfg(test)]` until they have a real consumer.
- **MEDIUM - Schema versions are not validated against actual cache schemas.** Manifest strings such as `go-facts-v2` are not tested against adapter cache constants, so drift could make the manifest inaccurate.
- **MEDIUM - Cache policy field is unenforced documentation.** The manifest declares cache behavior, but the kernel does not check that adapters actually perform the declared cache behavior.
- **MEDIUM - `provider_order_report_for_test` duplicates manifest data.** The report is a subset of manifest data and is asserted inline rather than through a durable snapshot.
- **LOW - Manifest dependency invariants are under-specified.** The test says dependency data is deterministic metadata, but the plan could define the exact invariant more clearly.
- **LOW - `precision_ceiling` is introduced without a rubric.** The plan does not explain why each provider receives `Exact`, `Syntax`, or `SetupAware`.

### Suggestions
- Reconsider production manifest consumption. The reviewer prefers `#[cfg(test)]` manifests until Phase 23 promotes them to a real consumer, or a documented lint expectation over fake runtime consumption.
- Add unit tests tying manifest schema strings to adapter cache schema constants.
- Either drop `precision_ceiling` from this phase or document the rubric for each variant.
- Add an `insta` snapshot of `provider_order_report_for_test()`.

### Risk Assessment
**MEDIUM.** Plan 20-01 is low risk. Plan 20-02 introduces metadata that has no real production consumer yet and is kept live by a synthetic consumption path. The success criteria are met, but the design creates debt for Phase 23.

---

## CodeRabbit Review

CodeRabbit was available but failed:

```text
[2026-05-16T20:53:50.826Z] REVIEW ERROR: Review failed: Unknown error
```

No CodeRabbit findings were produced.

---

## Consensus Summary

Only one external reviewer completed successfully, so there is no multi-reviewer consensus. The single completed review agreed that Phase 20 satisfies `SAE-FND-01` and preserves external behavior, with the main concern concentrated in Plan 20-02.

### Agreed Strengths

- The kernel facade is tightly scoped and behavior-preserving.
- Runner/CLI responsibilities remain outside the kernel.
- Public API discipline is preserved; no SDK or CLI provider surface is introduced.
- Provider order inspection is kept test-only.

### Agreed Concerns

- The manifest model is future-shaped and has no real production consumer yet.
- `provider_manifest_metadata_token()` reads as lint-driven synthetic work, even though it keeps manifest fields live without public API expansion.
- Manifest schema/cache/precision metadata should gain stronger validation or explanation in follow-up phases.

### Divergent Views

- No divergent reviewer views were available because CodeRabbit failed before producing findings.

### Follow-Up Recommendation

Do not block the Phase 20 PR on the review. The phase requirement explicitly asks for internal provider manifests now, and verification passed. Carry the main concern into Phase 21/23 planning: replace the synthetic manifest metadata consumption with a real provenance/cache/snapshot consumer as soon as those phases introduce one, and add schema/precision validation while doing so.
