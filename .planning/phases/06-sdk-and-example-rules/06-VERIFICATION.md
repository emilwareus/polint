---
phase: 06-sdk-and-example-rules
verified: 2026-05-01T06:48:59Z
status: passed
score: 25/25 must-haves verified
overrides_applied: 0
---

# Phase 6: SDK and Example Rules Verification Report

**Phase Goal:** Make rule authoring pleasant and prove the SDK by implementing the requested example rules.
**Verified:** 2026-05-01T06:48:59Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `polint-sdk` exposes a documented `Rule` trait, metadata, capabilities, prelude, and `RuleCtx` helpers. | VERIFIED | Rustdoc exists on `Rule`, `RuleMeta`, `Capabilities`, `RuleOptions`, and `RuleCtx`; SDK prelude re-exports the authoring surface in `crates/polint-sdk/src/lib.rs:1-20`. |
| 2 | Built-in example rules use the SDK rather than private core shortcuts. | VERIFIED | `crates/polint-rules/src/lib.rs:3` imports `polint_sdk::prelude::*`; production rule implementations use `RuleCtx` helpers and no `ctx.db()` calls. |
| 3 | All requested example rules emit deterministic, useful diagnostics against fixtures. | VERIFIED | `built_in_rules()` registers all 8 rule IDs; CLI Phase 6 JSON tests assert all IDs and evidence in `crates/polint-cli/tests/cli.rs:117-402`. |
| 4 | Heuristic rules use honest wording and include evidence/help. | VERIFIED | Go branch, suite-size, and assertion rules include `heuristic` help/messages and evidence labels in `crates/polint-rules/src/lib.rs:236-375`. |
| 5 | Snapshot tests cover representative diagnostics for each rule family. | VERIFIED | `crates/polint-rules/tests/snapshots.rs` covers human snapshots for complexity/import-boundary/Go heuristics and JSON snapshots/all-rule IDs. |
| 6 | Rule authors can start from `use polint_sdk::prelude::*;` without importing `polint-core` for normal rule authoring. | VERIFIED | SDK smoke test uses only `crate::prelude::*` and passes; generated templates start with `use polint_sdk::prelude::*;`. |
| 7 | Rule authors can query files, packages, functions, imports, import edges, branch obligations, Go tests, TS components/classes, literals, JSX attributes, and source files through `RuleCtx`. | VERIFIED | `RuleCtx` helpers exist in `crates/polint-core/src/lib.rs:508-705`; `cargo test -p polint-core --lib rule_ctx` passed. |
| 8 | Rule authors can query Go tests related to a production file through companion `_test.go` matching. | VERIFIED | `go_tests_for_related_file` implements same-directory `_test.go` matching at `crates/polint-core/src/lib.rs:587-634`; companion test passed. |
| 9 | `polint new-rule` scaffolds SDK-oriented rule code with helpers and capability declarations. | VERIFIED | Template emits language-specific capabilities and helper examples in `crates/polint-cli/src/main.rs:215-268`; scaffold tests assert exact generated strings. |
| 10 | Literal-based rules support exact literal allow-list values while preserving file allow-list behavior. | VERIFIED | `RuleConfig.allow` and `RuleOptions.allow` flow through `rule_options_from_config`; raw-color and denied-literal tests cover allow and `allow_files`. |
| 11 | Go string literals are available as `StringLiteralFact` values for SDK rules. | VERIFIED | Go parser pushes string facts and excludes import paths in `crates/polint-go/src/lib.rs:246-262`; targeted tests passed. |
| 12 | TS/JS regex literals are available as syntax-level literal facts. | VERIFIED | Oxc regex literal handling preserves raw slash-delimited text in `crates/polint-ts/src/lib.rs:2436-2454` and `2562-2586`; targeted tests passed. |
| 13 | Go and TS complexity rules use SDK-facing APIs and configurable `max` thresholds. | VERIFIED | Complexity rules read `ctx.options().max`, use `ctx.functions()`, and emit evidence/help in `crates/polint-rules/src/lib.rs:28-122`. |
| 14 | Go import-boundary rule uses configured `forbidden_imports` and deterministic evidence. | VERIFIED | Rule checks `ctx.options().forbidden_imports`, file scope, and emits `import` evidence in `crates/polint-rules/src/lib.rs:124-173`. |
| 15 | TS raw-color rule detects syntax-level raw colors with file and literal allow-list support. | VERIFIED | Rule scans string literals and JSX attributes, checks `allow`/`allow_files`, dedupes overlaps, and states syntax-level behavior in `crates/polint-rules/src/lib.rs:175-234` and `462-521`. |
| 16 | Config-query denied-literal rule denies configured Go/TS string and TS regex literal facts. | VERIFIED | Rule reads `ctx.options().deny`, respects file/literal allow lists, and emits `literal`, `matched`, and `language` evidence in `crates/polint-rules/src/lib.rs:378-435`. |
| 17 | Go branch-obligations rule reports missing nearby test evidence with honest heuristic wording. | VERIFIED | Rule uses `ctx.branches()` and `go_tests_for_related_file`; diagnostics include condition/edge/fingerprint evidence and heuristic help. |
| 18 | Go test-suite-size rule computes weighted maintainability score and reports configurable excess scores. | VERIFIED | Score formula `1 + subtests*4 + table_rows*2 + assertions`, default max 24, evidence labels, and tests are present in `crates/polint-rules/src/lib.rs:285-332`. |
| 19 | Go assertion-after-action rule warns when test evidence has no recognizable assertion or error check. | VERIFIED | Rule reports `assertion_count == 0` with test/assertions/evidence_terms labels and heuristic help in `crates/polint-rules/src/lib.rs:334-375`. |
| 20 | CLI integration tests prove every requested `examples/...` rule ID can run from a configured profile. | VERIFIED | `PHASE6_RULE_IDS` lists all 8 IDs and `check_phase6_runs_all_requested_example_rules` asserts all appear in parsed JSON. |
| 21 | Clean fixtures suppress example-rule diagnostics with reasonable thresholds and allow-lists. | VERIFIED | `check_phase6_clean_fixtures_do_not_emit_example_rule_diagnostics` asserts no `examples/*` diagnostics for clean fixtures. |
| 22 | Failing fixtures produce parsed JSON diagnostics with expected rule IDs, evidence, help text, and configured severity behavior. | VERIFIED | `check_phase6_rule_options_configure_thresholds_allow_lists_and_denied_literals` asserts severity override, import/literal/matched/score/branch evidence, and allow suppression. |
| 23 | Human snapshots cover complexity/import-boundary and Go heuristic rule families. | VERIFIED | Snapshot tests `snapshot_complexity_and_import_boundary_human` and `snapshot_go_heuristics_human` passed. |
| 24 | JSON snapshots cover raw-color and denied-literal rule families. | VERIFIED | `snapshot_raw_color_and_denied_literals_json` parses JSON renderer output and snapshots raw-color/denied-literal diagnostics. |
| 25 | Full workspace formatting, clippy, and tests pass after Phase 6. | VERIFIED | Verifier ran `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`; all passed. |

