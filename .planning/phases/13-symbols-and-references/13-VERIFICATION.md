---
phase: 13-symbols-and-references
verified: 2026-05-13T08:24:38Z
status: passed
score: 29/29 must-haves verified
overrides_applied: 0
---

# Phase 13: Symbols and References Verification Report

**Phase Goal:** Expose stable definitions, symbols, and references.
**Verified:** 2026-05-13T08:24:38Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Public symbol/reference fact types and IDs are available through the SDK. | VERIFIED | `SymbolId`, `DefinitionId`, `ReferenceId`, `SymbolFact`, `DefinitionFact`, `ReferenceFact`, precision/status enums exist in `crates/polint/src/core/mod.rs:46-430`; prelude exports them in `crates/polint/src/sdk/mod.rs:27-52`. |
| 2 | Go symbols and references are populated from typed package information where setup exists. | VERIFIED | Go sidecar uses `packages.Load`, `TypesInfo.Defs`, `Uses`, `Selections`, `Implicits`, and `objectpath` in `tools/polint-go-symbols/internal/symbols/emit.go:149-182`; Rust converts sidecar rows through `SymbolGraphBuilder` in `crates/polint/src/symbol_graph/go.rs:461-581`. |
| 3 | TS/JS symbols and references are populated from Oxc semantic facts where setup exists. | VERIFIED | TS provider builds `SemanticBuilder`, reads Oxc scoping symbols and references, emits unresolved root references, and links imports through module graph in `crates/polint/src/symbol_graph/ts.rs:169-423`. |
| 4 | A typed SDK view exposes references for supported symbols. | VERIFIED | `References<'_>` exposes `all`, `iter`, `to`, `for_file`, `unresolved`, and `ambiguous` in `crates/polint/src/sdk/facts.rs:408-443`. |
| 5 | Precision tiers distinguish exact, heuristic, unresolved, and ambiguous facts. | VERIFIED | `SymbolPrecision` includes `ExactSemantic`, `ExactLocal`, `ModuleLinked`, `Heuristic`, `Unresolved`, `Ambiguous`, `SetupMissing`, `Unsupported`; `SymbolResolutionStatus` includes resolved/uncertain states in `crates/polint/src/core/mod.rs:355-376`. |
| 6 | Rule authors can request `Symbols<'_>` and `References<'_>` through `#[polint::rule]` signatures. | VERIFIED | Macro maps canonical `Symbols` and `References` parameters to `symbols`/`references` capabilities in `crates/polint-macros/src/lib.rs:318-324`, with tests at `:441-461`. |
| 7 | Core facts distinguish symbols, definitions, and references instead of treating declarations as references. | VERIFIED | Separate fact structs and separate `AnalysisDb` storage vectors/indexes exist in `crates/polint/src/core/mod.rs:379-430` and `:535-543`. |
| 8 | `SymbolFact`, `DefinitionFact`, and `ReferenceFact` carry stable polint-owned IDs plus precision and resolution status. | VERIFIED | All facts carry stable ID newtypes and `stable_key`; symbol/definition facts carry precision; reference facts carry `status` and `precision` in `crates/polint/src/core/mod.rs:379-430` and `crates/polint/src/symbol_graph/model.rs:484-509`. |
| 9 | Public prelude exposes only normalized polint facts and views, not Oxc, Go, sidecar, or raw AST internals. | VERIFIED | Prelude exports normalized core facts and SDK views in `crates/polint/src/sdk/mod.rs:27-52`; grep found no public Oxc/Go/sidecar references in SDK facts/prelude beyond prose saying not exposed. |
| 10 | Provider code can derive stable `SymbolId`, `DefinitionId`, and `ReferenceId` values from semantic keys. | VERIFIED | Stable key builders and ID conversion helpers exist in `crates/polint/src/symbol_graph/stable_id.rs:35-180`; builder calls them in `crates/polint/src/symbol_graph/model.rs:122-160`. |
| 11 | Symbol graph output is deterministic across insertion order and repeated runs. | VERIFIED | Builder stages facts in `BTreeMap`s and sorts output in `crates/polint/src/symbol_graph/model.rs:22-30` and `:246-280`; deterministic tests exist at `:771-822`. |
| 12 | Stable key collisions and ID churn can be diagnosed without exposing raw parser objects. | VERIFIED | Collision/duplicate diagnostics are emitted as `polint/internal` with IDs and stable keys in `crates/polint/src/symbol_graph/model.rs:347-385`; stable keys omit raw source and transient `FileId` in `stable_id.rs:266-276`. |
| 13 | Engine derives module relationships when symbols or references need module context. | VERIFIED | Module graph trigger capabilities include `symbols` and `references` in `crates/polint/src/module_graph/mod.rs:19-20`; test coverage at `:473`. |
| 14 | Symbol graph derivation runs after syntax adapters and module graph derivation, before metrics and rules. | VERIFIED | Local rule-host path runs Go/TS analysis, module graph, symbol graph, then metrics and rules in `crates/polint/src/runner/mod.rs:169-182`; parent CLI path also derives symbol graph in `crates/polint/src/cli/mod.rs:1013-1018`. |
| 15 | Provider diagnostics and capability support overrides merge into the support view passed to rules. | VERIFIED | `SymbolGraphDerivation::support_view` overlays provider rows in `crates/polint/src/symbol_graph/mod.rs:24-38`; runner passes merged support to `run_rules_with_capability_support` in `crates/polint/src/runner/mod.rs:175-188`. |
| 16 | Language extraction modules have typed contracts that preserve uncertainty as facts or capability rows. | VERIFIED | `LanguageSymbolOutput` carries diagnostics/support rows in `symbol_graph/mod.rs:41-45`; TS emits unresolved/ambiguous/setup/unsupported reference facts; Go emits setup-missing support rows and setup-missing reference rows in `go.rs:376-428`. |
| 17 | TS/JS files produce exact local symbols and definitions from Oxc semantic data. | VERIFIED | TS provider maps Oxc symbols/definitions to `ExactLocal` facts in `crates/polint/src/symbol_graph/ts.rs:188-257` and `:632-637`. |
| 18 | Resolved TS/JS references point to stable `SymbolId` targets when Oxc can bind them. | VERIFIED | Oxc resolved references are mapped to builder references with stable symbol IDs in `crates/polint/src/symbol_graph/ts.rs:260-300`; tests assert resolved `ExactLocal` references at `:1230-1235`. |
| 19 | Unresolved TS/JS root references remain visible as unresolved facts. | VERIFIED | Provider reads `root_unresolved_references_ids` and calls `add_unresolved_reference` in `crates/polint/src/symbol_graph/ts.rs:302-321`; external CLI test asserts unresolved `missingGlobal` evidence in `crates/polint/tests/cli.rs:4144-4154`. |
| 20 | Import alias references link through existing module graph facts with `ModuleLinked` precision when target exports can be matched. | VERIFIED | TS import alias linker reads `resolved_imports()` and `module_nodes()` and emits `ModuleLinked` references in `crates/polint/src/symbol_graph/ts.rs:344-400`; external CLI test asserts `ModuleLinked` import evidence in `crates/polint/tests/cli.rs:4134-4142`. |
| 21 | Go repositories with root `go.mod` and usable Go tooling produce typed symbols, definitions, and references from `go/packages` and `go/types`. | VERIFIED | Go provider requires root `go.mod`, invokes sidecar with fixed `go run` args, validates JSON, and converts typed rows in `crates/polint/src/symbol_graph/go.rs:132-167` and `:244-290`. |
| 22 | Go package-level symbols prefer objectpath-style stable identity where possible. | VERIFIED | Sidecar includes `objectpath` in package-level stable keys in `tools/polint-go-symbols/internal/symbols/emit.go:523-535` and calls `objectpath.For` at `:775`; Rust regression covers package objectpath ID stability in `go.rs:1062`. |
| 23 | Go local symbols include package/test variant, file path, lexical owner chain, name, and position in stable keys. | VERIFIED | Sidecar local-key tests assert these parts in `tools/polint-go-symbols/internal/symbols/emit_test.go:123-163`; emitter uses scopes and file/name/position key material in `emit.go:783-791`. |
| 24 | Missing Go setup produces deterministic setup-missing capability diagnostics and no fabricated facts. | VERIFIED | Missing `go.mod`, command failures, invalid JSON, and bad paths return setup-missing support in `crates/polint/src/symbol_graph/go.rs:132-164` and `:376-428`; CLI test proves the requesting rule is blocked in `crates/polint/tests/cli.rs:4053-4086`. |
| 25 | External repo-local rules can consume `Symbols<'_>` and `References<'_>` through `polint::sdk::prelude::*`. | VERIFIED | Temp-rule sources import only the public prelude and request typed views in `crates/polint/tests/cli.rs:260-312`, `:465-624`, and `:654-764`; internal import guard at `:403-425`. |
| 26 | TS/JS external tests prove local definitions/references, unresolved references, and module-linked import aliases. | VERIFIED | `external_rule_consumes_ts_symbols_and_references_through_public_sdk` asserts symbol/function/local/declaration-merge cases plus resolved, unresolved, and module-linked reference cases in `crates/polint/tests/cli.rs:4090-4155`. |
| 27 | Go external tests prove package function definitions/calls, method selector references, and setup-missing behavior. | VERIFIED | External Go test asserts function call, method call, field selector, and local variable reference evidence in `crates/polint/tests/cli.rs:4158-4213`; setup-missing test at `:4053-4086`. |
| 28 | Stable `SymbolId` values are unchanged across cache-backed repeated checks for unchanged source. | VERIFIED | CLI cache regression compares symbol/reference ID evidence across cached runs in `crates/polint/tests/cli.rs:4027-4049`; focused spot-check passed. |
| 29 | Public docs describe precision/status limits without claiming TS type-checker resolution, Go SSA, call graph, CFG, or dataflow. | VERIFIED | Docs define facts, IDs, statuses, language coverage, and explicit non-claims in `docs/facts/symbols-and-references.md:1-322`; generated skill guidance repeats limits in `crates/polint/src/cli/skill.rs:344-358`. |

