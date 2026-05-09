# Phase 11: Capability-Driven Analysis Plan - Research

**Researched:** 2026-05-09 [VERIFIED: current_date]
**Domain:** Rust static-analysis orchestration, internal capability planning, adapter cache invalidation, local-rule-host CLI delegation. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]
**Confidence:** HIGH for codebase integration points and tests; MEDIUM for exact public support-view naming because the phase context delegates exact type names to the implementer. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

<user_constraints>
## User Constraints (from CONTEXT.md)

Source: `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md`. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

### Locked Decisions

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

### Claude's Discretion

- The agent may choose exact internal type names and module placement, but public API discipline from `AGENTS.md` applies: supported rule-author surface stays under `polint::sdk` / `polint::runner`.
- The agent may choose the exact stable JSON field names for `explain plan`, as long as output is deterministic, tested, and documented.
- The agent may choose whether plan digest is folded into the existing rule/cache hash or passed as a distinct cache-key component, as long as cache invalidation is correct and tests prove capability changes affect keys.

### Deferred Ideas (OUT OF SCOPE)

## Deferred Ideas

- Actual CFG fact construction - Phase 12.
- Coverage report import - Phase 13.
- Resolved imports/module graph - Phase 14.
- Direct call graph facts - Phase 15.
- Symbol/reference facts - Phase 16.
- Python and Java capability planning details beyond the shared adapter contract - Phases 18 and 19.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PLAN-01 | Rule authors can declare capabilities and see an explicit analysis plan derived from enabled rules. [VERIFIED: .planning/REQUIREMENTS.md] | Build the plan in `polint-local-rules` from enabled `Arc<dyn Rule>` values, expose `polint explain plan`, and add a narrow `RuleCtx` support view instead of exposing the full plan. [VERIFIED: crates/polint/src/runner/mod.rs, crates/polint/src/core/mod.rs] |
| PLAN-02 | The runner passes the resolved analysis plan to Go and TS/JS adapters before fact harvesting. [VERIFIED: .planning/REQUIREMENTS.md] | Current runner entrypoints call Go and TS adapters before `run_rules`, so the plan must be constructed before the calls at `runner::analyze_and_run` and threaded into both `analyze_with_options` signatures. [VERIFIED: crates/polint/src/runner/mod.rs:138, crates/polint/src/go/adapter.rs:39, crates/polint/src/ts/adapter.rs:46] |
| PLAN-03 | Cache keys change when requested capabilities or setup-sensitive analysis inputs change. [VERIFIED: .planning/REQUIREMENTS.md] | Current `CacheKey` includes file, config, rule hash, version, and schema; add a distinct plan digest component or fold the digest into the adapter hash path and prove capability changes alter cache IDs. [VERIFIED: crates/polint/src/cache/mod.rs:36, crates/polint/src/cache/keys.rs:25] |
| PLAN-04 | Missing or unsupported setup for requested capabilities becomes a clear diagnostic or structured warning. [VERIFIED: .planning/REQUIREMENTS.md] | Reuse `Diagnostic` for `check` and structured plan JSON for `explain plan`; unsupported reserved capabilities must not be accepted as supported. [VERIFIED: crates/polint/src/diagnostics/mod.rs:81, .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] |
</phase_requirements>

## Summary

Phase 11 should be implemented as an internal planning layer around the existing Rust workspace, not as a new dependency stack. [VERIFIED: Cargo.toml, crates/polint/src/lib.rs] The most reliable plan shape is: build a deterministic `AnalysisPlan` in the child `polint-local-rules` host from enabled rules, compute a stable plan digest, pass the plan to Go and TS/JS adapters, and expose only a narrow support view plus `polint explain plan`. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src/runner/mod.rs]

The highest-risk integration points are cache invalidation, JSON delegation, compatibility with existing fact harvesting, and unsupported reserved capabilities. [VERIFIED: crates/polint/src/cache/mod.rs, crates/polint/src/cli/mod.rs, crates/polint/src/core/mod.rs] The current code already has deterministic building blocks: sorted file discovery, `BTreeMap`/`BTreeSet` config structures, explicit stable hash helpers, deterministic diagnostic sorting, and CLI integration tests using temp repos. [VERIFIED: crates/polint/src/fs/mod.rs:54, crates/polint/src/cache/keys.rs:1, crates/polint/src/diagnostics/mod.rs:322, crates/polint/tests/cli.rs]

**Primary recommendation:** Add an internal `analysis_plan` model under `core` or a sibling internal module, add a distinct `plan_digest` cache-key component, delegate parent `polint explain plan` to `polint-local-rules explain plan`, and keep current facts available while marking reserved future capabilities unsupported. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src/cache/mod.rs]

## Project Constraints (from AGENTS.md)

