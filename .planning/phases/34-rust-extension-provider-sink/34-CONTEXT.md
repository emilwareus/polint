# Phase 34: Rust Extension/Provider Sink - Context

**Gathered:** 2026-05-23
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 34 --auto`

<domain>
## Phase Boundary

Phase 34 delivers the first repo-local Rust extension/provider sink for polint. It should add a process-isolated extension host/protocol, typed sink contracts, declared read/output sets, validation and merge gates, extension provenance/precision ceilings, activation status, cache-key participation, and default-vs-extended eval evidence.

This phase does **not** add broad framework semantics, public query views, refined call graph providers, full data-flow models, or agent-facing extension scaffolding as stable product workflows. Phase 34 should prove that extension-produced facts can safely enter the private analysis substrate under strict validation. Phase 35 can then use this boundary for framework entrypoints and trust boundaries; Phases 37-39 can use it for refined calls, flow, and evidence; Phase 41 can promote stable public ergonomics after validation.

</domain>

<decisions>
## Implementation Decisions

### Extension Runtime and Activation

- **D-01:** Use repo-local Rust extension crates under `.polint/extensions/<name>/` as the authoring unit. Do not use TOML/YAML-only models as the first advanced extension mechanism.
- **D-02:** Run extensions as process-isolated executables through a versioned protocol. Do not load extension code as Rust dynamic libraries or run arbitrary extension code in-process in Phase 34.
- **D-03:** Discover only checked-in local extension crates. Do not auto-fetch remote extension code, and do not allow hidden network/environment dependencies by default.
- **D-04:** Cache extension builds under `.polint/cache/extensions-target/` or an equivalent internal cache directory. Extension build, handshake, panic, timeout, and protocol failures must become controlled `polint/extension` or `polint/capability` diagnostics, not host crashes.
- **D-05:** Activation should be explicit and observable. The kernel should be able to represent extension states such as configured, built, handshake-ok, active, failed, validation-failed, and disabled; dependent rules or providers must not run with placeholder extension facts when setup fails.

### Typed Sinks and Merge Boundary

- **D-06:** Extension authors must emit through typed sinks, never by mutating `AnalysisDb` directly. The first sink surface should be the smallest real vertical slice that can prove the boundary; planners may choose a generic internal extension fact sink or a narrow preview fact sink, but it must be typed, validated, and fixture-backed.
- **D-07:** Every provider declares read sets and output fact families in a manifest/handshake. Observed reads must be checked against declared reads; undeclared reads are validation failures or quarantine causes.
- **D-08:** Extension output must bind to existing stable file/symbol/callsite/function IDs where applicable. Synthetic IDs are allowed only when explicitly declared with stable keys and evidence.
- **D-09:** Extension facts merge after native facts and before downstream consumers that request extension-influenced facts. Native facts remain authoritative unless a fact family defines an explicit merge policy; conflicts with native facts are rejected or quarantined rather than silently overriding native rows.

### Validation, Provenance, and Precision Ceilings

- **D-10:** Validate extension manifests, protocol responses, declared outputs, fact bindings, spans, language/file references, precision/provenance fields, duplicate/conflicting facts, and malformed payloads before merge.
- **D-11:** Every accepted extension fact must carry extension id, provider id, protocol/schema version, precision, confidence/status, evidence, and an output digest suitable for debug/eval/cache tracking.
- **D-12:** Precision ceilings are mandatory. Extension output must not claim `Exact` unless the validation evidence supports exactness; heuristic, user-asserted, generated-unvalidated, and fixture-validated outputs must retain distinct precision/status labels.
- **D-13:** Rejected extension facts should be observable in internal eval/debug output with stable reasons, but rejected facts must not enter normal analysis stores.

### Cache Identity, Quarantine, and Delta Evidence

- **D-14:** Extension cache identity must include extension source digest, `Cargo.lock`/dependency digest where present, extension manifest/protocol/sdk/schema versions, declared inputs, options/config, relevant input fact digests, and provider output digest.
- **D-15:** Reuse Phase 33 quarantine semantics: extension-influenced cache/query/summary/diagnostic nodes are quarantined on extension code, manifest, declared input, validation, or precision-ceiling changes. Native-only nodes are never quarantined.
- **D-16:** Phase 34 eval fixtures must prove invalid extension facts are rejected before merge, accepted extension facts carry provenance/precision, extension digest changes affect cache keys, and default-vs-extended reports show changed facts plus unknown reduction or an explicitly bounded surrogate metric.
- **D-17:** `--no-cache` behavior should still run extension discovery/handshake/provider execution but avoid analysis cache reads/writes, matching existing cache discipline.

### Public Surface and Deferrals

- **D-18:** Keep broad extension ergonomics private or preview-only in Phase 34 unless a surface is necessary for repo-local fixture crates to compile. Any exposed extension module, CLI command, JSON field, or generated text must be intentionally documented as supported or intentionally hidden/test-facing; do not leak internal protocol/debug terms into normal `polint check` output.
- **D-19:** Do not add framework entrypoint modeling, trust-boundary facts, refined call graph sinks, full data-flow sinks, slicing/evidence sinks, or public SDK query views in this phase. Those are later roadmap phases.

### the agent's Discretion

- The planner may choose whether the first typed sink is a narrow generic extension-fact sink used only by internal fixtures, or a minimal preview sink that later Phase 35 can specialize for entrypoints. The chosen sink must be real enough to exercise manifest, protocol, validation, merge, provenance, cache, quarantine, and eval paths.
- The planner may decide exact protocol encoding (JSON, JSONL, or a small serde payload), but it should optimize for deterministic tests and clear diagnostics over performance in this phase.
- The planner may split work across host/protocol, manifest/discovery, sink/validation, cache/quarantine, eval/no-leak, and docs/test fixture plans as long as each plan is independently reviewable and compiling.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 34 goal, requirement mapping (SAE-INT-04), research links, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-INT-04 requirement text and traceability.
- `.planning/PROJECT.md` — v1.2 boundaries, public API discipline, and extension-surface milestone intent.

### Extension Surface Research

- `research/agent-extension-surface/FINAL-REPORT.md` — two-surface model (`#[polint::rule]` vs analysis extensions), process isolation, typed sinks, required invariants, and first vertical-slice recommendation.
- `research/agent-extension-surface/RECOMMENDED_IMPLEMENTATION.md` — repo-local Rust extension layout, process host, manifest/handshake, typed sink concept, provenance, capability planning, validation, and extension test/diff expectations.
- `research/agent-extension-surface/VALIDATION.md` — manifest/protocol/fact/fixture/delta validation levels, precision gates, regression suite, and supply-chain defaults.
- `research/agent-extension-surface/implementation/polint-extension-surface-path.md` — concrete lifecycle path for `.polint/extensions`, protocol messages, activation, test/diff commands, and future ergonomics.
- `research/agent-extension-surface/algorithms/extension-lifecycle.md` — discovery, preparation, provider execution, merge, delta measurement, and rule scheduling pseudocode.

