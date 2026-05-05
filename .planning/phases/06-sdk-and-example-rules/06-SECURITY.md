---
phase: 06-sdk-and-example-rules
phase_number: "06"
status: verified
asvs_level: 1
block_on: open high/critical threats
threats_total: 24
threats_closed: 24
threats_open: 0
accepted_risks: 6
unregistered_flags: 0
verified_at: 2026-05-01
auditor: Codex gsd-security-auditor
---

# Phase 06 Security Verification

## Status

Phase 06 threat mitigations are verified against the six registered `<threat_model>` blocks in the plan files.

| Metric | Count |
|--------|-------|
| Registered threats | 24 |
| Closed mitigations | 18 |
| Accepted risks documented | 6 |
| Transfers | 0 |
| Open threats | 0 |
| Unregistered summary flags | 0 |

ASVS Level 1 posture is satisfied for this phase. No high or critical registered threat is open.

## Threat Register

| Threat ID | Category | Component | Disposition | Status | Evidence |
|-----------|----------|-----------|-------------|--------|----------|
| T-06-01-01 | Tampering | `rule_template` in `crates/polint-cli/src/main.rs` | mitigate | CLOSED | `validate_rule_name` requires a single safe path component and exact sanitized name in `crates/polint-cli/src/main.rs:199`; existing rule paths are rejected before writes at `crates/polint-cli/src/main.rs:160`; directories are created with exclusive `create_dir` at `crates/polint-cli/src/main.rs:170`; generated source starts from `polint_sdk::prelude::*` at `crates/polint-cli/src/main.rs:245`; CLI tests reject unsafe names and existing rules at `crates/polint-cli/tests/cli.rs:462` and `crates/polint-cli/tests/cli.rs:501`. |
| T-06-01-02 | Information Disclosure | `RuleCtx::source_file` | accept | CLOSED | Accepted risk AR-06-01-02. `RuleCtx::source_file` returns local `AnalysisDb` source metadata only at `crates/polint-core/src/lib.rs:533`. |
| T-06-01-03 | Denial of Service | `RuleCtx` helper iteration | mitigate | CLOSED | Helpers return borrowed slices or iterators, including `files`, `packages`, `functions`, `imports`, `branches`, file-scoped iterators, related Go tests, literals, JSX attributes, and import edges at `crates/polint-core/src/lib.rs:508` through `crates/polint-core/src/lib.rs:705`. `cargo test -p polint-core --lib rule_ctx` passed. |
| T-06-01-04 | Elevation of Privilege | SDK prelude exports | accept | CLOSED | Accepted risk AR-06-01-04. `polint-sdk` only re-exports local rule, fact, diagnostic, and `Result` types at `crates/polint-sdk/src/lib.rs:9`; no dynamic rule host is added. |
| T-06-02-01 | Tampering | `RuleConfig.allow` | mitigate | CLOSED | `RuleConfig.allow` is serde-defaulted at `crates/polint-config/src/lib.rs:92`; `RuleOptions.allow` exists at `crates/polint-core/src/lib.rs:470`; `rule_options_from_config` copies `config.allow.clone()` exactly at `crates/polint-rules/src/lib.rs:597`. `cargo test -p polint-config --lib allow_list` and `cargo test -p polint-rules --lib` passed. |
| T-06-02-02 | Information Disclosure | literal diagnostic evidence | mitigate | CLOSED | Raw-color diagnostics report only `literal` and `source` evidence at `crates/polint-rules/src/lib.rs:497`; denied-literal diagnostics report only `literal`, `matched`, and `language` at `crates/polint-rules/src/lib.rs:416`; no surrounding source dump is emitted. |
| T-06-02-03 | Denial of Service | Go/TS literal extraction | mitigate | CLOSED | Go parsing borrows the shared source via `Arc<str>` and extracts string literals from parser nodes at `crates/polint-go/src/lib.rs:34` and `crates/polint-go/src/lib.rs:246`; TS parsing borrows `file.source.as_ref()` at `crates/polint-ts/src/lib.rs:48` and pushes literal facts through Oxc traversal at `crates/polint-ts/src/lib.rs:2562`. Go and TS targeted literal tests passed. |
| T-06-02-04 | Spoofing | regex literal syntax facts | accept | CLOSED | Accepted risk AR-06-02-04. Regex literals are syntax facts only: Oxc regex nodes are copied as raw source text at `crates/polint-ts/src/lib.rs:2442` and `crates/polint-ts/src/lib.rs:2576`; tests assert slash-delimited values at `crates/polint-ts/src/lib.rs:3424`. |
| T-06-03-01 | Tampering | glob matching in rule file filters | mitigate | CLOSED | File filter helpers centralize `files` and `allow_files` decisions at `crates/polint-rules/src/lib.rs:558`; glob matching uses `globset` at `crates/polint-rules/src/lib.rs:577`; rule tests cover file filters and allow files. `cargo test -p polint-rules --lib` passed. |
| T-06-03-02 | Information Disclosure | literal diagnostic messages | mitigate | CLOSED | Raw-color and denied-literal rules emit only matched literal values and stable evidence labels at `crates/polint-rules/src/lib.rs:497` and `crates/polint-rules/src/lib.rs:416`; snapshot JSON confirms evidence shape in `crates/polint-rules/tests/snapshots.rs:311`. |
| T-06-03-03 | Denial of Service | rule loops over facts | mitigate | CLOSED | Rules iterate borrowed fact views from `RuleCtx`, for example functions at `crates/polint-rules/src/lib.rs:45`, string literals at `crates/polint-rules/src/lib.rs:192`, and denied-literal facts at `crates/polint-rules/src/lib.rs:397`; raw-color overlap dedupe is bounded by finding spans at `crates/polint-rules/src/lib.rs:478`. |
| T-06-03-04 | Repudiation | deterministic diagnostics | mitigate | CLOSED | Stable built-in rule IDs are registered at `crates/polint-rules/src/lib.rs:6`; evidence labels and ranges are emitted in rule implementations; all eight Phase 6 IDs are asserted in snapshots at `crates/polint-rules/tests/snapshots.rs:19` and `crates/polint-rules/tests/snapshots.rs:470`. |
| T-06-04-01 | Spoofing | heuristic evidence matching | mitigate | CLOSED | Go heuristic rules use explicit heuristic wording and non-exact coverage language at `crates/polint-rules/src/lib.rs:240`, `crates/polint-rules/src/lib.rs:274`, `crates/polint-rules/src/lib.rs:315`, and `crates/polint-rules/src/lib.rs:367`; disclosure tests are present at `crates/polint-rules/src/lib.rs:1051`, `crates/polint-rules/src/lib.rs:1206`, and `crates/polint-rules/src/lib.rs:1342`. |
| T-06-04-02 | Information Disclosure | evidence terms | mitigate | CLOSED | Branch diagnostics include only `condition`, `edge`, and `branch_fingerprint` evidence at `crates/polint-rules/src/lib.rs:271`; assertion diagnostics include test name, assertion count, and extracted evidence terms at `crates/polint-rules/src/lib.rs:364`. |
| T-06-04-03 | Denial of Service | branch-to-test matching | mitigate | CLOSED | Related Go tests are bounded to same file or same-directory `_test.go` files at `crates/polint-core/src/lib.rs:587` through `crates/polint-core/src/lib.rs:634`; matching uses containment only, not regex or global fuzzy matching, at `crates/polint-rules/src/lib.rs:543` and `crates/polint-rules/src/lib.rs:551`. |
| T-06-04-04 | Repudiation | weighted score diagnostics | mitigate | CLOSED | Go test-suite score uses the declared formula at `crates/polint-rules/src/lib.rs:307`; score components are emitted as `score`, `subtests`, `table_rows`, and `assertions` evidence at `crates/polint-rules/src/lib.rs:319`. |
| T-06-05-01 | Tampering | `.polint.toml` test config | mitigate | CLOSED | CLI tests parse JSON output with `serde_json::from_str` at `crates/polint-cli/tests/cli.rs:14`; Phase 6 tests write exact TOML config strings and invoke `--profile phase6 --format json --fail-on none` at `crates/polint-cli/tests/cli.rs:117`, `crates/polint-cli/tests/cli.rs:167`, and `crates/polint-cli/tests/cli.rs:277`. |
| T-06-05-02 | Information Disclosure | JSON diagnostics | accept | CLOSED | Accepted risk AR-06-05-02. The diagnostics in these tests are produced from synthetic fixtures copied by test helpers, not external secrets. |
| T-06-05-03 | Denial of Service | CLI fixture breadth | mitigate | CLOSED | Phase 6 CLI coverage uses an exact eight-rule list at `crates/polint-cli/tests/cli.rs:45`; tests run small temp repositories and target profile `phase6` at `crates/polint-cli/tests/cli.rs:167`. `cargo test -p polint-cli --test cli phase6` passed. |
| T-06-05-04 | Repudiation | rule coverage claims | mitigate | CLOSED | `check_phase6_runs_all_requested_example_rules` asserts every exact `examples/...` ID from `PHASE6_RULE_IDS` in parsed JSON at `crates/polint-cli/tests/cli.rs:184`; all-rule snapshot coverage asserts all IDs at `crates/polint-rules/tests/snapshots.rs:470`. |
| T-06-06-01 | Tampering | inline snapshots | mitigate | CLOSED | Snapshot tests build deterministic synthetic files and facts at `crates/polint-rules/tests/snapshots.rs:46`; rule selection goes through `built_in_rules()` at `crates/polint-rules/tests/snapshots.rs:194`; missing selected rules fail the test at `crates/polint-rules/tests/snapshots.rs:200`. |
| T-06-06-02 | Repudiation | rule coverage evidence | mitigate | CLOSED | JSON snapshots parse renderer output before assertion at `crates/polint-rules/tests/snapshots.rs:229`; the all-rule-ID JSON snapshot selects one diagnostic per rule and asserts all eight rule IDs at `crates/polint-rules/tests/snapshots.rs:237` and `crates/polint-rules/tests/snapshots.rs:470`. |
| T-06-06-03 | Denial of Service | workspace tests | accept | CLOSED | Accepted risk AR-06-06-03. Local cargo verification is required by project policy and involves no network or external service. Phase summary records workspace fmt, clippy, and tests passing in `.planning/phases/06-sdk-and-example-rules/06-06-SUMMARY.md:130`. |
| T-06-06-04 | Information Disclosure | snapshot contents | accept | CLOSED | Accepted risk AR-06-06-04. Snapshot fixtures are synthetic paths and literals constructed in `crates/polint-rules/tests/snapshots.rs:46`. |