**Score:** 29/29 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint/src/core/mod.rs` | Fact types, IDs, indexes, capabilities | VERIFIED | Structs/enums/storage/accessors present; `references()` enables `symbols` at `:1182-1185`. |
| `crates/polint/src/sdk/facts.rs` | `Symbols<'_>` and `References<'_>` typed views | VERIFIED | Borrowed query methods present at `:360-443`; `impl_fact_view!` mappings at `:725-726`. |
| `crates/polint/src/sdk/mod.rs` | Curated prelude exports | VERIFIED | Exports normalized fact types and views at `:27-52`. Formal checker missed exact text `Symbols, References` because export order is `References, ... Symbols`; intent is satisfied. |
| `crates/polint-macros/src/lib.rs` | Macro capability mapping | VERIFIED | `Symbols` and `References` map to capability names at `:323-324`; canonical path/lifetime validation preserved at `:302-316`. |
| `crates/polint/src/analysis_plan.rs` | Supported planner rows for symbols/references | VERIFIED | `support_for` marks `symbols` and `references` supported at `:588-596`; tests at `:1021-1069`. |
| `crates/polint/src/symbol_graph/stable_id.rs` | Stable semantic ID hashing | VERIFIED | Stable key encoders and ID helpers present; post-review fix removes transient `FileId` from span key at `:266-276`. |
| `crates/polint/src/symbol_graph/model.rs` | Deterministic builder/collision diagnostics | VERIFIED | `SymbolGraphBuilder` uses BTree staging, stable ID helpers, deterministic sort, and collision diagnostics. |
| `crates/polint/src/symbol_graph/query.rs` | Internal helper queries | VERIFIED | Query helpers wrap AnalysisDb indexes and match SDK view semantics. |
| `crates/polint/src/symbol_graph/mod.rs` | Derivation entrypoint/support overlay | VERIFIED | `derive_requested_symbols` runs TS/Go providers and stores facts at `:47-75`. |
| `crates/polint/src/lib.rs` | Internal symbol graph module registration | VERIFIED | `pub(crate) mod symbol_graph;` at `:33`; not public API. |
| `crates/polint/src/module_graph/mod.rs` | Module graph trigger for symbol context | VERIFIED | Trigger list includes `symbols` and `references` at `:19-20`. |
| `crates/polint/src/runner/mod.rs` | Local rule-host sequencing | VERIFIED | Symbol graph derivation runs before metrics/rules at `:175-182`. |
| `crates/polint/src/cli/mod.rs` | Parent/no-host sequencing | VERIFIED | Symbol graph derivation runs in parent path at `:1013-1016`. |
| `crates/polint/src/symbol_graph/ts.rs` | Oxc TS/JS extraction | VERIFIED | Uses `SemanticBuilder`, Oxc scoping references, unresolved roots, and module graph links. |
| `crates/polint/src/symbol_graph/go.rs` | Rust Go sidecar invocation/conversion | VERIFIED | Uses fixed `Command::new("go")` args, removes `GOFLAGS`, validates schema/path data, and converts sidecar rows. |
| `tools/polint-go-symbols/main.go` | Sidecar CLI entrypoint | VERIFIED | Parses `symbols` command and delegates to `symbols.Emit` at `:40-45`. Formal checker expected `packages.Load` in this file; actual loading is correctly inside `internal/symbols/emit.go`. |
| `tools/polint-go-symbols/internal/symbols/emit.go` | Go typed extraction | VERIFIED | Uses `packages.Load`, `TypesInfo`, selections, scopes, implicits, objectpath, and assignment classification. |
| `tools/polint-go-symbols/go.mod` | Go sidecar module | VERIFIED | Uses `golang.org/x/tools v0.45.0`. |
| `crates/polint/tests/cli.rs` | External consumer/cache/setup proof | VERIFIED | TS/JS, Go, macro determinism, cache stability, and setup-missing tests present. |
| `docs/facts/symbols-and-references.md` | Public fact contract docs | VERIFIED | Documents facts, IDs, query methods, statuses, precision, language limits, and non-claims. |
| `docs/facts/README.md` | Docs index link | VERIFIED | Links symbols/reference docs at `:15`. |
| `docs/facts/capability-plans.md` | Supported fact-view list | VERIFIED | Lists `Symbols<'_>` and `References<'_>` and notes references imply symbols. |
| `crates/polint/src/cli/skill.rs` | Generated skill guidance | VERIFIED | Symbol/reference guidance and precision/status caveats at `:344-358`. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| Macro parameters | `Capabilities` | Generated `Capabilities::symbols()` / `references()` calls | VERIFIED | Macro maps `Symbols`/`References` to method names; `Capabilities::references()` enables `symbols`. Literal checker missed the cross-file relationship due order/pattern strictness. |
| SDK views | `AnalysisDb` | Borrowed query helpers and DB lookup indexes | VERIFIED | `Symbols`/`References` call `symbol_graph::query`, which delegates to `AnalysisDb` indexes. |
| Prelude | SDK views | `pub use crate::sdk::facts::{..., References, ..., Symbols, ...}` | VERIFIED | Public prelude exports the views; literal checker false-negative due expected order. |
| Stable ID helpers | Core ID newtypes | Stable key hash returns `SymbolId`/`DefinitionId`/`ReferenceId` | VERIFIED | Helpers wrap public ID newtypes and are used by builder. |
| Symbol graph builder | Core facts | Builder emits `SymbolFact`, `DefinitionFact`, `ReferenceFact` | VERIFIED | Draft conversion methods construct normalized core facts. |
| Runner | Symbol derivation | `derive_requested_symbols` before rules | VERIFIED | Runner stores facts and passes merged support to rules. |
| TS provider | Module graph | `resolved_imports()` / `module_nodes()` for import alias linking | VERIFIED | TS import alias linker uses existing module graph facts and emits `ModuleLinked`. |
| Rust Go provider | Go sidecar | Fixed `go run` command for `tools/polint-go-symbols` | VERIFIED | Uses fixed sidecar directory, fixed arg list, `GOWORK`, and `env_remove("GOFLAGS")`; no shell execution. |
| Go sidecar | Rust provider | Versioned JSON schema | VERIFIED | Sidecar emits `polint-go-symbols-v1`; Rust validates schema before conversion. |
| External CLI tests | Public SDK | Temp repos import only `polint::sdk::prelude::*` | VERIFIED | Test helper rejects internal imports and manual capability declarations. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `#[polint::rule]` macro | `Capabilities` | Fact-view parameter types in rule signatures | Yes - `Symbols` and `References` map to real capability methods | FLOWING |
| `AnalysisPlan` | requested capabilities/support | `Rule::capabilities()` from macro-generated wrappers | Yes - `symbols`/`references` are supported rows | FLOWING |
| `runner::analyze_and_run` | symbol facts in `AnalysisDb` | Syntax adapters, module graph, then `symbol_graph::derive_requested_symbols` | Yes - derivation stores builder output before rules | FLOWING |
| `symbol_graph::ts` | TS/JS symbols/references | Oxc `SemanticBuilder`, scoping, root unresolved refs, module graph facts | Yes - facts from real parsed TS/JS source | FLOWING |
| `symbol_graph::go` | Go symbols/references | `go run` sidecar output validated against repo files/schema | Yes where `go.mod` setup exists; otherwise setup-missing support blocks rules | FLOWING |
| `tools/polint-go-symbols/internal/symbols/emit.go` | sidecar JSON symbols/references | `go/packages`, `go/types`, objectpath, scopes, selections | Yes - no static empty returns; tests reject raw source leakage | FLOWING |
| `sdk::facts::{Symbols, References}` | borrowed SDK facts | `AnalysisDb` indexes and query helpers | Yes - external rules read facts and emit asserted JSON diagnostics | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Go sidecar typed extraction tests run | `go test ./tools/polint-go-symbols/...` | Passed, internal symbols package cached OK | PASS |
| Planner promotes symbol capabilities | `cargo test -p polint --lib analysis_plan_supports_symbol_capabilities --locked` | Passed, 1 test | PASS |
| Stable IDs ignore transient `FileId` after review fix | `cargo test -p polint --lib stable_ids_do_not_include_transient_file_ids --locked` | Passed, 1 test | PASS |
| Unknown Go precision fails closed | `cargo test -p polint --lib unknown_go_reference_precision_is_unsupported --locked` | Passed, 1 test | PASS |
| Cache-backed symbol/reference IDs remain stable | `cargo test -p polint --test cli symbol_reference_cache_and_setup_keeps_stable_ids_across_cached_runs --locked` | Passed, 1 test, 22.31s | PASS |

