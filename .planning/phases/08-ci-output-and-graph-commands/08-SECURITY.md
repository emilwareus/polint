---
phase: 08
slug: ci-output-and-graph-commands
status: verified
threats_open: 0
asvs_level: 1
created: 2026-05-01
verified: 2026-05-01T11:49:00Z
---

# Phase 08 — Security

Per-phase security contract: threat register, accepted risks, and audit trail.

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| CLI -> stdout | `check`, `test-rules`, `explain`, `profile-rules`, and graph commands emit user-visible output. | Diagnostics, rule metadata, timing rows, and DOT graph labels. |
| diagnostics -> SARIF-like JSON | Diagnostics are serialized for CI output. | Rule IDs, severity level, message, fingerprint, file URI, and range metadata. |
| analysis DB -> DOT graph | Graph commands render import/function facts to DOT. | Local file paths, import specifiers, function names, and call labels. |
| command result -> process status | CLI converts diagnostics or fatal errors into exit codes. | `0`, `1`, or `2` process status. |

## Threat Register

| Threat ID | Category | Component | Disposition | Mitigation | Status |
|-----------|----------|-----------|-------------|------------|--------|
| T-08-01-01 | Tampering | machine-readable stdout | mitigate | `test-rules --format json` parse test passed and human prelude is gated to human format. | closed |
| T-08-01-02 | Spoofing | CI status | mitigate | Integration tests cover warn/error/none fail thresholds and fatal config error code 2. | closed |
| T-08-01-03 | Repudiation | explain output | mitigate | Known and unknown rule explain tests passed. | closed |
| T-08-01-04 | Information Disclosure | explain output | accept | Built-in rule IDs, severities, and descriptions are public local CLI metadata. | closed |
| T-08-02-01 | Tampering | SARIF-like fields | mitigate | Renderer snapshot and JSON pointer assertions cover required CI fields. | closed |
| T-08-02-02 | Spoofing | SARIF-like severity | mitigate | Renderer and CLI tests assert error/warning level mapping. | closed |
| T-08-02-03 | Repudiation | SARIF-like fingerprints | mitigate | Tests assert `fingerprints.polint` is present and nonempty. | closed |
| T-08-02-04 | Information Disclosure | SARIF-like payload | mitigate | Output includes diagnostic metadata only; no source text is added. | closed |
| T-08-03-01 | Repudiation | graph output | mitigate | CLI graph tests compare repeated DOT stdout exactly. | closed |
| T-08-03-02 | Tampering | graph labels/order | mitigate | Graph unit and CLI tests assert expected labels and deterministic output. | closed |
| T-08-03-03 | Denial of Service | missing function graph | mitigate | Missing function names produce valid empty DOT and exit 0. | closed |
| T-08-03-04 | Information Disclosure | graph output | accept | DOT output contains local file/module/function metadata already analyzed by the local CLI. | closed |
| T-08-04-01 | Repudiation | phase evidence | mitigate | `08-04-SUMMARY.md` records targeted and full verification commands. | closed |
| T-08-04-02 | Spoofing | CI pass/fail | mitigate | Targeted exit-code tests and full workspace tests passed. | closed |
| T-08-04-03 | Tampering | snapshots | mitigate | Diagnostic snapshots passed under single-crate and workspace builds after typed serialization hardening. | closed |
| T-08-04-04 | Information Disclosure | summary claims | mitigate | Summaries explicitly use "SARIF-like" and record out-of-scope items. | closed |

Status legend: `closed` threats have either implemented mitigation evidence or an explicit accepted-risk entry.

## Accepted Risks Log

| Risk ID | Threat Ref | Rationale | Accepted By | Date |
|---------|------------|-----------|-------------|------|
| AR-08-01 | T-08-01-04 | Explain output exposes only local built-in rule metadata needed by users. | Project owner via Phase 8 threat model | 2026-05-01 |
| AR-08-02 | T-08-03-04 | DOT output exposes local analysis metadata intentionally requested through graph commands. | Project owner via Phase 8 threat model | 2026-05-01 |

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By |
|------------|---------------|--------|------|--------|
| 2026-05-01 | 16 | 16 | 0 | Codex |

## Verification Evidence

- `cargo test -p polint-cli --test cli explain` passed.
- `cargo test -p polint-cli --test cli test_rules` passed.
- `cargo test -p polint-cli --test cli fail_on` passed.
- `cargo test -p polint-cli --test cli sarif` passed.
- `cargo test -p polint-cli --test cli graph` passed.
- `cargo test -p polint-diagnostics --lib sarif` passed.
- `cargo test -p polint-graph --lib` passed.
- `cargo fmt -- --check` passed.
- `cargo clippy --workspace --all-targets -- -D warnings` passed.
- `cargo test --workspace` passed.

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-05-01
