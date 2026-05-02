# Milestones

## v1.0 MVP (Shipped: 2026-05-02)

**Phases completed:** 10 phases, 35 plans, 90 tasks

**Artifacts:** [roadmap archive](milestones/v1.0-ROADMAP.md), [requirements archive](milestones/v1.0-REQUIREMENTS.md), [milestone audit](milestones/v1.0-MILESTONE-AUDIT.md)

**Tag:** `v1.0`

**Key accomplishments:**

- Core fact storage, span conversion, and rule execution are now covered by deterministic unit/property tests with ordered parallel runner output.
- Diagnostics now carry the full Phase 3 contract, use stable full-range fingerprints, and have deterministic unit, property, and inline snapshot coverage.
- Deterministic discovery now has focused filesystem and CLI evidence, and Phase 3 status records reflect only the verified core/diagnostics scope.
- Tree-sitter-backed Go parser diagnostics and package-name facts now feed the core AnalysisDb without broad core refactors.
- Tree-sitter-backed Go imports, declarations, calls, complexity, and heuristic test evidence now feed core facts and graph/rule consumers.
- Parser-backed Go branch obligations with stable fingerprints and conservative syntax-only error-path flags.
- Expanded Go fixtures and JSON CLI integration tests now prove Phase 4 parser diagnostics, Go facts, branch obligations, and import-boundary diagnostics through `polint check`.
- Oxc-backed TS/JS parsing now emits parser/ts diagnostics, preserves recoverable import facts, and parses borrowed SourceFile text without full-source cloning.
- Oxc AST-backed TS/JS imports, functions, classes, methods, component heuristics, and call facts with a narrow core TsClassFact contract.
- Completed the remaining parser-backed Phase 5 TS/JS fact extraction and unit proof.
- Proved Phase 5 end to end through fixtures, CLI JSON tests, and full workspace gates.
- SDK-first RuleCtx helpers, prelude exports, and new-rule scaffolds for borrowed fact queries
- Exact literal allow-list config plus Go string literal and TS/JS regex literal facts for SDK rules
- SDK-facing Go/TS complexity, import-boundary, raw-color, and denied-literal rules with configured thresholds, allow-lists, and deterministic evidence
- SDK-facing Go heuristic rules with companion test evidence, weighted suite scores, and assertion evidence labels
- Parsed JSON CLI integration coverage for all eight SDK example rules with clean and failing fixture proof
- Representative human and JSON diagnostic snapshots for all Phase 6 example rule families, with full workspace verification passing
- Schema-aware cache keys and disabled-cache proof for the Phase 7 parser/fact cache.
- Source-free Go and TS/JS parser facts cached under `.polint/cache` with deterministic restoration.
- Rayon-backed file and adapter analysis with deterministic `AnalysisDb` merge and repeated CLI output proof.
- Closed Phase 7 with deterministic per-rule profiling output and end-to-end cache/performance verification.
- CLI command surface and exit-code contracts are covered by integration tests, and `test-rules` no longer corrupts machine-readable stdout.
- SARIF-like output now has renderer snapshot coverage and CLI integration proof for CI fields and fail thresholds.
- Import and function graph DOT exports now have unit and CLI coverage for deterministic, valid output.
- Phase 8 targeted and full workspace verification passed, with SARIF-like output hardened against feature-dependent JSON field ordering.
- Experimental plugin WIT contract now has typed metadata, diagnostics, capabilities, and stable-ID host fact queries.
- Plugin manifest loading now has typed validation, manifest-relative component resolution, and optional Wasmtime byte validation coverage.
- Experimental Wasm plugin docs now match the implemented WIT and Wasmtime loading skeleton, with full workspace verification passing.
- README now gives a complete v1 user path for installation, quickstart, config, SDK authoring, rule testing, CI, examples, release checks, and roadmap.
- Top-level examples now document the quickstart, custom rule authoring helpers, and runnable Go/TS built-in rule fixtures with honest v1 limitations.
- CLI integration coverage now proves mixed Go/TS fixtures and the configured Go/TS example directories run through `polint check`.
- Phase 10 closes with README/examples verified, targeted fixture/example CLI tests passing, and the full Rust workspace release matrix clean.

---