## Accepted Risks Log

| Risk ID | Threat ID | Category | Rationale | Status |
|---------|-----------|----------|-----------|--------|
| AR-06-01-02 | T-06-01-02 | Information Disclosure | `RuleCtx::source_file` exposes only local analysis data already available to repo-local rules. This phase adds no network access, credential lookup, or secret store integration. | accepted |
| AR-06-01-04 | T-06-01-04 | Elevation of Privilege | The SDK prelude re-exports existing local Rust types only. It does not add dynamic rule loading or a new execution boundary. | accepted |
| AR-06-02-04 | T-06-02-04 | Spoofing | Regex literals are recorded as syntax-level source text for later rule matching. No semantic regex evaluation or trust decision is introduced. | accepted |
| AR-06-05-02 | T-06-05-02 | Information Disclosure | Phase 6 JSON diagnostics in integration tests use synthetic fixtures committed under `tests/fixtures`; no secrets or external repositories are read. | accepted |
| AR-06-06-03 | T-06-06-03 | Denial of Service | Workspace cargo verification is local developer verification required by the project workflow. It does not call external services. | accepted |
| AR-06-06-04 | T-06-06-04 | Information Disclosure | Inline snapshots contain synthetic paths, rule IDs, and synthetic literals only. | accepted |

## Unregistered Flags