- Work in the Rust 2024 workspace and preserve Go support through tree-sitter and TS/JS support through Oxc. [VERIFIED: AGENTS.md, Cargo.toml]
- Use deterministic parallelism and avoid cloning large source strings because large-repo support is a core requirement. [VERIFIED: AGENTS.md, crates/polint/src/fs/mod.rs:93, crates/polint/src/ts/tests.rs]
- Parser errors and rule panics should become diagnostics or controlled internal errors rather than uncontrolled crashes. [VERIFIED: AGENTS.md, crates/polint/src/core/mod.rs:1049]
- Capability and fact behavior must be truthful; unsupported or heuristic behavior must be explicit. [VERIFIED: AGENTS.md, docs/facts/branches.md, docs/facts/ts-js.md]
- Supported rule-author APIs are `polint::sdk` and `polint::runner`; `core`, `cache`, `config`, `fs`, `go`, `ts`, `graph`, and `cli` remain implementation details unless deliberately promoted. [VERIFIED: AGENTS.md, crates/polint/src/lib.rs:1]
- Use the narrowest visibility that works; new internal planning types should default to private or `pub(crate)`, and any support-view type exposed through `RuleCtx` must be an intentional SDK addition with docs. [VERIFIED: AGENTS.md, docs/API-VISIBILITY-PLAN.md]
- Repo-local rules, including example rules, must behave like external consumers using `polint::sdk::prelude::*` and `polint::runner::run_cli`. [VERIFIED: AGENTS.md, crates/polint/src/sdk/mod.rs:1, crates/polint/src/cli/mod.rs:401]
- A rule-authoring feature needs a temp-repo style integration test with generated `.polint/rules`, public SDK imports only, real facts consumed, and JSON diagnostics asserted through `polint check --format json`. [VERIFIED: AGENTS.md, crates/polint/tests/cli.rs:420]
- Config and resolved rule options that affect rule behavior must participate in deterministic cache digests. [VERIFIED: AGENTS.md, crates/polint/src/cache/keys.rs:25]
- New public facts must be documented under `docs/facts/`, including limits and heuristic behavior. [VERIFIED: AGENTS.md, docs/facts/README.md]
- `CLAUDE.md` is not present in the project root, so no separate CLAUDE.md directives were found. [VERIFIED: test -f CLAUDE.md]
- Project skills found are `.claude/skills/polint/SKILL.md` and `.agents/skills/rust-best-practices/SKILL.md`. [VERIFIED: find .claude/skills .agents/skills -name SKILL.md]

## Standard Stack

### Core

| Library / Module | Version | Purpose | Why Standard |
|------------------|---------|---------|--------------|
| `clap` derive | 4.6.1 | Add `explain plan` subcommands and `--format json`. | The CLI already derives `Parser`, `Args`, `Subcommand`, and `ValueEnum`; clap derive officially supports those traits for subcommands and value enums. [VERIFIED: cargo metadata --locked, crates/polint/src/cli/mod.rs:76] [CITED: https://docs.rs/clap/latest/clap/_derive/] |
| `serde` + `serde_json` | serde 1.0.228, serde_json 1.0.149 | Serialize deterministic `explain plan --format json` output and parse child-host JSON where needed. | The project already uses typed serializable report structs for JSON output and serde is already in the workspace. [VERIFIED: cargo metadata --locked, crates/polint/src/diagnostics/mod.rs:294] |
| `std::collections::BTreeMap` / `BTreeSet` | Rust std 1.95.0 | Stable ordering for rules, setup rows, capability rows, host manifests, and JSON object-like maps. | The codebase already uses `BTreeMap`/`BTreeSet` for config, profiles, rules, and deterministic filtering; Rust documents `BTreeMap` iteration as key ordered. [VERIFIED: crates/polint/src/config/mod.rs:18, crates/polint/src/cli/mod.rs:840] [CITED: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html] |
| `crate::cache::stable_hash` and `cache::keys` encoders | internal | Compute `plan_digest` and cache-key inputs. | Existing cache code explicitly uses deterministic, infallible encoders rather than relying on serializer behavior for cache invalidation. [VERIFIED: crates/polint/src/cache/keys.rs:1, crates/polint/src/cache/mod.rs:137] |
| `anyhow` + `Diagnostic` | anyhow 1.0.102, internal diagnostics | Return CLI errors for fatal command failures and diagnostics for unsupported/setup capability issues. | The runner and CLI already return `anyhow::Result`, while user-visible analysis problems are represented as `Diagnostic`. [VERIFIED: cargo metadata --locked, crates/polint/src/runner/mod.rs:8, crates/polint/src/diagnostics/mod.rs:81] |
| Existing Go and TS/JS adapters | tree-sitter 0.26.8, tree-sitter-go 0.25.0, Oxc 0.129.0 | Receive `AnalysisPlan` before optional fact harvesting. | The adapters already own per-language parsing/fact restoration and cache lookup. [VERIFIED: cargo metadata --locked, crates/polint/src/go/adapter.rs:39, crates/polint/src/ts/adapter.rs:46] |

### Supporting

| Library / Module | Version | Purpose | When to Use |
|------------------|---------|---------|-------------|
| `assert_cmd` / `predicates` / `tempfile` | 2.2.1 / 3.1.4 / 3.27.0 | CLI and temp-repo rule-host integration tests. | Use for `polint explain plan`, unsupported capability diagnostics, cache invalidation smoke tests, and external generated-rule proof. [VERIFIED: cargo metadata --locked, crates/polint/tests/cli.rs] |
| `proptest` | 1.11.0 | Determinism invariants for hashes and ordering. | Use for plan digest boundary/order invariants if a compact unit-level property test is practical. [VERIFIED: cargo metadata --locked, crates/polint/src/cache/mod.rs:230] |
| `insta` | 1.47.2 | Snapshot output contracts where stable JSON/human output is important. | Use only when exact `explain plan` output needs a regression snapshot; existing diagnostics already use snapshots. [VERIFIED: cargo metadata --locked, crates/polint/src/diagnostics/mod.rs tests list] |
| `rayon` | 1.12.0 | Preserve existing deterministic parallel adapter/rule execution. | Keep adapters parallel after the plan is constructed; do not build the plan from unordered parallel results. [VERIFIED: cargo metadata --locked, crates/polint/src/go/adapter.rs:52, crates/polint/src/ts/adapter.rs:59] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Internal `AnalysisPlan` | Public SDK `AnalysisPlan` | Rejected by locked decision D-01 and API visibility constraints. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, AGENTS.md] |
| Child host constructs plan | Parent constructs plan | Rejected by locked decision D-04 because the parent does not own the actual registered `Vec<Arc<dyn Rule>>` for repo-local rules. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src/runner/mod.rs:78] |
| Distinct `plan_digest` cache-key component | Fold digest into existing `rule_hash` string | Both satisfy D-09, but a distinct component is clearer because `rule_hash` currently means enabled rule metadata and resolved options. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src/cache/keys.rs:25] |
| Structured serde output | Manual JSON string assembly | Use typed serde structs because diagnostics JSON already follows that pattern and manual JSON risks escaping/order mistakes. [VERIFIED: crates/polint/src/diagnostics/mod.rs:302] |