Additional orchestrator-reported gates also passed after review fixes: `cargo fmt --all -- --check`, `go test ./tools/polint-go-symbols/...`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo test --workspace --all-features --locked`.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SYM-01 | 13-01, 13-03, 13-06 | Rule authors can read symbol, definition, and reference facts through typed SDK fact views. | SATISFIED | SDK views, prelude exports, macro mapping, external temp-repo tests through `polint::sdk::prelude::*`. |
| SYM-02 | 13-05, 13-06 | Go symbols and references are populated from typed package information where setup is available. | SATISFIED | Go sidecar uses `go/packages`/`go/types` and external Go rule test proves facts are consumable. |
| SYM-03 | 13-04, 13-06 | TS/JS symbols and references are populated from Oxc semantic facts where setup is available. | SATISFIED | TS provider uses `SemanticBuilder`; external TS rule test proves local, unresolved, and module-linked facts. |
| SYM-04 | 13-01, 13-02, 13-04, 13-05, 13-06 | Symbol/reference facts expose precision tiers and stable IDs suitable for diagnostics and cache restore. | SATISFIED | Stable key hashing, precision/status fields, collision diagnostics, review fix for transient `FileId`, and cache regression. |

No orphaned Phase 13 requirements were found in `.planning/REQUIREMENTS.md`; SYM-01 through SYM-04 are all declared in Phase 13 plan frontmatter and traced above.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/polint/tests/cli.rs` | multiple | `TODO` string literals | INFO | Test fixture data for existing string-literal policy behavior, not implementation stubs. |
| `crates/polint/src/symbol_graph/*` and sidecar files | n/a | Stub/hardcoded-empty scan | NONE | No blocking placeholder, `return []`, index-derived symbol ID, randomized hash, or unimplemented symbol/reference path found. |

### Human Verification Required

None. This phase has no visual, realtime, or external-service behavior that requires human-only verification.

### Gaps Summary

No goal-blocking gaps found.

Two formal `gsd-tools` literal pattern checks produced false negatives but were manually verified against intent:

- `crates/polint/src/sdk/mod.rs` exports `References` and `Symbols` in the same prelude export block, but not as the exact string `Symbols, References`.
- `tools/polint-go-symbols/main.go` is the sidecar CLI entrypoint and delegates to `symbols.Emit`; `packages.Load` correctly lives in `tools/polint-go-symbols/internal/symbols/emit.go`.

These are implementation organization differences, not unmet phase outcomes.

---

_Verified: 2026-05-13T08:24:38Z_
_Verifier: Claude (gsd-verifier)_
