# Phase 11: Capability-Driven Analysis Plan - Context

**Gathered:** 2026-05-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 11 makes `Capabilities` operational for the current Go and TypeScript/JavaScript pipeline. It builds a deterministic internal analysis plan from enabled repo-local rules, passes that plan into adapters before fact harvesting, includes plan/support inputs in cache keys, and adds `polint explain plan` for debugging.

This phase must not implement future fact families such as CFGs, coverage import, direct call graphs, symbols/references, Python, or Java. Those capabilities belong to later phases. Phase 11 defines the planning contract those later phases will plug into.

</domain>

<decisions>
## Implementation Decisions

### Plan Contract And Visibility
- **D-01:** Keep the full `AnalysisPlan` as an internal orchestration model, not a supported public SDK type.
- **D-02:** The plan should describe requested capabilities, languages, support/requestability status, setup probes, and cache digest inputs.
- **D-03:** Rules may inspect a narrow read-only plan/support view through `RuleCtx`, but they must not depend on or mutate the full internal planner structure.

### Local Rule Host Ownership
- **D-04:** The child `polint-local-rules` process owns real plan construction for repo-local rules, because it is where the actual `Vec<Arc<dyn Rule>>` is registered.
- **D-05:** When local rules exist, the parent `polint explain plan` command should delegate to the local rule host and relay deterministic output.
- **D-06:** When no local rules are registered, `polint explain plan` should produce an empty valid plan instead of failing.

### Fact Gating And Compatibility
- **D-07:** Use hybrid gating in Phase 11: keep basic parsing/source diagnostics and compatibility, but gate optional or future-expensive fact families where safe.
- **D-08:** Do not make currently harvested facts disappear from existing rules solely because a rule forgot to declare a capability. Keep behavior debuggable and compatible while docs/tests push honest capability declarations.
- **D-09:** Adapter cache keys must include a deterministic plan digest, or an equivalent digest component, once the plan affects harvested facts or setup-sensitive analysis.

### Unsupported And Setup-Sensitive Capabilities
- **D-10:** Do not partially implement future facts in Phase 11. CFG, call graph, coverage, symbols/references, and similar future families should not be treated as supported/requestable until their owning phases land real facts.
- **D-11:** If an existing reserved/public API still lets a rule request a future unsupported capability, the plan must not silently accept it. It should fail clearly or emit a deterministic capability diagnostic.
- **D-12:** When a supported capability needs setup and setup is absent, emit a deterministic capability/setup diagnostic with an actionable hint and docs path. Do not reuse parser diagnostics for setup failures.

### Explain Plan Output
- **D-13:** `polint explain plan` should support human output by default and deterministic `--format json` for agents and CI.
- **D-14:** Plan output should include enabled rules, requested capabilities, support/requestability status, setup probes, and plan digest.
- **D-15:** `polint explain plan` should not parse source files by default. It should load config/rules, build the plan, and run setup probes only.

### the agent's Discretion
- The agent may choose exact internal type names and module placement, but public API discipline from `AGENTS.md` applies: supported rule-author surface stays under `polint::sdk` / `polint::runner`.
- The agent may choose the exact stable JSON field names for `explain plan`, as long as output is deterministic, tested, and documented.
- The agent may choose whether plan digest is folded into the existing rule/cache hash or passed as a distinct cache-key component, as long as cache invalidation is correct and tests prove capability changes affect keys.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Milestone Scope
- `.planning/ROADMAP.md` - Phase 11 goal, success criteria, and release gate expectations.
- `.planning/REQUIREMENTS.md` - PLAN-01 through PLAN-04 acceptance requirements.
- `.planning/PROJECT.md` - Product value, public API discipline, truthfulness constraints, and v1.1 target features.

### Capability Research
- `docs/CAPABILITY-FULFILLMENT-RESEARCH.md` - Capability fulfillment research, build methods, adapter contract, and verification checklist.
- `docs/roadmap/01_ENTRY_1_ANALYSIS_PLAN.md` - Human roadmap entry for the capability-driven analysis plan.

### Prior Decisions
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` - Existing `Rule`, `Capabilities`, `RuleCtx`, deterministic runner, and capability-contract decisions.
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` - SDK public surface, rule-authoring helpers, and no large query-engine rewrite.
- `.planning/phases/07-cache-and-performance/07-CONTEXT.md` - Cache key, source-free cache payload, disabled cache, and deterministic parallelism decisions.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/core/mod.rs` - Defines `Capabilities`, `Rule`, `RuleCtx`, `AnalysisDb`, `run_rules`, and existing fact accessors. This is the likely home for internal planning types and narrow SDK-facing support views.
- `crates/polint/src/runner/mod.rs` - `polint-local-rules` owns real repo-local rule registration and currently builds rule options, rule hash, loads files, runs Go/TS adapters, and executes rules.
- `crates/polint/src/cli/mod.rs` - Parent `polint` discovers local rule hosts and delegates `check` to them; `explain plan` should follow the same ownership model when local rules exist.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - Adapter entrypoints currently accept cache/config/rule hashes and parallel flags; Phase 11 needs to thread plan input here.
- `crates/polint/src/cache/keys.rs` - Existing deterministic config/rule hash helpers are the starting point for plan digest/cache key participation.
- `crates/polint/tests/cli.rs` - Existing temp-repo/local-rule-host integration tests provide the pattern for proving external rule behavior.

### Established Patterns
- Public rule-author APIs are exposed through `polint::sdk::prelude::*`; crate-root internals should remain private or `pub(crate)` unless intentionally promoted.
- Existing behavior values compatibility and truthfulness: heuristic or unsupported behavior should be explicit, not silently overclaimed.
- Cache payloads should remain source-free and schema/versioned; stale hits are unacceptable.
- CLI machine output should be deterministic and parseable; human output can be concise but must not corrupt JSON/SARIF streams.

### Integration Points
- Build the actual plan before adapter execution in `runner::analyze_and_run`.
- Add parent CLI delegation for `polint explain plan` through local rule host execution, mirroring `check_local_rule_hosts`.
- Thread a plan digest or plan component into Go/TS adapter cache keys.
- Extend `RuleCtx` construction to include narrow read-only capability support state if rules need runtime visibility.

</code_context>

<specifics>
## Specific Ideas

- The public contract should be the capability/support view, not the internal `AnalysisPlan` structure.
- Phase 11 is an enabling phase. It should make future capabilities non-silent and non-fake, but not implement CFG, coverage, call graph, or symbol facts itself.
- `polint explain plan --format json` is important for agents and CI because it lets them inspect capability support without parsing source files.

</specifics>

<deferred>
## Deferred Ideas

- Actual CFG fact construction - Phase 12.
- Coverage report import - Phase 13.
- Resolved imports/module graph - Phase 14.
- Direct call graph facts - Phase 15.
- Symbol/reference facts - Phase 16.
- Python and Java capability planning details beyond the shared adapter contract - Phases 18 and 19.

</deferred>

---

*Phase: 11-capability-driven-analysis-plan*
*Context gathered: 2026-05-09*