**Installation:** No new crates are required for Phase 11. [VERIFIED: Cargo.toml, cargo metadata --locked]

```bash
cargo metadata --format-version 1 --locked
```

**Version verification:** Recommended package versions above were verified from the locked Cargo metadata on 2026-05-09, not inferred from training data. [VERIFIED: cargo metadata --format-version 1 --locked]

## Architecture Patterns

### Recommended Project Structure

```text
crates/polint/src/
├── core/mod.rs          # Keep public Rule/RuleCtx/Capabilities plus narrow support view.
├── analysis_plan.rs     # Preferred new internal module if the planner splits core cleanly.
├── runner/mod.rs        # Child host owns plan construction and check/explain execution.
├── cli/mod.rs           # Parent CLI delegates explain plan to local rule hosts.
├── go/adapter.rs        # Accept &AnalysisPlan before harvesting/caching Go facts.
├── ts/adapter.rs        # Accept &AnalysisPlan before harvesting/caching TS/JS facts.
└── cache/{mod,keys}.rs  # Add plan_digest participation in cache keys.

crates/polint/tests/cli.rs  # Temp-repo and parent/child delegation tests.
docs/facts/README.md        # Add capability support/explain-plan docs if public behavior changes.
```

This structure follows the existing lib boundary where only `runner`, `sdk`, and `run_main` are public, while `core`, `cache`, `go`, `ts`, and `cli` are crate-private. [VERIFIED: crates/polint/src/lib.rs:1]

### Pattern 1: Internal Plan Plus Public Support View

**What:** Keep `AnalysisPlan` and `LanguagePlan` internal, but expose only a small read-only support view through `RuleCtx`. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

**When to use:** Use the full plan for orchestration, adapter gating, setup probes, diagnostics, and cache digests; use the `RuleCtx` view only when rule authors need to know whether a requested capability is supported, unsupported, or missing setup. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

**Recommended support matrix for Phase 11:**

| Capability | Phase 11 Status | Adapter / Fact State |
|------------|-----------------|----------------------|
| `syntax` | supported | File loading, parser diagnostics, packages/functions are existing baseline facts. [VERIFIED: crates/polint/src/core/mod.rs:603, crates/polint/src/go/adapter.rs:152, crates/polint/src/ts/adapter.rs:159] |
| `imports` | supported | Go and TS/JS syntactic import facts and `RuleCtx::imports` exist. [VERIFIED: crates/polint/src/core/mod.rs:778, crates/polint/src/go/adapter.rs:286, crates/polint/src/ts/adapter.rs:251] |
| `go_tests` | supported for Go | Go `TestFact` and `RuleCtx::go_tests` exist. [VERIFIED: crates/polint/src/core/mod.rs:168, crates/polint/src/core/mod.rs:832] |
| `branch_obligations` | supported for Go | Branch obligations exist and are documented as syntax-only/heuristic. [VERIFIED: crates/polint/src/core/mod.rs:157, docs/facts/branches.md] |
| `ts_components` | supported for TS/JS | TS component facts and `RuleCtx::ts_components` exist. [VERIFIED: crates/polint/src/core/mod.rs:192, crates/polint/src/core/mod.rs:901] |
| `ts_classes` | supported for TS/JS | TS class facts and `RuleCtx::ts_classes` exist. [VERIFIED: crates/polint/src/core/mod.rs:200, crates/polint/src/core/mod.rs:917] |
| `string_literals` | supported for Go and TS/JS | String literal facts and `RuleCtx::string_literals` exist. [VERIFIED: crates/polint/src/core/mod.rs:209, crates/polint/src/core/mod.rs:930] |
| `jsx_attributes` | supported for TSX/JSX | JSX attribute facts and `RuleCtx::jsx_attributes` exist. [VERIFIED: crates/polint/src/core/mod.rs:217, crates/polint/src/core/mod.rs:946] |
| `cfg` | unsupported / reserved | The capability is public, but roadmap assigns real CFG facts to Phase 12. [VERIFIED: crates/polint/src/core/mod.rs:608, .planning/ROADMAP.md] |
| `call_graph` | unsupported / reserved | Current `FunctionFact::calls` is syntactic text, not a resolved call graph. [VERIFIED: crates/polint/src/core/mod.rs:610, docs/facts/functions.md] |
| `coverage_facts` | unsupported / reserved | `CoverageFact` exists, but external coverage import is deferred to Phase 13 and no `RuleCtx` coverage accessor exists. [VERIFIED: crates/polint/src/core/mod.rs:185, .planning/ROADMAP.md] |
| `test_suite_metrics` | unsupported / reserved for normalized metrics | Rich test metrics are deferred to Phase 17; current Go test evidence should be requested through `go_tests`. [VERIFIED: crates/polint/src/core/mod.rs:618, .planning/ROADMAP.md] |

### Pattern 2: Build Plan From Enabled Rules Before Loading Files

**What:** In `runner::analyze_and_run`, load config, select profile patterns, resolve per-rule options, build `AnalysisPlan`, then call file discovery/adapters/rules. [VERIFIED: crates/polint/src/runner/mod.rs:138]

**Why:** The current child runner computes enabled patterns and rule options before loading files and calling adapters, which is the correct insertion point for plan construction. [VERIFIED: crates/polint/src/runner/mod.rs:147]

