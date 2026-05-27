# Phase 25: Rule Manifest, Inspect, and Test Skeleton - Context

**Gathered:** 2026-05-18
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 25 promotes the first supported rule-authoring inspection and test loop. It should generate deterministic rule manifests from the existing `#[polint::rule]` macro shape, expose an intentional `polint inspect rule --format json` surface for rule metadata, and add the first `polint test` fixture runner that exercises repo-local rules through the real local-rule-host path and asserts JSON diagnostics.

This phase must not promote broad fact/query/debug surfaces, public provider or cache internals, model packs, extension providers, automatic dynamic rule loading beyond the existing Rust rule-host model, advanced SDK query builders, or public analysis engine internals. Later v1.2 phases own semantic deepening, provider extensions, public advanced SDK queries, and broader agent inspection commands.

</domain>

<decisions>
## Implementation Decisions

### Public Surface Boundary
- **D-01:** Treat `polint inspect rule` and `polint test` as intentional public CLI surfaces. They need stable help text, deterministic machine output, and tests that pin their JSON behavior.
- **D-02:** Keep the Rust `RuleManifest` implementation type internal or crate-private unless the planner finds an existing supported SDK boundary that truly needs it. The public contract for this phase is the CLI JSON shape, not a broad importable Rust manifest API.
- **D-03:** Do not add `polint facts`, `polint unknowns`, broad `polint explain`, provider inspection, cache inspection, or analysis graph commands in this phase. Those are separate promotion surfaces.
- **D-04:** Public JSON must exclude absolute paths, temp roots, timestamps as identity, raw source text, layer-cache internals, provider manifests, `AnalysisDb`, parser AST details, and transient run-local IDs.

### Manifest Generation
- **D-05:** Generate each manifest from the same source of truth the engine uses to run rules: `RuleMeta`, macro-derived typed fact-view parameters, generated capabilities, and resolved `RuleOptions`.
- **D-06:** The manifest should name fact views separately from capabilities. Agents need to see both "this rule requested `Imports<'_>`" and "that maps to `imports` capability."
- **D-07:** Start with the current macro contract: plain non-generic sync functions, `&mut RuleCtx<'_>` first, canonical SDK fact views with `'_`, and `RuleResult` return. Do not reintroduce manual `impl Rule`, handwritten `Capabilities::new()` examples, or broad `RuleCtx` fact access.
- **D-08:** Reserve space in the manifest schema for future metadata such as docs, tags, stability, fixability, message IDs, and limitations, but do not require large macro metadata expansion before the current success criteria are met.
- **D-09:** The `options` section should report the resolved generic `RuleOptions` shape and untyped custom `settings` from config. A typed option-schema system can be `null`, empty, or explicitly "not declared" in this first version.
- **D-10:** Unsupported or setup-missing hard capabilities remain capability diagnostics and must not execute rules with placeholder facts. Manifest output should make those requested capabilities inspectable before rule execution.

### Inspect Command Behavior
- **D-11:** `polint inspect rule --format json` should work without parsing source files or running rules. It should load config/rules, collect rule manifests, derive capability planning data, and return deterministic JSON.
- **D-12:** The parent `polint` CLI should delegate inspection to repo-local rule hosts when `.polint/rules/Cargo.toml` exists, because the child process owns the real `Vec<Rule>` registration.
- **D-13:** The local rule host should gain a matching inspect subcommand rather than exposing internals through the parent process. This mirrors the existing ownership model for `check`.
- **D-14:** With multiple rule hosts or multiple registered rules, inspect JSON should be sorted by manifest rule ID and host path. If a selector is provided, missing rules should produce a clear deterministic error.
- **D-15:** Human output may be concise, but JSON output is the primary acceptance surface for agents and tests.

### Test Runner
- **D-16:** Build `polint test` as a real fixture runner, not a wrapper around `cargo test`. It should compile/run the local rule host and execute `polint check --format json` against temporary repos created from fixture cases.
- **D-17:** Use a fixture layout under `.polint/tests/rules/<rule-name>/<case>/` with a small `polint-test.toml` case manifest, source files, and expected JSON diagnostic assertions or snapshots.
- **D-18:** The first runner should support the essential stable loop: run all cases, filter by rule and/or case if low-churn, emit human and JSON reports, honor `--no-cache`, and optionally keep temp repos for debugging. Blessing snapshots, high-parallel execution, and richer inline marker languages can be deferred if they threaten the first stable skeleton.
- **D-19:** Test output normalization must remove temp roots, absolute machine paths, exact durations, nondeterministic ordering, and cache-local details before comparison or JSON reporting.
- **D-20:** Fixture failures should explain expected vs observed diagnostics by rule ID, file, range/message matcher, and severity. They should not require users or agents to reverse-engineer raw `polint check` output.

