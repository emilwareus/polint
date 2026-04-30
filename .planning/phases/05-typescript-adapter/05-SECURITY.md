---
phase: 05-typescript-adapter
slug: typescript-adapter
status: verified
asvs_level: 1
threats_total: 16
threats_closed: 16
threats_open: 0
block_on: open_threats
created: 2026-04-30
verified: 2026-04-30
---

# Phase 05 TypeScript Adapter Security Verification

## Scope

This report verifies only the declared Phase 05 STRIDE threat registers from `05-01-PLAN.md` through `05-04-PLAN.md`. It does not scan for new broad vulnerabilities.

Implementation files were read-only during this verification. Only this security report was written.

No transfer dispositions were declared in the Phase 05 threat registers.

## Threat Verification

| Threat ID | Category | Component | Disposition | Status | Evidence |
|-----------|----------|-----------|-------------|--------|----------|
| T-05-01-01 | Denial of Service | `polint_ts::parse_ts_file` | mitigate | CLOSED | `crates/polint-ts/src/lib.rs:54-75` converts `parsed.errors` into `parser/ts` diagnostics; `crates/polint-ts/src/lib.rs:78-84` handles `parsed.panicked`; extraction continues at `crates/polint-ts/src/lib.rs:87-110` and returns normally at `crates/polint-ts/src/lib.rs:112`. |
| T-05-01-02 | Tampering | Oxc-to-core span conversion | mitigate | CLOSED | Parser diagnostic labels use `span_from_byte_range` at `crates/polint-ts/src/lib.rs:60-66`; `span_from_oxc` bridges Oxc spans through core span conversion at `crates/polint-ts/src/lib.rs:131-132`; span behavior is asserted at `crates/polint-ts/src/lib.rs:3317-3360`. |
| T-05-01-03 | Information Disclosure | `parser/ts` diagnostics | accept | CLOSED | Accepted risk logged below. Diagnostics use `db.path_for(file_id)`, a range, and a syntax-error message at `crates/polint-ts/src/lib.rs:70-74`; `path_for` returns relative paths at `crates/polint-core/src/lib.rs:349-352`; CLI JSON tests assert only file and message fields at `crates/polint-cli/tests/cli.rs:375-385`. |
| T-05-01-04 | Spoofing | SourceType selection | mitigate | CLOSED | `parse_source_type` uses `SourceType::from_path(path).unwrap_or_default()` at `crates/polint-ts/src/lib.rs:115-116`, and `parse_ts_file` derives `source_type` from `file.path` before Oxc parsing at `crates/polint-ts/src/lib.rs:48-51`. |
| T-05-02-01 | Tampering | `AnalysisDb::push_ts_class` and TS traversal | mitigate | CLOSED | `TsClassFact` storage is append-only at `crates/polint-core/src/lib.rs:223-224` and `crates/polint-core/src/lib.rs:289-290`; core insertion-order tests assert exact class order at `crates/polint-core/src/lib.rs:854-890`; TS adapter tests assert import/function/call order at `crates/polint-ts/src/lib.rs:3102-3158`. |
| T-05-02-02 | Spoofing | component detection | mitigate | CLOSED | Component detection is explicitly labeled with `syntax-level component heuristic` comments at `crates/polint-ts/src/lib.rs:938` and `crates/polint-ts/src/lib.rs:970`; heuristic behavior is tested at `crates/polint-ts/src/lib.rs:3177-3205`. |
| T-05-02-03 | Denial of Service | recursive AST call extraction | mitigate | CLOSED | Call vectors are sorted/deduped per function at `crates/polint-ts/src/lib.rs:922-923`; function body calls traverse parsed statement bodies once and dedupe at `crates/polint-ts/src/lib.rs:1019-1028`; class method extraction feeds the same bounded helper at `crates/polint-ts/src/lib.rs:979-999`. |
| T-05-02-04 | Elevation of Privilege | new `Capabilities::ts_classes` | accept | CLOSED | Accepted risk logged below. The capability is a boolean fact-availability flag at `crates/polint-core/src/lib.rs:370-374` with a builder at `crates/polint-core/src/lib.rs:429-432`; tests assert it only toggles local capability metadata at `crates/polint-core/src/lib.rs:926-929`. |
| T-05-03-01 | Tampering | string/template extraction | mitigate | CLOSED | Static templates are exact only when `template.expressions.is_empty()`; dynamic templates emit non-empty static quasis only at `crates/polint-ts/src/lib.rs:2627-2638`; the regression test asserts no synthetic expression-containing exact value at `crates/polint-ts/src/lib.rs:3368-3385`. |
| T-05-03-02 | Spoofing | raw color literal facts | accept | CLOSED | Accepted risk logged below. Raw-color diagnostics consume local `StringLiteralFact` values at `crates/polint-rules/src/lib.rs:180-195`; the rule description is bounded to detecting raw color literals at `crates/polint-rules/src/lib.rs:170-171`; fixture/example coverage is local source syntax at `tests/fixtures/ts/failing/component.tsx:10-12` and `examples/ts-design-tokens/Button.tsx:2-5`. |
| T-05-03-03 | Denial of Service | recursive expression/statement traversal | mitigate | CLOSED | Literal extraction starts from the parsed program at `crates/polint-ts/src/lib.rs:2283-2308`; complexity starts at parsed function bodies at `crates/polint-ts/src/lib.rs:1253-1268` and counts AST control-flow/logical nodes at `crates/polint-ts/src/lib.rs:1281-1335` and `crates/polint-ts/src/lib.rs:1374-1384`; anti-pattern scan found no old whole-source scan helpers in the Phase 05 implementation files. |
| T-05-03-04 | Information Disclosure | import graph DOT labels | accept | CLOSED | Accepted risk logged below. `ImportGraph::from_db` labels graph nodes from `file.relative_path`, `db.path_for(import.file)`, and `import.path` at `crates/polint-graph/src/lib.rs:16-30`; TS import graph proof is at `crates/polint-ts/src/lib.rs:3674-3687`. |
| T-05-04-01 | Repudiation | CLI JSON integration tests | mitigate | CLOSED | CLI tests parse stdout as JSON through `stdout_json` and query diagnostics structurally at `crates/polint-cli/tests/cli.rs:13-33`; parser JSON assertions check `rule_id`, `file`, and message at `crates/polint-cli/tests/cli.rs:375-385`; full-profile assertions check rule IDs, evidence, file, and message at `crates/polint-cli/tests/cli.rs:475-498`. |
| T-05-04-02 | Tampering | fixtures | mitigate | CLOSED | Clean fixture syntax is present at `tests/fixtures/ts/clean/component.tsx:1-29`; failing fixture syntax is present at `tests/fixtures/ts/failing/component.tsx:1-45`; mixed fixture syntax is present at `tests/fixtures/mixed/view.ts:1-7`; CLI tests copy fixtures/examples into temp repos at `crates/polint-cli/tests/cli.rs:399-401`, `crates/polint-cli/tests/cli.rs:453-455`, and `crates/polint-cli/tests/cli.rs:516-518`. |
| T-05-04-03 | Denial of Service | workspace verification | mitigate | CLOSED | Phase 05 plan 04 records targeted tests, `cargo fmt -- --check`, `cargo test -p polint-ts --lib`, `cargo test -p polint-core --lib ts_class`, workspace clippy, and workspace tests passing at `.planning/phases/05-typescript-adapter/05-04-SUMMARY.md:84-92`; phase verification repeats workspace clippy/tests pass evidence at `.planning/phases/05-typescript-adapter/05-VERIFICATION.md:88-93`. |
| T-05-04-04 | Spoofing | heuristic TS diagnostics | mitigate | CLOSED | Existing TS rule wording stays bounded to TS/JS complexity and raw color literals at `crates/polint-rules/src/lib.rs:77-78` and `crates/polint-rules/src/lib.rs:170-171`; CLI tests assert facts and rule IDs without React/CSS/type-semantic claims at `crates/polint-cli/tests/cli.rs:430-498`; Phase 05 summary confirms no semantic TS analysis or exact semantic claims were added at `.planning/phases/05-typescript-adapter/05-03-SUMMARY.md:120-122` and `.planning/phases/05-typescript-adapter/05-04-SUMMARY.md:107-109`. |