**Required detail:** The plan builder must apply the same enabled-rule pattern semantics as `run_rules` and `rule_hash`, including exact IDs, `prefix/*`, and `*`. [VERIFIED: crates/polint/src/core/mod.rs:1111, crates/polint/src/cache/keys.rs:33]

### Pattern 3: Parent Delegates Explain Plan To Child Host

**What:** Add `ExplainCommand::Plan(ExplainPlanArgs)` to the parent and a matching `Command::Explain { Plan }` path to `polint-local-rules`. [VERIFIED: crates/polint/src/cli/mod.rs:117, crates/polint/src/runner/mod.rs:15]

**Why:** The parent discovers rule host manifests but has an empty `Vec<Arc<dyn Rule>>` in its own analysis path, while the child host receives the registered local rule vector from `polint::runner::run_cli`. [VERIFIED: crates/polint/src/cli/mod.rs:606, crates/polint/src/runner/mod.rs:78]

**Multiple hosts:** `discover_local_rule_hosts` already returns a sorted set of manifests, so parent JSON should be deterministic across multiple rule hosts by wrapping child plans in manifest order or rejecting ambiguous multi-host output with a clear error. [VERIFIED: crates/polint/src/cli/mod.rs:840]

### Pattern 4: Plan Digest As First-Class Cache Input

**What:** Compute `plan_digest = stable_hash(deterministic_plan_parts)` and pass it to adapters and `CacheKey::for_file`. [VERIFIED: crates/polint/src/cache/mod.rs:36, crates/polint/src/cache/keys.rs:1]

**Why:** The current cache key changes on file content, config hash, rule hash, cache version, and adapter schema; Phase 11 needs the same invalidation guarantee for requested capabilities and setup-sensitive inputs. [VERIFIED: crates/polint/src/cache/mod.rs:52, .planning/REQUIREMENTS.md]

**Recommended implementation:** Add a `plan_hash` field to `CacheKey`, include it in `stable_id`, and update Go/TS adapter entrypoints to accept `&AnalysisPlan` or `plan_digest: &str`. [VERIFIED: crates/polint/src/cache/mod.rs:10, crates/polint/src/go/adapter.rs:39, crates/polint/src/ts/adapter.rs:46]

### Pattern 5: Explain Plan Must Not Parse Source Files

**What:** `polint explain plan` should load config and local-rule registrations, select enabled rules, build the plan, run setup probes, and render output without calling `load_analysis_files`, `go::analyze_with_options`, or `ts::analyze_with_options`. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src/cli/mod.rs:653]

**Why:** Existing `explain go-test` parses Go files by design, but the locked Phase 11 decision explicitly says plan explanation should not parse files by default. [VERIFIED: crates/polint/src/cli/mod.rs:653, .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

### Anti-Patterns to Avoid

- **Parent-only plan construction:** The parent currently has no local rule vector, so parent-only planning cannot satisfy PLAN-01 for repo-local rules. [VERIFIED: crates/polint/src/cli/mod.rs:606, crates/polint/src/runner/mod.rs:78]
- **Exposing `AnalysisPlan` through `polint::sdk::prelude::*`:** This contradicts D-01 and the public API visibility plan. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, docs/API-VISIBILITY-PLAN.md]
- **Hard-gating all current fact families immediately:** This contradicts D-07/D-08 and can break existing rules that read current facts without perfect capability declarations. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]
- **Accepting `cfg`, `call_graph`, `coverage_facts`, or `test_suite_metrics` as supported:** These are reserved/future capabilities in the current roadmap and docs. [VERIFIED: .planning/ROADMAP.md, docs/CAPABILITY-FULFILLMENT-RESEARCH.md]
- **Rendering child explain JSON with human prelude text:** Existing JSON report parsing expects child stdout to be pure JSON, and Phase 8 already protected machine output from human prelude contamination. [VERIFIED: crates/polint/src/cli/mod.rs:941, .planning/STATE.md]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Plan hash algorithm | A new hashing function | Existing `crate::cache::stable_hash` and deterministic encoders | Cache code already centralizes stable hashing and tests cache-key invalidation. [VERIFIED: crates/polint/src/cache/mod.rs:137] |
| JSON output | String-concatenated JSON | Typed `Serialize` structs plus `serde_json::to_string_pretty` | Diagnostics JSON already uses typed serde wire structs. [VERIFIED: crates/polint/src/diagnostics/mod.rs:302] |
| Child host command execution | Shell command strings | `std::process::Command` with explicit args | Existing local rule host delegation uses `ProcessCommand` and arg arrays, avoiding shell interpolation. [VERIFIED: crates/polint/src/cli/mod.rs:889] |
| Rule selection semantics | A second wildcard matcher | Existing `core::rule_id_matches` | Existing selection is shared by `run_rules` and cache rule hashing. [VERIFIED: crates/polint/src/core/mod.rs:1111, crates/polint/src/cache/keys.rs:33] |
| Future facts | Placeholder CFG, coverage, call graph, symbol, or metrics facts | Unsupported/setup diagnostics plus roadmap docs | The milestone explicitly defers those facts to later phases. [VERIFIED: .planning/ROADMAP.md, .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] |
| Plan explanation through analysis | Reusing `check`/adapter parsing to infer plan state | Config/rule loading plus setup probes only | `explain plan` must not parse source files by default. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] |

**Key insight:** The planner should treat capability planning as orchestration and truthfulness plumbing, not as the first implementation of deferred fact families. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, docs/CAPABILITY-FULFILLMENT-RESEARCH.md]

## Common Pitfalls