### New Rule And External Consumer Proof
- **D-21:** `polint new-rule` should stay aligned with the macro path and may add a minimal fixture skeleton after the test runner format exists. Generated rule code must continue to use only `polint::sdk::prelude::*` and `polint::runner::run_cli`.
- **D-22:** Add at least one temp-repo integration test that behaves like an outside user: generate or write `.polint/rules`, depend on the local `polint` crate as an external dependency, inspect the rule manifest, run `polint test`, and assert a diagnostic through `polint check --format json`.
- **D-23:** Public-surface regression tests should reject internal imports in generated or fixture rule code, including `polint::core`, parser adapters, `analysis_kernel`, cache/layer internals, and manual capability declarations.

### Documentation And Schema
- **D-24:** Add public docs for the new inspect/test authoring loop and keep them honest about Rust rule-host compilation, fixture limits, and which JSON fields are stable.
- **D-25:** Add or update JSON schemas for public inspect/test outputs if the implementation exposes stable JSON beyond the existing `polint-report-v1` schema.
- **D-26:** Keep fact docs aligned with manifest capability names, especially where facts are heuristic, setup-aware, unsupported, or future-reserved.

### the agent's Discretion
- The planner may choose exact file/module names for the manifest model, inspect command, and test runner as long as visibility stays narrow and public JSON is stable.
- The planner may decide whether manifest collection lives in `core`, `analysis_plan`, `runner`, or a new crate-private module, provided `Rule` remains opaque for normal authors.
- The planner may choose whether the first `polint test` assertions are manifest-based, snapshot-based, or a minimal hybrid, as long as temp-repo JSON diagnostics are asserted deterministically.
- The planner may split the phase into separate plans for manifest model, inspect command, test runner, scaffold/docs, and external-consumer regression coverage.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 25 goal, success criteria, research refs, and milestone order.
- `.planning/REQUIREMENTS.md` - `SAE-FND-06` acceptance requirement and v1.2 out-of-scope constraints.
- `.planning/PROJECT.md` - Product value, public API discipline, rule-authoring contract, and behavior-preservation constraints.
- `.planning/STATE.md` - Current milestone state and accumulated prior decisions.

### Rule Authoring Research
- `research/agent-rule-authoring/FINAL-REPORT.md` - Rule authoring direction, manifest purpose, inspect workflow, and fixture-runner expectations.
- `research/agent-rule-authoring/RECOMMENDED_IMPLEMENTATION.md` - Recommended manifest fields, inspect command, `polint test` layout, new-rule fixture direction, and acceptance criteria.
- `research/agent-rule-authoring/VALIDATION.md` - Validation notes against current macro, SDK, and rule-host code.
- `research/agent-rule-authoring/STANDARD.md` - Shared terminology for rule manifests, typed views, RuleCtx, and agent inspect tools.
- `research/agent-rule-authoring/implementation/POLINT-RULE-SDK-AUTHORING.md` - Concrete implementation sketch for manifests, inspect, test, and scaffold improvements.

### Existing Rule-Authoring Docs
- `docs/STATIC-CAPABILITY-DERIVATION-RESEARCH.md` - Current macro-derived capability contract and rejected alternatives.
- `docs/RULE-AUTHORING-PLATFORM-REVIEW.md` - External-consumer proof gaps and rule-authoring platform expectations.
- `docs/CONSUMER-SETUP.md` - Existing repo-local rule setup and config guidance.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

