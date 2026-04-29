---
phase: 04
slug: go-adapter
status: verified
asvs_level: 1
threats_total: 16
threats_closed: 16
threats_open: 0
block_on: open_threats
created: 2026-04-29
verified: 2026-04-29
---

# Phase 04 Go Adapter Security Verification

## Scope

This report verifies only the declared Phase 04 threat register. It does not scan for new vulnerabilities.

Implementation files were read-only during this verification. Only this security report was written.

## Threat Verification

| Threat ID | Category | Component | Disposition | Status | Evidence |
|-----------|----------|-----------|-------------|--------|----------|
| T-04-01-01 | Denial of Service | `polint_go::analyze` | mitigate | CLOSED | `crates/polint-go/src/lib.rs:19-29` converts parser helper failures into `parser/go` diagnostics; `crates/polint-go/src/lib.rs:43-50` parses with tree-sitter and checks `root.has_error()`; `crates/polint-go/src/lib.rs:66-74` emits a stable parser diagnostic. |
| T-04-01-02 | Tampering | `AnalysisDb::push_package` | mitigate | CLOSED | `crates/polint-core/src/lib.rs:239-241` assigns `PackageId` from `self.packages.len()`; tests at `crates/polint-core/src/lib.rs:1031` and `crates/polint-core/src/lib.rs:1061-1062` assert deterministic IDs. |
| T-04-01-03 | Information Disclosure | `parser/go` diagnostics | accept | CLOSED | Accepted risk logged below. Diagnostic construction uses local path, parser range, and stable message at `crates/polint-go/src/lib.rs:66-74`; test asserts file/message at `crates/polint-go/src/lib.rs:1208-1210`. |
| T-04-01-04 | Spoofing | package extraction | accept | CLOSED | Accepted risk logged below. Package facts are extracted from `package_identifier` syntax and stored as `PackageFact`, not identity data, at `crates/polint-go/src/lib.rs:81-99`. |
| T-04-02-01 | Tampering | import/function/test fact insertion | mitigate | CLOSED | Deterministic traversal is by named-child index at `crates/polint-go/src/lib.rs:648-657`; import/function extraction uses tree order at `crates/polint-go/src/lib.rs:159-177` and `crates/polint-go/src/lib.rs:245-294`; exact fact-order tests assert imports/functions at `crates/polint-go/src/lib.rs:1257`, `crates/polint-go/src/lib.rs:1280`, `crates/polint-go/src/lib.rs:1300`, and `crates/polint-go/src/lib.rs:1339-1344`. |
| T-04-02-02 | Denial of Service | call/evidence extraction | mitigate | CLOSED | Source is parsed through borrowed `&str` after cloning only the `Arc<str>` handle at `crates/polint-go/src/lib.rs:34-45`; extraction walks existing parser nodes at `crates/polint-go/src/lib.rs:383-395`, `crates/polint-go/src/lib.rs:499-545`, and `crates/polint-go/src/lib.rs:648-657`; call vectors are sorted/deduped at `crates/polint-go/src/lib.rs:394-395`. |
| T-04-02-03 | Information Disclosure | import graph labels | accept | CLOSED | Accepted risk logged below. `ImportGraph::from_db` labels nodes from `file.relative_path` and `import.path` at `crates/polint-graph/src/lib.rs:18-30`; graph test exists at `crates/polint-go/src/lib.rs:1373-1388`. |
| T-04-02-04 | Spoofing | test evidence heuristics | mitigate | CLOSED | Heuristic status is documented in the plan summary at `.planning/phases/04-go-adapter/04-02-SUMMARY.md:23` and `.planning/phases/04-go-adapter/04-02-SUMMARY.md:103`; user-facing Go rule help avoids exact-coverage claims at `crates/polint-rules/src/lib.rs:238` and `crates/polint-rules/src/lib.rs:311`. |
| T-04-03-01 | Tampering | branch stable fingerprints | mitigate | CLOSED | Branch fingerprints use path, function name, start line/col, normalized condition, and edge at `crates/polint-go/src/lib.rs:1056-1066`; branch ID is only the DB-assigned placeholder field at `crates/polint-go/src/lib.rs:1068`; exclusion is tested at `crates/polint-go/src/lib.rs:2010`. |
| T-04-03-02 | Spoofing | error-path heuristic | mitigate | CLOSED | Helper is explicitly syntax-only at `crates/polint-go/src/lib.rs:1104-1113`; clear syntax patterns are matched at `crates/polint-go/src/lib.rs:1115-1128` and error-looking returns at `crates/polint-go/src/lib.rs:1144-1169`; test coverage starts at `crates/polint-go/src/lib.rs:1786`. |
| T-04-03-03 | Denial of Service | branch traversal | mitigate | CLOSED | Branch extraction receives existing function body nodes from `extract_functions` at `crates/polint-go/src/lib.rs:286-294` and traverses them deterministically at `crates/polint-go/src/lib.rs:673-685` via `visit_named_descendants` at `crates/polint-go/src/lib.rs:648-657`, with no branch-level reparsing path. |
| T-04-03-04 | Repudiation | branch diagnostics downstream | mitigate | CLOSED | Parser-derived branch spans are asserted at `crates/polint-go/src/lib.rs:1736` and `crates/polint-go/src/lib.rs:1763-1782`; stable fingerprint construction is at `crates/polint-go/src/lib.rs:1056-1066`; repeated-source stability is tested at `crates/polint-go/src/lib.rs:1982` and `crates/polint-go/src/lib.rs:2001`. |
| T-04-04-01 | Tampering | test `.polint.toml` profiles | mitigate | CLOSED | CLI tests create explicit temp configs at `crates/polint-cli/tests/cli.rs:160-166`, `crates/polint-cli/tests/cli.rs:250-256`, and `crates/polint-cli/tests/cli.rs:298-311`; parsed JSON assertions check exact rule IDs/evidence at `crates/polint-cli/tests/cli.rs:194`, `crates/polint-cli/tests/cli.rs:286`, and `crates/polint-cli/tests/cli.rs:341`. |
| T-04-04-02 | Denial of Service | invalid Go source through CLI | mitigate | CLOSED | Invalid source test is present at `crates/polint-cli/tests/cli.rs:160`; it asserts `parser/go` for `broken.go` at `crates/polint-cli/tests/cli.rs:194` and syntax-error text at `crates/polint-cli/tests/cli.rs:200-201` under the `--fail-on none` path. |
| T-04-04-03 | Information Disclosure | import-boundary diagnostic output | accept | CLOSED | Accepted risk logged below. The Go import-boundary rule reports local file/range and `import.path` evidence at `crates/polint-rules/src/lib.rs:145-158`; the fixture import is `net/http` at `crates/polint-cli/tests/cli.rs:311` and asserted as evidence at `crates/polint-cli/tests/cli.rs:341`. |
| T-04-04-04 | Spoofing | heuristic branch diagnostics | mitigate | CLOSED | Branch-obligation help states the rule is heuristic and does not prove exact coverage at `crates/polint-rules/src/lib.rs:238`; CLI test asserts help contains `heuristic` at `crates/polint-cli/tests/cli.rs:292`. |

