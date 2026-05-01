---
phase: 08-ci-output-and-graph-commands
verified: "2026-05-01T11:46:00.000Z"
status: passed
score: 5/5 success criteria verified
---

# Phase 08: ci-output-and-graph-commands — Verification

## Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Command surface is implemented and covered for `explain`, `test-rules`, `profile-rules`, `graph imports`, and `graph function`. | verified | `cargo test -p polint-cli --test cli explain`, `test_rules`, `fail_on`, `graph`, and existing `profile_rules_*` tests passed. |
| 2 | SARIF-like output includes rule IDs, locations, messages, severities, and fingerprints. | verified | `render_sarif_snapshot_includes_ci_fields` and `sarif_output_includes_ci_fields_and_honors_fail_threshold` passed. |
| 3 | `--fail-on warn|error|none` produces stable exit codes, while fatal config errors return code 2. | verified | `check_fail_on_warn_error_none_exit_codes_are_stable` and `fatal_config_parse_error_exits_two` passed. |
| 4 | DOT graph export works for import graphs and available function graphs. | verified | `cargo test -p polint-graph --lib` and `cargo test -p polint-cli --test cli graph` passed. |
| 5 | Integration and snapshot tests cover CI output and exit-code behavior. | verified | `cargo test -p polint-rules --test snapshots`, `cargo test -p polint-diagnostics --lib sarif`, and `cargo test --workspace` passed. |

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `.planning/phases/08-ci-output-and-graph-commands/08-01-SUMMARY.md` | CLI contract completion summary | present | Records `test-rules` machine-output fix and exit-code coverage. |
| `.planning/phases/08-ci-output-and-graph-commands/08-02-SUMMARY.md` | SARIF-like output completion summary | present | Records renderer and CLI SARIF-like field coverage. |
| `.planning/phases/08-ci-output-and-graph-commands/08-03-SUMMARY.md` | DOT graph command completion summary | present | Records graph helper and CLI DOT coverage. |
| `.planning/phases/08-ci-output-and-graph-commands/08-04-SUMMARY.md` | verification summary | present | Records targeted and full workspace verification commands. |

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `polint test-rules --format json` | CI machine stdout | `Command::TestRules` delegates without human prelude unless format is human | verified | CLI JSON parse test passed. |
| `polint check --format sarif` | SARIF-like CI output | `render_sarif` typed serialization | verified | Field assertions and snapshot passed in single-crate and workspace test runs. |
| `polint graph imports/function` | DOT graph output | `ImportGraph::from_db` and `FunctionGraph::from_db` | verified | Unit and CLI graph tests passed with repeated byte-identical output. |
| fail thresholds | process status | `exit_code_for` and top-level fatal error path | verified | Integration tests cover warn/error/none and fatal parse error code 2. |

## Requirements Coverage

| Requirement | Status | Blocking Issue |
|-------------|--------|----------------|
| CLI-04 | verified | none |
| CLI-05 | verified | none |
| DIAG-03 | verified | none |
| TEST-02 | verified | none |
| TEST-03 | verified | none |

## Result

Phase 08 verification passed. The phase delivers the requested CLI command surface coverage, SARIF-like CI output, fail-threshold behavior, deterministic DOT graph command proof, and full workspace verification. Full SARIF certification, dynamic rule loading, alternate graph formats, and semantic graph resolution remain out of scope.
