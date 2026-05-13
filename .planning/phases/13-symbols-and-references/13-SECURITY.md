---
phase: 13-symbols-and-references
security_review: 13-SECURITY
status: secured
threats_open: 0
threats_total: 26
asvs_level: standard
block_on: open_threats
generated: 2026-05-13
---

# Phase 13 Security Verification

Verified only the threats declared in the Phase 13 plan threat models. All declared threats are classified as `mitigate`; no `accept` or `transfer` dispositions were present.

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-13-01-01 | Spoofing | mitigate | CLOSED | Stable IDs are derived from normalized stable keys in `crates/polint/src/symbol_graph/stable_id.rs:74` and hashed into core ID newtypes at `crates/polint/src/symbol_graph/stable_id.rs:184`; `replace_symbol_graph_facts` preserves provider IDs without renumbering at `crates/polint/src/core/mod.rs:673`. `rg "SymbolId\\(index|DefinitionId\\(index|ReferenceId\\(index"` returned no matches in symbol graph/core files. |
| T-13-01-02 | Tampering | mitigate | CLOSED | Macro mapping recognizes `Symbols` and `References` at `crates/polint-macros/src/lib.rs:318`; `Capabilities::references()` enables both `references` and `symbols` at `crates/polint/src/core/mod.rs:1182`. |
| T-13-01-03 | Information Disclosure | mitigate | CLOSED | Stable key material is language/module/package/file/owner/kind/name/span only at `crates/polint/src/symbol_graph/stable_id.rs:74`; normalization does not include source text at `crates/polint/src/symbol_graph/stable_id.rs:254`; a regression rejects source snippets in stable keys at `crates/polint/src/symbol_graph/stable_id.rs:451`. |
| T-13-01-04 | Denial of Service | mitigate | CLOSED | `AnalysisDb` has BTree lookup indexes for symbol/reference hot paths at `crates/polint/src/core/mod.rs:538`; SDK views delegate to indexed query helpers for file/name/target lookups at `crates/polint/src/sdk/facts.rs:377` and `crates/polint/src/sdk/facts.rs:425`. |
| T-13-02-01 | Tampering | mitigate | CLOSED | Stable key parts are length-prefixed before hashing at `crates/polint/src/symbol_graph/stable_id.rs:228` and `crates/polint/src/symbol_graph/stable_id.rs:239`; boundary-change regression exists at `crates/polint/src/symbol_graph/stable_id.rs:459`. |
| T-13-02-02 | Repudiation | mitigate | CLOSED | Builder records ID-to-stable-key collisions and emits deterministic `polint/internal` diagnostics with ID and stable key evidence at `crates/polint/src/symbol_graph/model.rs:347`; collision regression exists at `crates/polint/src/symbol_graph/model.rs:824`. |
| T-13-02-03 | Information Disclosure | mitigate | CLOSED | Stable key fields are normalized metadata at `crates/polint/src/symbol_graph/stable_id.rs:74`; debug-safe/no-source regression at `crates/polint/src/symbol_graph/stable_id.rs:451`. |
| T-13-02-04 | Denial of Service | mitigate | CLOSED | SymbolGraphBuilder stages facts in `BTreeMap`/`BTreeSet` collections at `crates/polint/src/symbol_graph/model.rs:22`; finish performs deterministic sort passes at `crates/polint/src/symbol_graph/model.rs:246`; query helpers delegate to AnalysisDb indexes at `crates/polint/src/symbol_graph/query.rs:5`. |
| T-13-03-01 | Tampering | mitigate | CLOSED | Runner order is syntax adapters, module graph, symbol graph, metrics, then rules with merged support at `crates/polint/src/runner/mod.rs:163`; capability support is overlaid before rule execution at `crates/polint/src/runner/mod.rs:175`. |
| T-13-03-02 | Repudiation | mitigate | CLOSED | `SymbolGraphDerivation` emits `polint/capability` diagnostics with rule, capability, language, status, reason, docs path, and hint evidence at `crates/polint/src/symbol_graph/mod.rs:98`. |
| T-13-03-03 | Information Disclosure | mitigate | CLOSED | Symbol graph language modules are crate-private at `crates/polint/src/symbol_graph/mod.rs:1`; crate root exports `symbol_graph` only as `pub(crate)` at `crates/polint/src/lib.rs:33`; public SDK prelude exports normalized polint facts/views only at `crates/polint/src/sdk/mod.rs:27`. |
| T-13-03-04 | Denial of Service | mitigate | CLOSED | Symbol derivation returns default unless `symbols` or `references` are requested at `crates/polint/src/symbol_graph/mod.rs:47`; module graph expands its trigger list only for symbol/reference capability names at `crates/polint/src/module_graph/mod.rs:19`. |
| T-13-04-01 | Tampering | mitigate | CLOSED | TS provider keeps Oxc IDs local while emitting normalized `SymbolDraft` fields with repo-relative file key, owner chain, name, span, and precision at `crates/polint/src/symbol_graph/ts.rs:207`; public prelude contains no Oxc exports at `crates/polint/src/sdk/mod.rs:27`. |
| T-13-04-02 | Information Disclosure | mitigate | CLOSED | TS stable keys use repo-relative keys without source snippets, with regression at `crates/polint/src/symbol_graph/ts.rs:1155`; public docs explicitly state no Oxc IDs, raw semantic records, AST nodes, or resolver internals are exposed at `docs/facts/symbols-and-references.md:285`. |
| T-13-04-03 | Denial of Service | mitigate | CLOSED | TS extraction only handles TS-family files and exits when no symbol/reference capability is requested at `crates/polint/src/symbol_graph/ts.rs:68`; it borrows `SourceFile.source` with `file.source.as_ref()` at `crates/polint/src/symbol_graph/ts.rs:151`. |
| T-13-04-04 | Spoofing | mitigate | CLOSED | Module-linked import aliases resolve through module graph and unique exported symbols, using `ModuleLinked`, `Unresolved`, `Ambiguous`, `SetupMissing`, or `Unsupported` states at `crates/polint/src/symbol_graph/ts.rs:379`; external CLI proof asserts `ModuleLinked` evidence at `crates/polint/tests/cli.rs:4134`. |
| T-13-05-01 | Elevation of Privilege | mitigate | CLOSED | Rust invokes the sidecar with fixed `Command::new("go")` args, `current_dir(root)`, fixed sidecar path, fixed `go.work`, and `env_remove("GOFLAGS")` at `crates/polint/src/symbol_graph/go.rs:244`; no shell invocation is used. |
| T-13-05-02 | Tampering | mitigate | CLOSED | Rust validates schema `polint-go-symbols-v1` before conversion at `crates/polint/src/symbol_graph/go.rs:285`; all sidecar file paths are normalized, absolute/repo-escaping paths rejected, and paths must map to discovered Go `FileId`s at `crates/polint/src/symbol_graph/go.rs:298`. |
| T-13-05-03 | Information Disclosure | mitigate | CLOSED | Sidecar JSON row structs contain metadata fields only at `tools/polint-go-symbols/internal/symbols/emit.go:27`; regression rejects raw source markers and `Source`/`Content` fields at `tools/polint-go-symbols/internal/symbols/emit_test.go:166`. |
| T-13-05-04 | Denial of Service | mitigate | CLOSED | Go derivation returns default unless Go files exist and symbol/reference capabilities are requested at `crates/polint/src/symbol_graph/go.rs:127`; package patterns/build tags/include tests are configurable at `crates/polint/src/symbol_graph/go.rs:191`; sidecar package loading uses the requested package modes/patterns at `tools/polint-go-symbols/internal/symbols/emit.go:129`. |
| T-13-05-05 | Repudiation | mitigate | CLOSED | Setup-missing support rows include capability, Go language, reason, hint, and docs path at `crates/polint/src/symbol_graph/go.rs:415`; package load diagnostics include package ID/path, reason, and docs path at `crates/polint/src/symbol_graph/go.rs:675`. |
| T-13-06-01 | Spoofing | mitigate | CLOSED | External rule test guard requires `use polint::sdk::prelude::*`, typed `Symbols<'_>`/`References<'_>`, and rejects internal imports/manual capability paths at `crates/polint/tests/cli.rs:403`; external TS/Go tests consume public SDK views at `crates/polint/tests/cli.rs:4090` and `crates/polint/tests/cli.rs:4158`. |
| T-13-06-02 | Tampering | mitigate | CLOSED | Planner promotes `symbols` and `references` to supported rows at `crates/polint/src/analysis_plan.rs:588`; tests assert supported plan rows at `crates/polint/src/analysis_plan.rs:1021`; Go setup-missing still blocks requesting rules at `crates/polint/tests/cli.rs:4053`. |
| T-13-06-03 | Repudiation | mitigate | CLOSED | External CLI helpers parse stdout as `PolintReport` at `crates/polint/tests/cli.rs:3956`; tests assert evidence for symbol/reference IDs, target, status, and precision at `crates/polint/tests/cli.rs:3914`, `crates/polint/tests/cli.rs:4116`, and `crates/polint/tests/cli.rs:4181`. |
| T-13-06-04 | Information Disclosure | mitigate | CLOSED | Public docs limit exposure to IDs/names/spans/status/precision and explicitly exclude Oxc IDs, AST nodes, Go object values, addresses, package loader JSON, and sidecar DTOs at `docs/facts/symbols-and-references.md:285` and `docs/facts/symbols-and-references.md:309`; generated skill guidance repeats limits at `crates/polint/src/cli/skill.rs:353`. |
| T-13-06-05 | Denial of Service | mitigate | CLOSED | External CLI tests are targeted temp-repo tests at `crates/polint/tests/cli.rs:4027`, `crates/polint/tests/cli.rs:4090`, and `crates/polint/tests/cli.rs:4158`; final verification records full workspace gates passing in `.planning/phases/13-symbols-and-references/13-VERIFICATION.md`. |

## Threat Flags

No `## Threat Flags` sections were present in the Phase 13 summary, review-fix, or verification artifacts. No unregistered flags were recorded.

## Summary

- Threats closed: 26/26
- Threats open: 0/26
- Implementation files modified during security verification: none
- Security artifact written: `.planning/phases/13-symbols-and-references/13-SECURITY.md`