## Accepted Risks Log

| Threat ID | Accepted Risk | Rationale | Evidence |
|-----------|---------------|-----------|----------|
| T-04-01-03 | `parser/go` diagnostics expose local file path, range, and a stable syntax-error message. | Local static analysis diagnostics require pointing developers to malformed local files. No source excerpt or external data is added by this mitigation. | `crates/polint-go/src/lib.rs:66-74`; `crates/polint-go/src/lib.rs:1208-1210`. |
| T-04-01-04 | Go package names are treated as syntax facts, not identity or authorization boundaries. | Semantic package verification is out of scope for this phase and no trust decision is based on package names. | `crates/polint-go/src/lib.rs:81-99`; `crates/polint-core/src/lib.rs:132-138`. |
| T-04-02-03 | Import graph DOT labels contain local relative file paths and import strings already present in analyzed source. | Graph output is local analysis output and reflects repository source facts already available to the user. | `crates/polint-graph/src/lib.rs:18-30`; `crates/polint-go/src/lib.rs:1373-1388`. |
| T-04-04-03 | Import-boundary diagnostics expose local file paths and import paths already present in analyzed source fixtures. | The rule must report the offending local import to be actionable; it does not add secret or external data. | `crates/polint-rules/src/lib.rs:145-158`; `crates/polint-cli/tests/cli.rs:311`; `crates/polint-cli/tests/cli.rs:341`. |

## Threat Flags

No unregistered flags.

`04-04-SUMMARY.md` reports: none; modified files added test fixtures and integration tests only, with no new production network endpoint, auth path, file access boundary, or schema trust boundary.

## Result

SECURED

## Sign-Off

- [x] All threats have a disposition (mitigate / accept / transfer)
- [x] Accepted risks documented in Accepted Risks Log
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-29
