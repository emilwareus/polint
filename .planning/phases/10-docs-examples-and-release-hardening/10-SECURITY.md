---
phase: 10
slug: docs-examples-and-release-hardening
status: verified
threats_open: 0
asvs_level: 1
created: 2026-05-01
verified: 2026-05-01T16:18:49Z
---

# Phase 10 — Security

Per-phase security contract: threat register, accepted risks, and audit trail.

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| README -> user expectations | Users rely on README claims for v1 capabilities and limits. | Commands, feature descriptions, non-goals, and future work. |
| examples -> user repositories | Users may copy example commands and configs into local repos. | `.polint.toml` snippets, command lines, and synthetic source examples. |
| examples -> CLI tests | Example source/config is executed through `polint check` in temp repos. | Local fixture files and diagnostic JSON. |
| verification commands -> release confidence | Final commands define release readiness evidence. | Test, fmt, clippy, and docs inventory outcomes. |

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-10-01-01 | Spoofing | README capability claims | mitigate | README states generated repo-local Rust rules are scaffolded but not dynamically compiled or loaded by `polint check` in v1. | closed |
| T-10-01-02 | Tampering | example command snippets | mitigate | README examples use explicit command paths and verified profile/format examples. | closed |
| T-10-01-03 | Repudiation | release readiness | mitigate | README and `10-04-SUMMARY.md` list exact verification commands and pass status. | closed |
| T-10-01-04 | Information Disclosure | docs/examples | mitigate | Examples are synthetic and contain no secrets, tokens, or customer data. | closed |
| T-10-02-01 | Spoofing | custom-rule examples | mitigate | Custom-rule READMEs state the dynamic-loading limitation. | closed |
| T-10-02-02 | Tampering | `.polint.toml` examples | mitigate | Runnable example configs are minimal and use exact built-in rule IDs. | closed |
| T-10-02-03 | Repudiation | heuristic examples | mitigate | Go branch-obligations README explicitly says the rule is heuristic. | closed |
| T-10-02-04 | Information Disclosure | examples | mitigate | Example source remains synthetic local code. | closed |
| T-10-03-01 | Spoofing | examples | mitigate | Configured examples are executed through CLI integration tests. | closed |
| T-10-03-02 | Tampering | mixed fixture proof | mitigate | Mixed fixture test asserts parsed JSON and DOT output containing `mixed/view.ts`. | closed |
| T-10-03-03 | Repudiation | test traceability | mitigate | `10-03-SUMMARY.md` records targeted CLI tests and property-test evidence. | closed |
| T-10-03-04 | Denial of Service | tests | mitigate | Smoke fixtures remain small temp-repo tests and avoid stress suites. | closed |
| T-10-04-01 | Spoofing | release readiness | mitigate | Final summary records pass status for docs inventory, targeted tests, fmt, clippy, and workspace tests. | closed |
| T-10-04-02 | Repudiation | final evidence | mitigate | `10-04-SUMMARY.md` and `10-VERIFICATION.md` record command-level evidence. | closed |
| T-10-04-03 | Tampering | future-work claims | mitigate | README and summary list future work instead of placeholder feature claims. | closed |
| T-10-04-04 | Denial of Service | verification | mitigate | Final matrix uses standard workspace commands, not benchmark or stress suites. | closed |

Status legend: `closed` threats have implemented mitigation evidence or an explicit accepted-risk entry.

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| None | - | No accepted security risks remain for Phase 10. | - | - |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-05-01 | 16 | 16 | 0 | Codex |

## Verification Evidence

```bash
cargo test -p polint-cli --test cli check_mixed_fixture_handles_go_and_ts_sources
cargo test -p polint-cli --test cli example_go_branch_obligations_reports_expected_diagnostic
cargo test -p polint-cli --test cli example_ts_design_tokens_reports_raw_colors
cargo fmt -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All listed commands passed during Phase 10 release verification.

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-05-01