**Score:** 25/25 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint-core/src/lib.rs` | Documented rule contract, `RuleOptions.allow`, `RuleCtx` helpers | VERIFIED | Exists, substantive, wired through SDK prelude and rule execution; GSD artifact check passed. |
| `crates/polint-sdk/src/lib.rs` | Public SDK prelude and compile smoke coverage | VERIFIED | Re-exports core fact/rule/diagnostic types; SDK prelude test passed. |
| `crates/polint-cli/src/main.rs` | SDK-oriented new-rule template and check wiring | VERIFIED | Template uses SDK prelude and helpers; `check` loads config, analysis facts, built-in rules, and `run_rules`. |
| `crates/polint-cli/tests/cli.rs` | Phase 6 CLI integration tests and new-rule scaffold assertions | VERIFIED | Phase 6 tests pass and assert parsed JSON rather than substring-only output. |
| `crates/polint-config/src/lib.rs` | `allow = [...]` TOML config field | VERIFIED | `RuleConfig.allow` has serde default and parsing tests. |
| `crates/polint-go/src/lib.rs` | Go string literal extraction | VERIFIED | Parser-backed string facts pushed to `AnalysisDb`; import string exclusion tested. |
| `crates/polint-ts/src/lib.rs` | TS/JS regex literal extraction | VERIFIED | Regex literals become syntax-level `StringLiteralFact` values with raw text/flags. |
| `crates/polint-rules/src/lib.rs` | SDK-facing implementations for all 8 example rules | VERIFIED | Uses SDK prelude, registers all rules, has unit tests for config/evidence/heuristic behavior. |
| `tests/fixtures/go/failing/payment_test.go` | Go failing fixture for suite-size and assertion heuristics | VERIFIED | Contains oversized suite and no-assertion test; consumed by CLI integration tests. |
| `tests/fixtures/ts/failing/component.tsx` | TS failing fixture with raw color, denied string, regex literal syntax | VERIFIED | Fixture contains `#ff00aa`, `legacy-testid`, and `/legacy-testid/`; consumed by CLI tests. |
| `crates/polint-rules/Cargo.toml` | Snapshot test dev-dependencies | VERIFIED | Adds existing workspace `insta` and `serde_json` dev-dependencies. |
| `crates/polint-rules/tests/snapshots.rs` | Human and JSON rule-family snapshots | VERIFIED | Executes selected rules through `built_in_rules()` and renders human/JSON diagnostics. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/polint-sdk/src/lib.rs` | `crates/polint-core/src/lib.rs` | Prelude re-exports | WIRED | GSD key-link check passed; prelude re-exports `Rule`, `RuleCtx`, facts, diagnostics. |
| `crates/polint-cli/src/main.rs` | `crates/polint-sdk/src/lib.rs` | Generated `use polint_sdk::prelude::*;` template | WIRED | GSD key-link check passed; scaffold tests assert no generated `polint_core::` import. |
| `crates/polint-config/src/lib.rs` | `crates/polint-core/src/lib.rs` | `RuleConfig.allow` mapped to `RuleOptions.allow` through `rule_options_from_config` | WIRED | Manual check: `RuleConfig.allow` in config, `RuleOptions.allow` in core, and mapping at `crates/polint-rules/src/lib.rs:589-601`. GSD pattern check missed due exact pattern text. |
| `crates/polint-go/src/lib.rs` | `crates/polint-core/src/lib.rs` | `AnalysisDb::push_string_literal` | WIRED | GSD key-link check passed; Go string literal tests passed. |
| `crates/polint-ts/src/lib.rs` | `crates/polint-core/src/lib.rs` | Regex literal `StringLiteralFact` | WIRED | GSD key-link check passed; TS regex literal tests passed. |
| `crates/polint-rules/src/lib.rs` | `crates/polint-sdk/src/lib.rs` | `use polint_sdk::prelude::*;` | WIRED | GSD key-link check passed; production rule module uses SDK prelude. |
| `crates/polint-rules/src/lib.rs` | `RuleOptions` | `ctx.options()` fields | WIRED | Manual check: rules consume `max`, `files`, `allow`, `allow_files`, `deny`, and `forbidden_imports`; GSD pattern check missed escaped regex. |
| `crates/polint-rules/src/lib.rs` | `RuleCtx::branches` and `RuleCtx::go_tests_for_related_file` | Go branch evidence lookup | WIRED | GSD key-link check passed; branch-obligation companion evidence test passed. |
| `crates/polint-rules/src/lib.rs` | `TestFact` assertion/subtest/table fields | Weighted and assertion heuristics | WIRED | GSD key-link check passed; suite-size/assertion tests passed. |
| `crates/polint-cli/tests/cli.rs` | `polint check --profile phase6 --format json` | `assert_cmd` integration tests | WIRED | GSD key-link check passed; Phase 6 CLI tests passed. |
| Fixtures | Rules | Profile includes all eight examples | WIRED | Manual check: tests copy Go/TS fixtures and use exact `PHASE6_RULE_IDS`; GSD check could not resolve synthetic `fixtures` source. |
| `crates/polint-rules/tests/snapshots.rs` | `polint_rules::built_in_rules` | Public registration path | WIRED | GSD key-link check passed. |
| `crates/polint-rules/tests/snapshots.rs` | `polint_diagnostics::render` | Human and JSON renderer snapshots | WIRED | GSD key-link check passed; JSON snapshots parse renderer output before snapshotting. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `polint-sdk` prelude | Rule/fact/diagnostic authoring types | `pub use polint_core::{...}` and `pub use polint_diagnostics::{...}` | Yes - compile smoke implements and runs a rule using only prelude imports | FLOWING |
| `RuleCtx` helpers | Files, functions, imports, packages, branches, tests, literals, JSX attributes | Borrowed `AnalysisDb` slices/iterators | Yes - core tests construct `AnalysisDb` facts and assert exact helper outputs/order | FLOWING |
| Literal config | `RuleOptions.allow` | TOML `RuleConfig.allow` -> `rule_options_from_config` -> `ctx.options().allow` | Yes - config/rules tests and raw-color/denied-literal allow tests pass | FLOWING |
| Go literal facts | `StringLiteralFact` | tree-sitter Go traversal -> `AnalysisDb::push_string_literal` -> `ctx.string_literals()` | Yes - Go tests assert exact literal values and no import path duplication | FLOWING |
| TS regex literal facts | `StringLiteralFact` | Oxc regex AST nodes -> raw regex text -> `ctx.string_literals()` | Yes - TS tests assert `/legacy-testid/` and `/^unsafe-/i` values | FLOWING |
| Built-in rules | Diagnostics | `built_in_rules()` -> `run_rules()` -> `Diagnostic` with evidence/help | Yes - 26 rule unit tests and snapshots pass | FLOWING |
| CLI Phase 6 tests | JSON diagnostics | temp repo fixtures/config -> `polint check --profile phase6 --format json` | Yes - CLI tests assert all rule IDs, severity/evidence, clean suppression | FLOWING |
| Snapshot tests | Rendered diagnostics | synthetic `AnalysisDb` -> `built_in_rules()` -> `render(Human/Json)` | Yes - 4 snapshots passed and JSON is parsed before snapshotting | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| SDK prelude is sufficient for rule authoring | `cargo test -p polint-sdk --lib sdk_prelude_exports_rule_authoring_surface` | 1 passed | PASS |
| RuleCtx helper surface works | `cargo test -p polint-core --lib rule_ctx` | 4 passed | PASS |
| Go string literal facts flow | `cargo test -p polint-go --lib string_literal` | 2 passed | PASS |
| TS regex literal facts flow | `cargo test -p polint-ts --lib regex_literal` | 2 passed | PASS |
| Built-in example rule unit behavior | `cargo test -p polint-rules --lib` | 26 passed | PASS |
| Representative rule snapshots | `cargo test -p polint-rules --test snapshots` | 4 passed | PASS |
| CLI Phase 6 profile behavior | `cargo test -p polint-cli --test cli phase6` | 3 passed | PASS |
| Workspace formatting | `cargo fmt -- --check` | exit 0 | PASS |
| Workspace clippy | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | PASS |
| Workspace tests | `cargo test --workspace` | all workspace tests and doctests passed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SDK-01 | 06-01 | `polint-sdk` exposes documented `Rule`, `RuleMeta`, `Capabilities`, `RuleCtx`, and prelude. | SATISFIED | SDK prelude docs/re-exports and core rustdoc verified; SDK smoke test passed. |
| SDK-02 | 06-01, 06-02 | `RuleCtx` exposes high-level queries for fact families and diagnostics. | SATISFIED | Helpers for files/functions/imports/graphs/branches/tests/TS/literals/JSX/source files verified; core tests passed. |
| RULE-01 | 06-03, 06-05, 06-06 | `examples/go-cyclomatic-complexity` works and is configurable. | SATISFIED | Rule uses `max`; unit, CLI, and snapshot coverage passed. |
| RULE-02 | 06-03, 06-05, 06-06 | `examples/ts-cyclomatic-complexity` works and is configurable. | SATISFIED | Rule uses `max`; unit, CLI, and snapshot coverage passed. |
| RULE-03 | 06-03, 06-05, 06-06 | `examples/go-import-boundaries` works and is configurable. | SATISFIED | Rule uses `forbidden_imports`, file filters, evidence; unit/CLI/snapshot coverage passed. |
| RULE-04 | 06-02, 06-03, 06-05, 06-06 | `examples/ts-no-raw-colors` detects raw color literals with allow-list support. | SATISFIED | Raw-color rule checks string/JSX facts, `allow`, `allow_files`, dedupe; unit/CLI/JSON snapshot coverage passed. |
| RULE-05 | 06-04, 06-05, 06-06 | `examples/go-branch-obligations` reports missing nearby test evidence using honest heuristic wording. | SATISFIED | Uses `branches` and related Go test helper; diagnostic contains heuristic help and condition/edge/fingerprint evidence. |
| RULE-06 | 06-04, 06-05, 06-06 | `examples/go-test-suite-size` computes weighted maintainability score. | SATISFIED | Formula and configurable max verified by unit tests, CLI evidence, and snapshots. |
| RULE-07 | 06-04, 06-05, 06-06 | `examples/go-assertion-after-action` warns when tests appear to lack assertions. | SATISFIED | `assertion_count == 0` behavior, heuristic help, and evidence labels verified. |
| RULE-08 | 06-02, 06-03, 06-05, 06-06 | `examples/config-query-no-literal` denies configured string/regex literals across supported languages where possible. | SATISFIED | Go strings and TS regex facts flow into rule; `deny`/`allow` behavior verified. |
| TEST-01 | 06-01 through 06-06 | Unit tests cover rule logic, config, generation, parser facts, and supporting behavior. | SATISFIED | Workspace tests passed; relevant package tests cover helper, parser fact, config, and rule behavior. |
| TEST-03 | 06-06 | Snapshot tests cover diagnostics. | SATISFIED FOR PHASE 6 | Human and JSON rule-family snapshots passed. Broader SARIF/CI hardening remains Phase 8 by roadmap scope. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| Multiple test/fixture files | Various | Empty arrays, Go table-test literals, wildcard match arms | Info | Intentional test data/control flow only; no placeholder runtime implementation or hollow user-visible output found. |

### Human Verification Required

None.

### Gaps Summary

No blocking gaps found. Phase 6 achieves the SDK/example-rule goal without taking ownership of later-phase SARIF/CI hardening, cache/performance, graph commands, dynamic rule loading, or final documentation.

---

_Verified: 2026-05-01T06:48:59Z_
_Verifier: Claude (gsd-verifier)_