### Pitfall 1: Plan Built Where Rules Are Not Registered
**What goes wrong:** Parent `polint explain plan` shows an empty or misleading plan even when repo-local rules exist. [VERIFIED: crates/polint/src/cli/mod.rs:606]
**Why it happens:** Parent CLI currently discovers manifests and delegates `check`, while real `Arc<dyn Rule>` registration happens inside `polint-local-rules`. [VERIFIED: crates/polint/src/cli/mod.rs:852, crates/polint/src/runner/mod.rs:78]
**How to avoid:** Add a child-host `explain plan` command and delegate from the parent when manifests exist. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]
**Warning signs:** A test with a generated local rule requesting `.string_literals()` produces zero requested capabilities in parent JSON. [VERIFIED: crates/polint/tests/cli.rs:420]

### Pitfall 2: Capability Changes Reuse Stale Cache Entries
**What goes wrong:** A rule changes capabilities but adapters restore facts from old cache keys. [VERIFIED: crates/polint/src/cache/mod.rs:52]
**Why it happens:** Current cache keys include `rule_hash`, but current `rule_hash` includes rule metadata/options and not `rule.capabilities()`. [VERIFIED: crates/polint/src/cache/keys.rs:25]
**How to avoid:** Include `plan_digest` in `CacheKey::stable_id` or fold it into the adapter hash path, and add tests that capability-only changes alter cache filenames. [VERIFIED: .planning/REQUIREMENTS.md]
**Warning signs:** Cache file count and filenames remain identical after changing a generated rule from `.syntax()` to `.string_literals()`. [VERIFIED: crates/polint/tests/cli.rs:1910]

### Pitfall 3: Uncaught `meta()` Or `capabilities()` Panics During Planning
**What goes wrong:** A malformed rule can crash before `run_rules` gets a chance to convert failure into an internal diagnostic. [VERIFIED: crates/polint/src/core/mod.rs:1049]
**Why it happens:** `run_rules` catches `meta()` and `run()` panics, but `rule_hash` currently calls `rule.meta()` directly. [VERIFIED: crates/polint/src/core/mod.rs:1049, crates/polint/src/cache/keys.rs:31]
**How to avoid:** Plan construction and plan hashing should catch `meta()`/`capabilities()` panics and emit `internal/<rule>` or `polint/capability` diagnostics instead of unwinding. [VERIFIED: AGENTS.md]
**Warning signs:** New plan tests pass for normal rules but fail when a rule panics in `capabilities()`. [VERIFIED: crates/polint/src/core/mod.rs:1887]

### Pitfall 4: Unsupported Future Capabilities Look Supported
**What goes wrong:** A rule requests `.cfg()` or `.coverage_facts()` and users infer those facts were computed. [VERIFIED: crates/polint/src/core/mod.rs:645, crates/polint/src/core/mod.rs:665]
**Why it happens:** The public `Capabilities` builder already has reserved methods before the fact families exist. [VERIFIED: crates/polint/src/core/mod.rs:608]
**How to avoid:** Mark reserved future capabilities as unsupported in the plan and emit deterministic diagnostics or structured warnings when requested. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]
**Warning signs:** `explain plan --format json` lists `cfg` with `"status": "supported"` before Phase 12 exists. [VERIFIED: .planning/ROADMAP.md]

### Pitfall 5: `explain plan` Accidentally Parses Source Files
**What goes wrong:** `polint explain plan` fails on syntax errors, becomes slow on large repos, or populates cache even though it is only explaining setup. [VERIFIED: crates/polint/src/cli/mod.rs:653]
**Why it happens:** Existing `check`, `graph`, and `explain go-test` paths call `load_analysis_files` and adapters. [VERIFIED: crates/polint/src/cli/mod.rs:611, crates/polint/src/cli/mod.rs:662, crates/polint/src/cli/mod.rs:791]
**How to avoid:** Keep plan explanation on a separate config/rule/setup-probe path. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]
**Warning signs:** A temp repo with invalid `.go` source causes `polint explain plan` to produce `parser/go`. [VERIFIED: crates/polint/src/go/adapter.rs:176]

### Pitfall 6: Deterministic JSON Uses Unordered Collections
**What goes wrong:** Agent/CI snapshots flake because capability rows or rules shift order between runs. [VERIFIED: crates/polint/src/diagnostics/mod.rs:322]
**Why it happens:** Parallelism and unordered maps can introduce nondeterministic ordering when not sorted. [VERIFIED: crates/polint/src/go/adapter.rs:52, crates/polint/src/ts/adapter.rs:59]
**How to avoid:** Use `BTreeMap`/`BTreeSet`, sorted `Vec`s, and stable manual encoders for plan digest and JSON arrays. [VERIFIED: crates/polint/src/cache/keys.rs:155] [CITED: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html]
**Warning signs:** Running `polint explain plan --format json` three times yields different byte output in the same temp repo. [VERIFIED: crates/polint/tests/cli.rs:1947]

## Code Examples

Verified patterns from current codebase, adapted for planning. [VERIFIED: crates/polint/src/cache/keys.rs, crates/polint/src/runner/mod.rs]

### Deterministic Plan Digest

```rust
// Source: crates/polint/src/cache/keys.rs deterministic encoder pattern.
pub(crate) fn plan_digest(plan: &AnalysisPlan) -> String {
    let mut parts = Vec::<String>::new();
    parts.push(format!("schema={}", encode_str("analysis-plan-v1")));
    for rule in &plan.rules {
        parts.push(format!("rule={}", encode_str(&rule.id)));
        parts.push(format!("capabilities={}", encode_capabilities(rule.capabilities)));
    }
    for support in &plan.support {
        parts.push(format!(
            "support={}:{}:{}",
            encode_str(support.capability.as_str()),
            encode_str(support.language.as_str()),
            encode_str(support.status.as_str())
        ));
    }
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}
```

