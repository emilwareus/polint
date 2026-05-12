---
phase: 12-resolved-imports-and-module-relationships
plan: "02"
subsystem: core-analysis
tags: [rust, module-graph, resolved-imports, capabilities, runner]

requires:
  - phase: 12-01
    provides: core relationship fact records, SDK views, macro capability derivation, and initial unsupported planner rows
provides:
  - Crate-private module graph provider invoked before rule execution
  - Deterministic module/file/package/external graph builder and lexical path normalization helpers
  - Conservative TS/JS and Go resolver contracts that preserve uncertainty as facts
  - Provider-derived setup-missing capability support overrides and diagnostics
  - Supported analysis-plan rows for resolved_imports and module_graph
affects: [12-03, 12-04, module-graph-provider, capability-planning, runner]

tech-stack:
  added: []
  patterns:
    - project-wide derived fact provider runs after syntax adapters and before metrics/rules
    - resolver adapters return small crate-private drafts instead of mutating AnalysisDb
    - provider support overrides merge with static AnalysisPlan support before rule execution

key-files:
  created:
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/module_graph/model.rs
    - crates/polint/src/module_graph/paths.rs
    - crates/polint/src/module_graph/query.rs
    - crates/polint/src/module_graph/ts.rs
    - crates/polint/src/module_graph/go.rs
    - .planning/phases/12-resolved-imports-and-module-relationships/12-02-SUMMARY.md
  modified:
    - crates/polint/src/lib.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/src/analysis_plan.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Run module graph derivation after Go and TS/JS syntax analysis and before derived metrics or rule execution."
  - "Keep TS/JS and Go resolver outputs as crate-private drafts; public facts expose only polint-owned IDs and status enums."
  - "Do not synthesize a root module node for an empty repository; empty relationship views stay empty."
  - "Provider-derived setup-missing support rows emit their own capability diagnostics before rules are blocked."

patterns-established:
  - "ModuleGraphBuilder owns deterministic BTreeMap indexes and sorted output vectors for nodes and edges."
  - "ResolverInput carries root, AnalysisDb, import, and owner node context without exposing resolver metadata publicly."
  - "ModuleGraphDerivation::support_view overlays provider support rows on the static plan support view."

requirements-completed: [MOD-01, MOD-04]

duration: 1h 1m
completed: 2026-05-11
---

# Phase 12 Plan 02: Module Graph Provider Summary

**Project-wide module graph derivation with deterministic relationship facts and setup-aware capability support**

## Performance

- **Duration:** 1h 1m
- **Started:** 2026-05-11T14:31:17Z
- **Completed:** 2026-05-11T15:33:05Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments

- Added `crates/polint/src/module_graph/` with provider orchestration, deterministic builder/indexing, lexical repo-relative path normalization, direct graph query helpers, and conservative TS/Go resolver boundaries.
- Registered the provider in `lib.rs` and wired it into both local-rule-host and parent/no-host analysis paths after syntax adapters and before metrics/rule execution.
- Switched `resolved_imports` and `module_graph` from static `Unsupported` planner rows to `Supported`, while allowing provider-derived setup-missing rows to block requesting rules with capability diagnostics.
- Added CLI proof that relationship-view rules run on an empty repo and that Go setup-missing support blocks rule execution instead of running with placeholder facts.

## Task Commits

1. **Task 1 RED: provider and builder behavior tests** - `49de6ab` (test)
2. **Task 1 GREEN: deterministic provider foundation** - `40fac5d` (feat)
3. **Task 2 RED: resolver contract tests** - `d3ee29a` (test)
4. **Task 2 GREEN: conservative resolver contracts** - `717eebf` (feat)
5. **Task 3 RED: provider wiring tests** - `41d97a7` (test)
6. **Task 3 GREEN: provider lifecycle wiring** - `a7b6e7e` (feat)

## Files Created/Modified

- `crates/polint/src/module_graph/mod.rs` - Provider entrypoint, derivation output, support-view overlay, setup-missing diagnostics, and provider tests.
- `crates/polint/src/module_graph/model.rs` - Deterministic builder, resolver input/draft contracts, node draft helpers, and graph output model.
- `crates/polint/src/module_graph/paths.rs` - Lexical repo-relative path normalization with `..` escape rejection and no canonicalization.
- `crates/polint/src/module_graph/query.rs` - Deterministic outgoing, incoming, and BFS reachability helpers.
- `crates/polint/src/module_graph/ts.rs` - Conservative TS-family resolver returning `Unresolved/NotFound`.
- `crates/polint/src/module_graph/go.rs` - Typed `GoPackageIndex` boundary and conservative Go resolver returning `SetupMissing/SetupMissing` when metadata is absent.
- `crates/polint/src/lib.rs` - Registered the crate-private `module_graph` module.
- `crates/polint/src/runner/mod.rs` - Runs module graph derivation before metrics and passes merged support to rule execution.
- `crates/polint/src/cli/mod.rs` - Runs the provider on the parent/no-local-rule path for lifecycle consistency.
- `crates/polint/src/analysis_plan.rs` - Marks `resolved_imports` and `module_graph` as supported capability rows.
- `crates/polint/tests/cli.rs` - Adds external-consumer style capability planning coverage for relationship views and setup-missing blocking.

## Decisions Made

- The provider creates no synthetic root module for a truly empty repo, so `ModuleGraphFacts` can honestly expose empty node/edge vectors.
- Go metadata remains a typed empty `GoPackageIndex` boundary in this plan; Plan 12-04 will populate it from Go package metadata.
- TS/JS resolution is intentionally conservative in this plan; Plan 12-03 can replace the resolver body without changing the provider/builder contract.
- Provider-derived setup-missing rows are not plan diagnostics, so the provider emits matching `polint/capability` diagnostics itself before rule execution is blocked.

## Verification

- `cargo test -p polint --lib module_graph --locked`
- `cargo test -p polint --lib module_graph_resolver_contracts --locked`
- `cargo test -p polint --lib analysis_plan_supports_module_relationship_capabilities --locked`
- `cargo test -p polint --test cli capability_planning --locked`
- `cargo fmt --all -- --check`
- Structural `rg` checks for provider registration/calls, support-view merge, supported capability rows, module-node APIs, deterministic builder patterns, resolver contracts, absence of `canonicalize`, absence of public resolver traits, and no public module graph CLI command.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

- The first provider support override blocked rules silently because `run_rules_with_capability_support` only blocks and does not create diagnostics. The provider now emits setup-missing capability diagnostics with the support override.
- The initial empty-repo CLI proof exposed that an unconditional root module node made empty graph views non-empty. The provider now only creates a root module when there are files, package facts, or imports to relate.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Stub-pattern scan hits only existing CLI fixture literals such as `TODO` and intentional TOML empty arrays; the conservative TS/Go resolver behavior is this plan's documented scope and is handed to Plans 12-03 and 12-04.

## Next Phase Readiness

Plan 12-03 can replace the conservative TS/JS resolver with `oxc_resolver` behavior through `module_graph::ts` without changing public SDK contracts. Plan 12-04 can fill `GoPackageIndex` from Go metadata and emit local/external Go dependency targets through the same draft path.

## Self-Check: PASSED

- Found `.planning/phases/12-resolved-imports-and-module-relationships/12-02-SUMMARY.md`.
- Found all key module graph provider, resolver, runner, CLI, and planner files.
- Found task commits `49de6ab`, `40fac5d`, `d3ee29a`, `717eebf`, `41d97a7`, and `a7b6e7e`.

---
*Phase: 12-resolved-imports-and-module-relationships*
*Completed: 2026-05-11*