### Prior Phase Decisions
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` - SDK public surface, example rule strategy, `polint new-rule`, and temp-repo proof direction.
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - Internal analysis plan, capability support, setup diagnostics, and local rule-host ownership.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary and no public provider surface.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Internal metadata and no public metadata surface.
- `.planning/phases/22-internal-evaluation-harness-mvp/22-CONTEXT.md` - Internal eval harness, fixture model, deterministic hashing, and no public eval CLI.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Internal snapshot/key vocabulary and no public cache identity surface.
- `.planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-CONTEXT.md` - Layer-cache internals stay private; Phase 25 owns public rule manifest/inspect/test loops.

### Source Surfaces To Inspect
- `crates/polint-macros/src/lib.rs` - Current `#[polint::rule]` metadata parsing, signature validation, typed fact-view capability derivation, and macro tests.
- `crates/polint/src/core/mod.rs` - `RuleMeta`, `Capabilities`, opaque `Rule`, `RuleOptions`, `RuleCtx`, and rule execution.
- `crates/polint/src/sdk/mod.rs` - Supported `polint::sdk::prelude::*` exports and hidden `__private::make_rule` boundary.
- `crates/polint/src/sdk/facts.rs` - Typed fact views that manifest generation must name and map to capabilities.
- `crates/polint/src/analysis_plan.rs` - Existing rule/capability planning, support diagnostics, and option digest behavior.
- `crates/polint/src/runner/mod.rs` - Local rule-host CLI, `run_cli`, and child-owned rule registration.
- `crates/polint/src/cli/mod.rs` - Parent public CLI commands, local rule-host discovery/delegation, `new-rule`, `check`, and current help surface tests.
- `crates/polint/tests/cli.rs` and `crates/polint/tests/common/mod.rs` - Temp-repo, local rule-host, no-leak, and public CLI integration patterns.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint-macros/src/lib.rs` already parses rule metadata, rejects non-analyzable signatures, derives capability methods from canonical SDK fact-view parameters, and generates opaque `Rule` values through `polint::sdk::__private::make_rule`.
- `crates/polint/src/core/mod.rs` already has the runtime ingredients for a manifest: `RuleMeta`, `Capabilities::requested_names`, opaque `Rule` metadata/capability closures, `RuleOptions`, and `CapabilitySupportView`.
- `crates/polint/src/sdk/mod.rs` already defines the public authoring prelude and hidden `__private` bridge used by generated macro code.
- `crates/polint/src/analysis_plan.rs` already collects rule plan inputs, resolved options, capability support rows, and rule/options digests.
- `crates/polint/src/runner/mod.rs` already owns local rule registration through `run_cli(vec![...])` and can be extended with an inspect subcommand that sees the actual registered rules.
- `crates/polint/src/cli/mod.rs` already discovers local rule hosts from `[rules].paths`, delegates `check` to child Cargo processes, and has established command/format/exit-code patterns.
- `crates/polint/tests/cli.rs` already creates temp repos with `.polint/rules`, rewrites generated manifests to local path dependencies, checks public JSON, and asserts public surfaces do not leak internals.

### Established Patterns
- Public rule authors import `polint::sdk::prelude::*` and register with `polint::runner::run_cli`; they do not import `polint::core`, adapters, cache, eval, or analysis kernel modules.
- CLI machine output is deterministic, serde-backed, and tested by parsing JSON into public report types or `serde_json::Value`.
- Internals added in recent phases are protected by no-leak tests that scan public JSON, help output, SDK, runner, and CLI source surfaces.
- The parent CLI delegates local-rule behavior to child rule hosts because only the child process knows the actual rule vector.
- Existing tests prefer temp repos and real `cargo run --manifest-path .polint/rules/Cargo.toml -- ...` execution for external-consumer proof.

### Integration Points
- Add manifest data construction where `Rule` metadata and capabilities are available without making `Rule` a public trait again.
- Extend macro generation so fact-view names can be recorded alongside generated capability methods.
- Extend `polint-local-rules` with an inspect path and have parent `polint inspect rule` delegate to it.
- Add a public parent `polint test` command that discovers fixture cases, creates temporary repos, runs the real local rule host/check path, normalizes check JSON, and reports fixture results.
- Update `polint new-rule` and docs only after the test runner format is clear enough to generate useful fixture skeletons.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: make the first manifest small, deterministic, and useful for agents instead of trying to model every future authoring concept.
- Auto-selected default: prefer public JSON schemas and integration tests over exposing a Rust `RuleManifest` type through the SDK.
- Auto-selected default: `polint inspect rule` should be usable before analysis setup succeeds, because its job is to explain what rules require.
- Auto-selected default: `polint test` should exercise the same child rule-host and `polint check --format json` behavior users rely on, not a parallel in-process shortcut.
- Auto-selected default: generated fixtures should prove public SDK ergonomics and catch accidental regressions to internal imports or manual capabilities.

</specifics>

<deferred>
## Deferred Ideas

- Broad `polint facts`, `polint unknowns`, `polint explain`, provider inspection, cache inspection, and graph/query debug commands - later promotion phases.
- Typed option-schema authoring and rich message/fix descriptor APIs - useful future rule-authoring work, but not required for the first manifest/test skeleton.
- Model packs, provider extensions, generated semantic overlays, and extension handshake manifests - Phase 34 and related later phases.
- Advanced query builders for calls, flow, evidence, effects, and bounded path search - later semantic/interprocedural phases and Phase 41 promotion.
- Parallel `polint test --jobs`, snapshot blessing workflows, and rich inline marker languages - add after the first deterministic fixture runner is stable.

</deferred>

---

*Phase: 25-rule-manifest-inspect-and-test-skeleton*
*Context gathered: 2026-05-18*
