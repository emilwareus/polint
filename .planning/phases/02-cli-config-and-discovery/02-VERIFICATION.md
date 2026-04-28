---
phase: 02-cli-config-and-discovery
verified: 2026-04-28T09:13:39Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
deferred:
  - truth: "Full TEST-02 coverage beyond the first CLI/config/discovery loop"
    addressed_in: "Phases 4, 5, 8, and 10"
    evidence: "REQUIREMENTS.md keeps TEST-02 in progress and later roadmap phases carry broader command, CI, fixture, and hardening coverage."
  - truth: "CLI-04, CLI-05, DIAG-03, cache persistence, snapshots, and property tests"
    addressed_in: "Phases 3, 7, 8, and 10"
    evidence: "REQUIREMENTS.md leaves those requirement IDs unchecked and the Phase 2 summaries explicitly avoid claiming them complete."
---

# Phase 2: CLI, Config, and Discovery Verification Report

**Phase Goal:** Make `polint init`, `new-rule`, and `check` work on discovered files.
**Verified:** 2026-04-28T09:13:39Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `polint init` creates `.polint.toml` and `.polint/rules` without overwriting an existing config. | VERIFIED | `crates/polint-cli/tests/cli.rs` contains `init_creates_config` and `init_does_not_overwrite_existing_config`; the focused CLI test suite passed. |
| 2 | `polint new-rule go <name>` and `polint new-rule ts <name>` create SDK-oriented repo-local Rust rule skeletons. | VERIFIED | Tests `new_rule_go_creates_sdk_oriented_skeleton` and `new_rule_ts_creates_sdk_oriented_skeleton` assert `Cargo.toml`, `src/lib.rs`, custom rule IDs, and expected SDK capability chains. |
| 3 | `polint check` exposes `--profile`, `--format`, `--no-cache`, and `--fail-on`. | VERIFIED | `crates/polint-cli/src/main.rs` has `CheckArgs` with `profile`, `format`, `no_cache`, and `fail_on`; `polint check --help` lists all four options. |
| 4 | JSON output remains parseable and does not mix missing-config guidance into stdout. | VERIFIED | `check_json_without_config_is_parseable` parses stdout as JSON and asserts the missing-config message is absent from JSON stdout. `/tmp/exlint-phase2-check.json` also parsed with `python3 -m json.tool`. |
| 5 | Missing config uses defaults and suggests `polint init` in human output. | VERIFIED | `check_human_without_config_suggests_init` covers the human suggestion; `load_config` returns `PolintConfig::default()` with `missing: true` when `.polint.toml` is absent. |
| 6 | `.polint.toml` supports include/exclude globs, profiles, rule paths, severity overrides, and language sections. | VERIFIED | `PolintConfig` contains workspace, rules, profiles, and languages sections; tests cover profiles, severity, `rules.paths`, include, and exclude behavior. |
| 7 | Discovery respects `.gitignore`, include globs, exclude globs, default excludes, and supported Go/TS/TSX/JS/JSX extensions. | VERIFIED | `discover_files` uses `WalkBuilder`, `.git_ignore(true)`, `.require_git(false)`, include/exclude `GlobSet`s, language detection, and deterministic sorting. CLI tests cover gitignore, include/exclude, default excludes, and supported extensions. |
| 8 | Explicit empty excludes do not exclude all files. | VERIFIED | `exclude_set` builds from the configured exclude list directly, while `include_set` owns the all-files fallback; `discovery_detects_all_supported_extensions` passes with `exclude = []`. |
| 9 | Full workspace verification passes after Phase 2 changes. | VERIFIED | `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` exited 0. Regression rerun of `cargo test --workspace` also passed. |
| 10 | GSD status closes only Phase 2 scope and keeps later requirements open. | VERIFIED | `REQUIREMENTS.md` marks CLI-01, CLI-02, CLI-03, CFG-01, CFG-02, FS-01, and DIAG-02 complete; TEST-02 is in progress; CLI-04, CLI-05, DIAG-03, FS-02, snapshots, property tests, and cache persistence remain open. |

**Score:** 10/10 truths verified

### Deferred Items