### Rule Authoring Research

- `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md` — rule/extension separation, manifest discipline, narrow `RuleCtx`, test runner expectations, and future domain query ergonomics.
- `research/agent-rule-authoring/FINAL-REPORT.md` — provider extension positioning relative to normal rules and agent inspect/debug workflows.
- `research/agent-rule-authoring/implementation/POLINT-RULE-SDK-AUTHORING.md` — provider extension protocol sketch and distinction between rule packs and provider extensions.

### Upstream Phase Decisions

- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — demand-query substrate, extension-aware quarantine, query trace, and explicit deferral of real extension providers to Phase 34.
- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — summary kernel, direct summary store, extension-authored summaries deferred to Phase 34.
- `.planning/phases/31-p0-abstract-domain-kernel/31-CONTEXT.md` — domain solver/product state and extension-authored domain slots deferred to Phase 34+.

### Existing Implementation

- `crates/polint/src/analysis_kernel/provider.rs` — provider manifests, provider kinds, schema versions, precision ceilings, and current provider order.
- `crates/polint/src/analysis_kernel/mod.rs` — current eager kernel provider execution order and integration point for extension host scheduling.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` — typed input snapshot with currently absent extension provider component.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` — `LayerKind::Extension`, extension digests on layer keys, `SummaryKey.extension_digest`, and absent extension sentinels.
- `crates/polint/src/analysis_kernel/incremental/quarantine.rs` — cache-level quarantine store, native-only detection, quarantine/reinstate lifecycle.
- `crates/polint/src/analysis/demand/quarantine.rs` — extension quarantine reason vocabulary for demand/query-level records.
- `crates/polint/src/analysis_kernel/metadata.rs` and `crates/polint/src/analysis_kernel/validation.rs` — fact metadata, provider manifest validation, precision ceiling checks, and diagnostics patterns.
- `crates/polint/src/eval/model.rs`, `crates/polint/src/eval/fixtures.rs`, and `tests/eval-fixtures/extension/rejection-delta/expected.polint-eval.toml` — current extension fixture area, accepted/rejected synthetic fact evidence, and default-vs-extension delta invariant pattern.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — public API visibility rules and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- Provider manifest infrastructure already records provider id, kind, inputs, outputs, language scope, cache policy, schema versions, and precision ceiling. Phase 34 can extend or parallel this shape for extension providers.
- `InputSnapshot` already has an `extensions` component with an absent sentinel for "no extension providers configured"; Phase 34 should replace that placeholder with real extension source/manifest/dependency/config digests.
- `LayerKey` already carries `extension_digests`, `LayerKind::Extension` exists, `SummaryKey` has `extension_digest`, and existing layer constructors use absent extension sentinels. Phase 34 should wire real digests through these paths rather than inventing a separate cache identity system.
- Phase 33 added both cache-level and demand-level quarantine vocabulary. Real extension providers should feed these mechanisms rather than introducing another quarantine model.
- The eval harness already has `FixtureArea::Extension`, synthetic observed rows gated to extension fixtures, accepted/rejected fact statuses, and an `extension.default_vs_extension_delta` invariant. These are the starting point for real extension sink fixtures.