## Accepted Risks Log

| Threat ID | Accepted Risk | Rationale | Bound |
|-----------|---------------|-----------|-------|
| T-05-01-03 | `parser/ts` diagnostics expose local relative file path, line/column range, and Oxc syntax-error wording. | Parser diagnostics must identify the malformed local file to be actionable. The output does not add source excerpts, secrets, external data, or absolute paths beyond the local analysis result. | Local TS/JS static analysis diagnostics only; evidence at `crates/polint-ts/src/lib.rs:70-74` and `crates/polint-core/src/lib.rs:349-352`. |
| T-05-02-04 | `Capabilities::ts_classes` exposes whether TS class facts are available. | The capability is metadata for rule authors and does not grant file, network, process, or privilege access. | Local in-process rule capability declaration only; evidence at `crates/polint-core/src/lib.rs:370-374` and `crates/polint-core/src/lib.rs:429-432`. |
| T-05-03-02 | Raw color facts are syntax-level literal facts and may be interpreted as policy signals by users. | Phase 05 deliberately reports source syntax facts, not CSS semantic validation or design-token authority. The rule remains an advisory local lint diagnostic. | Local string/JSX literal facts consumed by local rules; evidence at `crates/polint-rules/src/lib.rs:170-195`. |
| T-05-03-04 | Import graph DOT labels expose local relative paths and module specifier strings. | The graph helper visualizes imports already present in analyzed source and uses local repository paths/module strings needed for useful output. | Local DOT output only; no resolver, network lookup, or external disclosure path is added. Evidence at `crates/polint-graph/src/lib.rs:16-30`. |

## Threat Flags

No unregistered flags.

- `05-01-SUMMARY.md`: none; modified surface stayed within the planned repository-source-to-Oxc parser, diagnostic conversion, and TS-adapter-to-core-DB boundaries.
- `05-02-SUMMARY.md`: none; modified surface stayed within Oxc AST to core facts, SDK-facing core facts, and component heuristic boundaries.
- `05-03-SUMMARY.md`: none; no resolver behavior, graph commands, CSS parsing, or exact semantic claims were added.
- `05-04-SUMMARY.md`: none; fixture and assertion changes did not broaden CLI command behavior, graph commands, SARIF behavior, module resolution, or semantic TS analysis.

## Security Audit 2026-04-30

| Metric | Count |
|--------|-------|
| Threats found | 16 |
| Closed | 16 |
| Open | 0 |
| Accepted risks | 4 |
| Transfers | 0 |

## Result

SECURED

## Sign-Off

- [x] All threats have a disposition: mitigate or accept
- [x] Accepted risks are documented and bounded
- [x] Threat flags from all Phase 05 summaries are incorporated
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter

**Approval:** verified 2026-04-30