| # | Item | Addressed In | Evidence |
|---|------|--------------|----------|
| 1 | Full TEST-02 coverage beyond the first CLI/config/discovery loop | Phases 4, 5, 8, and 10 | `REQUIREMENTS.md` keeps TEST-02 in progress; later roadmap phases carry clean/failing Go and TS fixtures, command hardening, CI output, and final release coverage. |
| 2 | CLI-04, CLI-05, DIAG-03, cache persistence, snapshots, and property tests | Phases 3, 7, 8, and 10 | Requirement checkboxes and traceability rows remain pending for those IDs; Phase 2 summaries list them as intentionally not overclaimed. |

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/polint-cli/tests/cli.rs` | Focused Phase 2 CLI integration coverage | VERIFIED | Contains all named Phase 2 tests and passed under `cargo test -p polint-cli --test cli`. |
| `crates/polint-cli/src/main.rs` | CLI command surface for init, new-rule, and check | VERIFIED | Contains `Init`, `NewRule`, `Check`, and `CheckArgs`; CLI smoke checks passed. |
| `crates/polint-config/src/lib.rs` | Config model and defaults | VERIFIED | Contains load/default config, workspace include/exclude, profiles, rule paths/config, severities, and languages. |
| `crates/polint-fs/src/lib.rs` | File discovery behavior | VERIFIED | Uses `ignore` and `globset`, honors `.gitignore`, detects supported languages, and sorts by relative path. |
| `.planning/phases/02-cli-config-and-discovery/02-01-SUMMARY.md` | Source/test hardening summary | VERIFIED | Records focused tests, source fixes, and passing CLI integration suite. |
| `.planning/phases/02-cli-config-and-discovery/02-02-SUMMARY.md` | Closure/status summary | VERIFIED | Records full verification commands, result, completed requirements, and scoped TEST-02 status. |
| `.planning/phases/02-cli-config-and-discovery/02-REVIEW.md` | Phase code review report | VERIFIED | Status is `clean` with 0 findings across 3 reviewed files. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/polint-cli/src/main.rs` | `crates/polint-config/src/lib.rs` | `load_config`, profile rules, rule config | VERIFIED | `check` loads config, uses the selected profile, and passes rule config into built-in rules. |
| `crates/polint-cli/src/main.rs` | `crates/polint-fs/src/lib.rs` | `load_analysis_files` | VERIFIED | CLI analysis loads files discovered by the config-aware filesystem layer. |
| `crates/polint-cli/tests/cli.rs` | DIAG-02 JSON output | `serde_json` parsing | VERIFIED | JSON and SARIF smoke tests parse stdout as JSON values and assert stable fields. |
| `.planning/REQUIREMENTS.md` | Phase 2 status | Traceability rows | VERIFIED | Completed rows align with verified behavior and later-phase rows remain pending or in progress. |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Formatting is clean | `cargo fmt -- --check` | Exit 0 | PASS |
| Clippy is warning-clean | `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 | PASS |
| Workspace tests pass | `cargo test --workspace` | Exit 0 | PASS |
| Focused CLI integration tests pass | `cargo test -p polint-cli --test cli` | Exit 0; 14 tests passed | PASS |
| JSON output parses | `python3 -m json.tool /tmp/exlint-phase2-check.json` | Exit 0 | PASS |
| CLI help lists Phase 2 options | `rg -n -- '--profile|--format|--no-cache|--fail-on' /tmp/exlint-phase2-check-help.txt` | Found all four options | PASS |
| Schema drift gate | `gsd-tools verify schema-drift 02` | `drift_detected: false` | PASS |
| Code review gate | `.planning/phases/02-cli-config-and-discovery/02-REVIEW.md` | `status: clean` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CLI-01 | `02-01-PLAN.md` | `polint init` creates `.polint.toml` and `.polint/rules` | SATISFIED | CLI integration tests cover creation and non-overwrite behavior. |
| CLI-02 | `02-01-PLAN.md` | `polint new-rule <language> <rule-name>` scaffolds repo-local Rust rule | SATISFIED | Go and TS skeleton tests assert files, IDs, and SDK capability chains. |
| CLI-03 | `02-01-PLAN.md`, `02-02-PLAN.md` | `polint check` supports profile, format, no-cache, and fail threshold options | SATISFIED | `CheckArgs`, CLI help, integration tests, and smoke commands verify the surface. |
| CFG-01 | `02-01-PLAN.md` | Config supports include/exclude, profiles, rule paths, severity overrides, and language settings | SATISFIED | Config model and integration tests cover the required fields. |
| CFG-02 | `02-01-PLAN.md` | Missing config runs defaults and suggests init | SATISFIED | Missing-config JSON and human tests cover both paths. |
| FS-01 | `02-01-PLAN.md` | Discovery respects gitignore/include/exclude and detects Go, TS, TSX, JS, JSX | SATISFIED | `polint-fs` implementation and discovery tests verify the behavior. |
| DIAG-02 | `02-01-PLAN.md`, `02-02-PLAN.md` | Diagnostics render as JSON | SATISFIED | JSON output is parsed in tests and closure smoke checks. |
| TEST-02 | `02-01-PLAN.md`, `02-02-PLAN.md` | Integration tests cover the first Phase 2 loop, with broader command/CI coverage later | SATISFIED FOR PHASE 2 SCOPE | Focused integration suite passes; requirement remains in progress for later phases. |

### Anti-Patterns Found

None.

### Human Verification Required

None.

### Gaps Summary

No Phase 2 gaps found. The first usable CLI/config/discovery loop is verified on `main`: users can initialize config, scaffold rules, run checks over discovered files, receive parseable JSON output, and rely on missing-config defaults plus `.gitignore`/include/exclude filtering. Broader command, CI, cache, SARIF-hardening, snapshot, and property-test work remains explicitly scheduled in later phases.

---

_Verified: 2026-04-28T09:13:39Z_
_Verifier: Codex (inline GSD verifier)_