Use length-prefixed strings or an equivalent delimiter-safe encoder because cache key encoders already avoid ambiguity between joined strings. [VERIFIED: crates/polint/src/cache/keys.rs:215, crates/polint/src/cache/keys.rs:257]

### Plan Construction In Child Runner

```rust
// Source: crates/polint/src/runner/mod.rs analyze_and_run ordering.
let loaded = load_config_for_check(root, &args.paths)?;
let enabled = selected_rule_patterns(&loaded, args.profile.as_deref())?;
let options = resolved_rule_options(&loaded, rules)?;
let plan = AnalysisPlan::from_rules(rules, enabled.as_ref(), &options)?;
let plan_digest = plan.digest();

let mut db = load_analysis_files(&loaded)?;
diagnostics.extend(crate::go::analyze_with_options(
    &mut db,
    &cache,
    &config_digest,
    &rule_digest,
    &plan_digest,
    &plan,
    true,
));
```

Build the plan before file loading and adapter execution because adapters must receive the plan before optional harvesting. [VERIFIED: .planning/REQUIREMENTS.md, crates/polint/src/runner/mod.rs:161]

### Capability Diagnostic Shape

```rust
// Source: crates/polint/src/diagnostics/mod.rs Diagnostic builders.
Diagnostic::error(
    "polint/capability",
    "<workspace>",
    TextRange::point(1, 1),
    "Rule `local/needs-cfg` requested unsupported capability `cfg`.",
)
.with_help("CFG facts are planned for Phase 12; see docs/facts/README.md.")
.with_evidence("capability", "cfg")
```

Use a distinct `polint/capability` or `polint/setup` rule ID family instead of parser IDs because setup/capability failures are not parser diagnostics. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src/diagnostics/mod.rs:175]

### Parent Delegation Pattern

```rust
// Source: crates/polint/src/cli/mod.rs run_local_rule_host pattern.
let mut command = ProcessCommand::new(&cargo);
command.current_dir(root).args([
    "run",
    "--quiet",
    "--manifest-path",
    manifest_str,
    "--",
    "explain",
    "plan",
    "--format",
    "json",
]);
```

Keep command construction argument-based and avoid shell strings because current local-host delegation already follows that pattern. [VERIFIED: crates/polint/src/cli/mod.rs:889]

## State of the Art

| Old Approach | Current Approach | When Changed / Scope | Impact |
|--------------|------------------|----------------------|--------|
| Capabilities are descriptive metadata only. | Capabilities should become a deterministic analysis plan before adapters run. | Phase 11 scope. [VERIFIED: docs/CAPABILITY-FULFILLMENT-RESEARCH.md, .planning/ROADMAP.md] | Enables setup checks, adapter gating, cache correctness, and explainability. [VERIFIED: .planning/REQUIREMENTS.md] |
| Go and TS/JS adapters harvest standard facts regardless of rule declarations. | Keep compatibility for current facts, but gate future/expensive facts where safe. | Locked D-07/D-08. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] | Avoids breaking existing rule packs while making future analyzers truthful. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] |
| Cache invalidation is file/config/rule/schema based. | Cache invalidation must include plan/support inputs once plan affects facts. | PLAN-03. [VERIFIED: crates/polint/src/cache/mod.rs, .planning/REQUIREMENTS.md] | Prevents stale facts when requested capabilities change. [VERIFIED: .planning/REQUIREMENTS.md] |
| `explain` covers rule stubs and Go test facts. | `explain plan` should show enabled rules, requested capabilities, support/setup status, and digest without parsing files. | Phase 11 D-13 through D-15. [VERIFIED: crates/polint/src/cli/mod.rs:641, .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] | Gives agents and CI a cheap plan inspection surface. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] |