### Established Patterns

- New analysis families stay crate-private unless a phase intentionally promotes a supported surface with tests, docs, and no-leak coverage.
- Provider output is normalized deterministically, assigned metadata, validated before use, exposed to eval/debug through test-facing JSON, and kept out of normal public check JSON unless deliberately promoted.
- Cache identities include provider/schema/config/lifecycle/upstream digests plus absent future extension/model/toolchain slots; Phase 34 should turn the extension slot from absent to real.
- Setup gaps and unsupported capabilities produce diagnostics and block dependent execution rather than running with placeholder facts.
- Public no-leak tests should scan normal CLI JSON/help, SDK, runner, README, and docs for internal protocol/debug terms.

### Integration Points

- Extension discovery/handshake should happen after source/config snapshots exist and before providers that consume extension facts.
- Extension provider rows should participate in `KernelRunReport` provider outputs or an equivalent extension-output report with cache stats and output digest.
- Metadata validation must validate extension producer/layer ids against extension manifests, not only the static native provider list.
- Capability planning needs an extension-aware status path: native-supported, extension-supported, setup-missing, validation-failed, unsupported.
- Eval observation should compare default vs extended runs and normalize accepted/rejected facts, validation failures, unknown-count deltas, runtime, and cache behavior.

</code_context>

<specifics>
## Specific Ideas

- Start with a deliberately small real vertical slice: one repo-local Rust extension crate in a fixture emits one validated extension fact family through a typed sink, plus one malformed fact that is rejected before merge.
- Treat the Phase 22 synthetic extension fixture as the compatibility baseline, then replace or supplement it with a real extension-hosted fixture that proves `extension.real_sink_active = true`.
- Prefer a stable, compact protocol payload that contains manifest, declared reads, emitted facts, diagnostics, output digest inputs, and validation evidence; raw `AnalysisDb`, parser ASTs, absolute paths, and run-local dense IDs should not cross the process boundary.
- Use `polint.extension.<extension_id>.<provider_id>` or another deterministic producer id scheme for extension metadata rows.
- If a public extension SDK module is unavoidable for fixture crates, keep it narrow and intentional; otherwise keep the first host/protocol crate-private and test-facing until Phase 41 promotion.

</specifics>

<deferred>
## Deferred Ideas

- Framework entrypoints, lifecycle callbacks, dispatch, jobs, CLIs, MCP tools/resources/prompts, tests, generated dispatch, and trust boundaries: Phase 35.
- Type/value/place/alias hint sinks beyond minimal extension-boundary proof: Phase 36.
- Refined call graph providers and extension-provided target edges: Phase 37.
- Full data-flow model sinks, source/sink/sanitizer/barrier models, and path search: Phase 38.
- Slicing, paths, evidence bundles, and diagnostic evidence rendering: Phase 39.
- External benchmark adapters and promotion gates for extension precision claims: Phase 40.
- Stable public extension ergonomics, agent scaffolding, generated docs, and public SDK query views: Phase 41 unless a smaller surface is intentionally promoted in Phase 34.

</deferred>

---

*Phase: 34-rust-extension-provider-sink*
*Context gathered: 2026-05-23*