None. `rg -n "^## Threat Flags|Threat Flags" .planning/phases/06-sdk-and-example-rules/06-*-SUMMARY.md` returned no `## Threat Flags` sections.

## Audit Trail

| Time | Action | Result |
|------|--------|--------|
| 2026-05-01 | Loaded all six PLAN files and extracted every `<threat_model>` block. | 24 registered threats found. |
| 2026-05-01 | Loaded all requested SUMMARY, REVIEW, REVIEW-FIX, VERIFICATION, and implementation/test files. | REVIEW status was clean; REVIEW-FIX documented the `new-rule` overwrite fix as all fixed. |
| 2026-05-01 | Checked summary files for `## Threat Flags`. | No summary threat flags found; no unregistered flags logged. |
| 2026-05-01 | Grepped mitigation patterns in cited files for rule-template safety, path traversal/overwrite prevention, borrowed helper iteration, literal allow copying, limited diagnostic evidence, heuristic wording, bounded branch matching, parsed JSON config tests, and deterministic snapshots. | All registered mitigation patterns found. |
| 2026-05-01 | Ran `cargo test -p polint-cli new_rule`. | Passed: 6 new-rule tests, including unsafe-name and existing-rule overwrite regression tests. |
| 2026-05-01 | Ran `cargo test -p polint-core --lib rule_ctx`. | Passed: 4 RuleCtx helper tests. |
| 2026-05-01 | Ran `cargo test -p polint-config --lib allow_list`. | Passed: 2 allow-list config tests. |
| 2026-05-01 | Ran `cargo test -p polint-go --lib string_literal`. | Passed: 2 Go literal extraction tests. |
| 2026-05-01 | Ran `cargo test -p polint-ts --lib regex_literal`. | Passed: 2 TS regex literal tests. |
| 2026-05-01 | Ran `cargo test -p polint-rules --lib`. | Passed: 26 rule behavior tests. |
| 2026-05-01 | Ran `cargo test -p polint-rules --test snapshots`. | Passed: 4 snapshot tests. |
| 2026-05-01 | Ran `cargo test -p polint-cli --test cli phase6`. | Passed: 3 Phase 6 CLI JSON tests. |

## Security Status

All registered Phase 06 threats are closed by implemented mitigations or documented accepted risks. `threats_open: 0`.