**Relevant external reference pattern:** Go `packages.Config.Mode` controls how much package information is loaded, which is a useful analogy for capability-style analysis planning, but Phase 11 should not add Go semantic loading. [CITED: https://pkg.go.dev/golang.org/x/tools/go/packages] [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

**Relevant external reference pattern:** Oxc semantic analysis exposes symbols and references through semantic APIs, but Phase 11 must not implement symbol/reference facts before their owning phase. [CITED: https://docs.rs/oxc_semantic/latest/oxc_semantic/] [VERIFIED: .planning/ROADMAP.md]

**Deprecated/outdated:**
- Treating `Capabilities` comments as the whole contract is outdated for v1.1 because PLAN-01 through PLAN-04 require operational planning, cache participation, and diagnostics. [VERIFIED: .planning/REQUIREMENTS.md, docs/CAPABILITY-FULFILLMENT-RESEARCH.md]
- Treating unsupported reserved capability requests as empty facts is outdated because D-11 requires clear failure or deterministic diagnostics. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | A distinct `plan_hash` field is the preferred cache-key shape rather than folding the digest into `rule_hash`. [ASSUMED] | Standard Stack / Architecture Patterns | If the implementer chooses fold-in instead, tests must still prove cache invalidation and explain output must still expose the plan digest. |

## Open Questions

1. **Should `test_suite_metrics` be treated as unsupported or as an alias for current Go `TestFact` aggregate fields?** [VERIFIED: crates/polint/src/core/mod.rs:618, .planning/ROADMAP.md]
   - What we know: Rich test-suite metrics are Phase 17, while current Go `TestFact` already stores assertion/subtest/table-row evidence. [VERIFIED: crates/polint/src/core/mod.rs:168, .planning/ROADMAP.md]
   - What's unclear: The existing `Capabilities::test_suite_metrics()` comment says aggregate-like Go metrics are currently stored on `TestFact`, but the v1.1 roadmap treats normalized metrics as future work. [VERIFIED: crates/polint/src/core/mod.rs:618, .planning/ROADMAP.md]
   - Recommendation: In Phase 11, mark `test_suite_metrics` as unsupported/reserved for normalized metrics and point users to `go_tests` for current Go test evidence. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| `cargo` | Building/running local rule hosts and validation | yes | 1.95.0 | None needed. [VERIFIED: cargo --version] |
| `rustc` | Workspace MSRV and local rule-host compilation | yes | 1.95.0 | None needed. [VERIFIED: rustc --version, Cargo.toml] |
| `cargo clippy` | Workspace lint gate | yes | 0.1.95 | None needed. [VERIFIED: cargo clippy --version] |
| `rustfmt` | Formatting gate | yes | 1.9.0-stable | None needed. [VERIFIED: rustfmt --version] |
| `node` | GSD tooling only, not product implementation | yes | v20.19.4 | Not required for Rust implementation. [VERIFIED: node --version] |
| `jq` | Research/version extraction only | yes | jq-1.8.1 | Use `cargo metadata` output directly. [VERIFIED: jq --version] |
| Global `polint` binary | Manual smoke only | yes | 0.1.6 | Use `cargo run -p polint -- ...` or `assert_cmd::cargo_bin("polint")` because workspace crate version is 0.1.7. [VERIFIED: polint --version, Cargo.toml] |

**Missing dependencies with no fallback:** None found for Phase 11 planning and validation. [VERIFIED: environment audit commands]

**Missing dependencies with fallback:** Global `polint` is older than the workspace, so planner tasks should prefer Cargo-built binaries. [VERIFIED: polint --version, Cargo.toml]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` with `assert_cmd`, `predicates`, `tempfile`, `serde_json`, `proptest`, and `insta`. [VERIFIED: Cargo.toml, cargo metadata --locked] |
| Config file | Root `Cargo.toml` plus `Makefile` commands. [VERIFIED: Cargo.toml, Makefile] |
| Quick run command | `cargo test -p polint --lib analysis_plan --locked` after new plan tests are named with `analysis_plan`. [VERIFIED: cargo test -p polint --lib -- --list] |
| CLI quick run command | `cargo test -p polint --test cli explain_plan --locked` after new CLI tests are named with `explain_plan`. [VERIFIED: cargo test -p polint --test cli -- --list] |
| Full suite command | `cargo test --workspace --all-features --locked`. [VERIFIED: Makefile] |
| Lint command | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`. [VERIFIED: Makefile] |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| PLAN-01 | Enabled rules merge into deterministic plan and `polint explain plan --format json` shows rule IDs/capabilities. [VERIFIED: .planning/REQUIREMENTS.md] | unit + CLI integration | `cargo test -p polint --lib analysis_plan --locked && cargo test -p polint --test cli explain_plan --locked` | no, Wave 0. [VERIFIED: cargo test -- --list] |
| PLAN-02 | Go and TS/JS adapters receive resolved plan before harvesting. [VERIFIED: .planning/REQUIREMENTS.md] | unit adapter signature + integration smoke | `cargo test -p polint --lib adapter_receives_plan --locked` | no, Wave 0. [VERIFIED: crates/polint/src/go/adapter.rs, crates/polint/src/ts/adapter.rs] |
| PLAN-03 | Cache keys change when requested capabilities or setup-sensitive inputs change. [VERIFIED: .planning/REQUIREMENTS.md] | unit + CLI cache integration | `cargo test -p polint --lib plan_digest --locked && cargo test -p polint --test cli capability_change_changes_cache --locked` | no, Wave 0. [VERIFIED: crates/polint/src/cache/mod.rs tests list, crates/polint/tests/cli.rs] |
| PLAN-04 | Unsupported future capability and missing setup emit deterministic diagnostics or structured warnings. [VERIFIED: .planning/REQUIREMENTS.md] | unit + CLI integration | `cargo test -p polint --test cli unsupported_capability --locked` | no, Wave 0. [VERIFIED: cargo test -p polint --test cli -- --list] |

### Sampling Rate

- **Per task commit:** Run the most specific unit or CLI filter for the edited surface, such as `cargo test -p polint --lib analysis_plan --locked` or `cargo test -p polint --test cli explain_plan --locked`. [VERIFIED: cargo test -- --list]
- **Per wave merge:** Run `cargo test -p polint --lib --locked` and `cargo test -p polint --test cli --locked`. [VERIFIED: cargo test -- --list]
- **Phase gate:** Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo test --workspace --all-features --locked`. [VERIFIED: Makefile]

### Wave 0 Gaps

- [ ] `crates/polint/src/analysis_plan.rs` or equivalent tests in `core/mod.rs` covering deterministic plan merge, enabled-pattern filtering, support statuses, and digest stability. [VERIFIED: current file tree, crates/polint/src/core/mod.rs]
- [ ] `crates/polint/src/cache/keys.rs` tests proving plan digest changes cache IDs. [VERIFIED: crates/polint/src/cache/keys.rs tests list]
- [ ] `crates/polint/tests/cli.rs` tests for parent delegation, child host JSON, empty valid plan with no rules, no source parsing by default, unsupported capability diagnostics, and capability-change cache invalidation. [VERIFIED: crates/polint/tests/cli.rs tests list]
- [ ] Adapter tests for Go and TS/JS plan-aware cache participation. [VERIFIED: crates/polint/src/go/tests.rs, crates/polint/src/ts/tests.rs]

## Security Domain

Security enforcement is enabled because `.planning/config.json` does not set `security_enforcement` to `false`. [VERIFIED: .planning/config.json]

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase 11 adds local CLI planning, not user authentication. [VERIFIED: .planning/ROADMAP.md, crates/polint/src/cli/mod.rs] |
| V3 Session Management | no | Phase 11 has no sessions or web state. [VERIFIED: .planning/ROADMAP.md] |
| V4 Access Control | no | Phase 11 delegates to configured local rule hosts and does not add remote authorization. [VERIFIED: crates/polint/src/cli/mod.rs:840] |
| V5 Input Validation | yes | Validate CLI args through `clap`, config through TOML deserialization/globs, and child-host JSON through typed parsing. [VERIFIED: crates/polint/src/cli/mod.rs:76, crates/polint/src/config/mod.rs:177, crates/polint/src/diagnostics/mod.rs:316] |
| V6 Cryptography | no | Phase 11 uses non-cryptographic stable hashes for cache invalidation, not security tokens or cryptographic verification. [VERIFIED: crates/polint/src/cache/mod.rs:137] |

### Known Threat Patterns for Rust CLI / Local Rule Host

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Shell injection through local rule host invocation | Tampering / Elevation of Privilege | Continue using `ProcessCommand` with explicit args and no shell string. [VERIFIED: crates/polint/src/cli/mod.rs:889] |
| Machine-readable output contaminated by human text | Tampering / Repudiation | Keep `--format json` stdout deterministic and parseable; send errors to stderr or structured JSON where appropriate. [VERIFIED: crates/polint/src/cli/mod.rs:941, crates/polint/src/diagnostics/mod.rs:400] |
| Untrusted local rule code execution | Elevation of Privilege | Treat repo-local rules as intentionally executed local code, keep behavior explicit in docs, and do not make `explain plan` parse source or execute extra setup beyond probes. [VERIFIED: AGENTS.md, crates/polint/src/runner/mod.rs:78] |
| Path/config injection through docs path or setup hint | Spoofing / Tampering | Use static docs paths and structured capability names rather than echoing untrusted strings into shell commands. [VERIFIED: crates/polint/src/diagnostics/mod.rs:210, crates/polint/src/cli/mod.rs:889] |
| Unsupported capability silently producing empty facts | Tampering / Information Disclosure by omission | Emit deterministic `polint/capability` diagnostics or structured warnings for unsupported requests. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md] |

## Sources

### Primary (HIGH confidence)

- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - locked Phase 11 decisions, discretion, deferred scope, and code context. [VERIFIED: file read]
- `.planning/REQUIREMENTS.md` - PLAN-01 through PLAN-04. [VERIFIED: file read]
- `.planning/ROADMAP.md` - Phase 11 success criteria and future capability ownership phases. [VERIFIED: file read]
- `.planning/STATE.md` - project status, prior decisions, deterministic output and truthfulness requirements. [VERIFIED: file read]
- `AGENTS.md` - project stack, public API discipline, rule authoring platform contract, and GSD workflow enforcement. [VERIFIED: file read]
- `docs/CAPABILITY-FULFILLMENT-RESEARCH.md` - capability planning build method and adapter contract. [VERIFIED: file read]
- `docs/roadmap/01_ENTRY_1_ANALYSIS_PLAN.md` - human technical roadmap for AnalysisPlan. [VERIFIED: file read]
- `crates/polint/src/core/mod.rs` - `Capabilities`, `Rule`, `RuleCtx`, `AnalysisDb`, fact accessors, and `run_rules`. [VERIFIED: codebase grep]
- `crates/polint/src/runner/mod.rs` - child local-rule host command flow and adapter invocation order. [VERIFIED: codebase grep]
- `crates/polint/src/cli/mod.rs` - parent CLI, existing explain commands, local rule host delegation, and no-rule parent analysis path. [VERIFIED: codebase grep]
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - adapter signatures, cache lookup, parser diagnostics, and fact harvesting. [VERIFIED: codebase grep]
- `crates/polint/src/cache/mod.rs` and `crates/polint/src/cache/keys.rs` - cache-key fields, stable hashing, deterministic encoders, and cache tests. [VERIFIED: codebase grep]
- `crates/polint/tests/cli.rs` - temp-repo CLI patterns, existing explain tests, and cache determinism tests. [VERIFIED: codebase grep]
- `Cargo.toml`, `crates/polint/Cargo.toml`, and `cargo metadata --format-version 1 --locked` - workspace versions and dependencies. [VERIFIED: cargo metadata --locked]

### Secondary (MEDIUM confidence)

- Rust `BTreeMap` docs - key-order iteration guarantee used to justify ordered maps for deterministic output. [CITED: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html]
- clap derive docs - derive API support for `Parser`, `Args`, `Subcommand`, and `ValueEnum`. [CITED: https://docs.rs/clap/latest/clap/_derive/]
- Go `packages` docs - external analogy for capability-style load modes, not a Phase 11 implementation dependency. [CITED: https://pkg.go.dev/golang.org/x/tools/go/packages]
- Oxc semantic docs - external reference for future semantic/symbol phases, not a Phase 11 implementation dependency. [CITED: https://docs.rs/oxc_semantic/latest/oxc_semantic/]

### Tertiary (LOW confidence)

- None. [VERIFIED: all phase-critical claims sourced from project files, codebase grep, Cargo metadata, or official docs]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new libraries are required and current versions were verified from locked Cargo metadata. [VERIFIED: cargo metadata --locked]
- Architecture: HIGH - runner, CLI, adapter, cache, and SDK boundaries were verified in source. [VERIFIED: crates/polint/src]
- Pitfalls: HIGH - each pitfall maps to an existing code path or locked Phase 11 decision. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md, crates/polint/src]
- Public support-view naming: MEDIUM - the need for a narrow view is locked, but exact type names are delegated to implementer discretion. [VERIFIED: .planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md]

**Research date:** 2026-05-09 [VERIFIED: current_date]
**Valid until:** 2026-06-08 for codebase-specific findings; re-check Cargo metadata and source signatures if dependencies or local-rule-host architecture changes. [VERIFIED: Cargo.toml, crates/polint/src/runner/mod.rs]
