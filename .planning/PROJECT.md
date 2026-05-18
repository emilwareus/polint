# polint

## What This Is

polint is a high-performance Rust framework for writing repo-local static-analysis rules across multiple languages. Adapters today cover **Go** (tree-sitter) and **TypeScript / JavaScript** (Oxc); more languages can be added through the same adapter contract. It initially supports Go and TypeScript/JavaScript and gives rule authors reusable infrastructure for file discovery, parsing, typed facts, diagnostics, local rule execution, and CI output.

The product is for engineering teams using AI-assisted development who need executable project-specific policies instead of repeating local conventions in prompts. It is not a replacement for ESLint, Ruff, Biome, golangci-lint, or formatters; it is a framework for checks that those generic tools cannot know.

## Core Value

Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

## Current State

v1.0 MVP shipped on 2026-05-02. It includes the Rust workspace, CLI/config/discovery loop, deterministic core facts and diagnostics, Go and TypeScript/JavaScript adapters, SDK and self-contained example-local rules, cache/performance support, CI output, README/examples, and final release verification.

Phase 11 of v1.1 completed on 2026-05-09. Declared rule capabilities now produce an internal deterministic `AnalysisPlan`, the child rule host passes that plan to Go and TS/JS adapters before harvesting, adapter cache keys include the plan digest, and unsupported reserved capabilities produce structured `polint/capability` diagnostics.

Phase 12 of v1.1 completed on 2026-05-11. Syntactic Go and TS/JS imports now flow into setup-aware resolved import facts and module/file/package/dependency graph facts exposed through typed SDK views for repo-local architecture rules.

Phase 13 of v1.1 completed in May 2026. `Symbols<'_>` and `References<'_>` are available through the SDK, with Go and TS/JS symbol/reference facts, stable IDs, and precision tiers.

Static-analysis engine research completed on 2026-05-16. `research/ROADMAP.md` now defines the next implementation sequence: 22 independently reviewable PR-sized steps that build the private analysis kernel, evaluation harness, cache substrate, semantic backbone, interprocedural engine foundations, extension surface, precision layers, benchmark gates, and final SDK/query promotion path.

Phase 20 of v1.2 completed on 2026-05-16. Current source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics orchestration now runs behind a crate-private analysis kernel facade with deterministic internal provider manifests and test-only provider-order inspection, preserving existing public behavior.

Phase 21 of v1.2 completed on 2026-05-17. Existing kernel-produced fact families now carry crate-private sidecar metadata for provenance, precision, confidence, validation status, stable keys, and deterministic merge validation, with internal debug JSON for files/imports/symbols/references and no public metadata surface.

Phase 22 of v1.2 completed on 2026-05-17. polint now has a crate-private internal evaluation harness MVP with deterministic expected/observed JSON, total-order report normalization, output hashes that exclude transient runtime data, generic matchers, unified metrics, and native fixtures covering kernel, provenance, cache, and extension invariants, with no public CLI, SDK, runner, or check JSON surface.

Archived milestone records:

- `.planning/milestones/v1.0-ROADMAP.md`
- `.planning/milestones/v1.0-REQUIREMENTS.md`
- `.planning/milestones/v1.0-MILESTONE-AUDIT.md`

## Current Milestone: v1.2 Static Analysis Engine Implementation

**Goal:** Turn the completed static-analysis research roadmap into a private, validated, cache-aware, agent-extensible analysis engine while preserving public API discipline.

**Target features:**

- Private analysis kernel facade and provider manifests for existing providers.
- Provenance, precision, validation, stable-key, and merge metadata for fact families.
- Internal evaluation harness with deterministic fixtures and promotion metrics.
- Input snapshots, typed cache keys, persistent layer cache, and conservative invalidation.
- Rule manifests, `polint inspect rule`, and first `polint test` fixture runner.
- Deepened semantic index and layered module/package/topology graph.
- Private semantic MIR, place identity, CFG/control dependence, and direct call facts.
- Abstract-domain, summary, demand-query, and summary-SCC cache foundations.
- Repo-local Rust extension/provider sink with validation, provenance, precision ceilings, and cache participation.
- Framework entrypoint, lifecycle, dispatch, and trust-boundary facts.
- P0 type/value/place/alias substrate, refined call graph providers, data-flow facts, slicing, paths, and evidence bundles.
- Benchmark adapters, promotion gates, and validated SDK query/agent ergonomics.

The longer-term target remains a complete, agent-consumable static-analysis graph of the codebase: modules, symbols, references, calls, CFGs, summaries, type/value/alias facts, dataflow, taint, evidence, and extension-provided repository knowledge. v1.2 should build the substrate first, keep internals private by default, and promote public SDK/CLI contracts only when fixtures and benchmark gates prove them.

## Requirements

### Validated

- [x] Create a compiling Rust 2024 workspace with clear crate boundaries for CLI, config, diagnostics, filesystem, cache, core analysis, SDK, Go, TS, internal relationship helpers, and example rules. Validated in Phase 1: Workspace Foundation.
- [x] Provide the first CLI loop for `polint init`, `polint new-rule`, and `polint check` with profiles, output formats, cache disabling, and fail thresholds. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Load `.polint.toml` with include/exclude globs, profiles, rule paths, severity overrides, language settings, and sane defaults when config is missing. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Discover Go, TS, TSX, JS, and JSX files with `.gitignore`, include glob, and exclude glob support. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Render `polint check` diagnostics as parseable JSON. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Make file discovery output deterministic by sorting normalized root-relative paths after `.gitignore`, include/exclude, and language filtering, with deterministic `AnalysisDb` file ID insertion. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Define stable v1 core fact models and deterministic in-run IDs for files, spans, functions, imports, branch obligations, tests, coverage placeholders, and analysis database accessors. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Run rules through the core registry with capability declarations, severity/options, deterministic sequential/parallel output, deduplication, and panic/error containment. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Provide the Phase 3 diagnostic contract: severities, labels, evidence, suggestions/fixes, help text, stable fingerprints, deterministic sort/dedupe, and human/JSON rendering coverage. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Implement Go analysis with tree-sitter-go for packages, imports, functions, methods, tests, branch obligations, import relationship facts, and cyclomatic complexity foundations. Validated in Phase 4: Go Adapter.
- [x] Implement TypeScript/JavaScript analysis with Oxc for parser diagnostics, imports/exports, functions, classes, JSX attributes, string literals, component heuristics, cyclomatic complexity, and import relationship facts. Validated in Phase 5: TypeScript Adapter.
- [x] Provide a public `polint-sdk` with a clean `Rule` trait, capability declarations, typed fact-view authoring, and helpers for reporting diagnostics. Validated in Phase 6: SDK and Example Rules.
- [x] Ship example rules that dogfood the same SDK users will use, including Go complexity, TS complexity, Go import boundaries, TS raw color detection, Go branch obligations, Go test suite size, Go assertion-after-action, and configured denied literals. Validated in Phase 6: SDK and Example Rules.
- [x] Support repo-local Rust rule scaffolding through `polint new-rule` and document the native registration boundary without claiming dynamic loading. Validated in Phase 6: SDK and Example Rules.
- [x] Add a hash-based cache that can be disabled, deterministic parallel file/parser/rule execution, and internal timing instrumentation. Validated in Phase 7: Cache and Performance.
- [x] Harden CI-facing command behavior and final exit-code semantics. Validated in Phase 8: CI Output and Graph Commands. Later cleanup removed internal debug commands from the public CLI surface.
- [x] Keep file discovery and execution scalable enough for v1 through deterministic parallel reads/parsing/rule execution and cache support. Validated in Phase 7: Cache and Performance.
- [x] Harden SARIF-like diagnostics, fail thresholds, exit code semantics, and CI-facing command behavior. Validated in Phase 8: CI Output and Graph Commands.
- [x] Provide meaningful unit, integration, snapshot, and property tests for the core behavior. Validated across Phases 1–9, with final traceability closed in Phase 9: Docs, Examples, and Release Hardening.
- [x] Write a README that explains the goal, non-goals, quickstart, custom rule authoring, CI usage, and roadmap. Validated in Phase 9: Docs, Examples, and Release Hardening.
- [x] Let rule authors declare capabilities and inspect an explicit analysis plan derived from enabled rules. Validated in Phase 11: Capability-Driven Analysis Plan.
- [x] Pass the resolved analysis plan to Go and TS/JS adapters before fact harvesting. Validated in Phase 11: Capability-Driven Analysis Plan.
- [x] Include requested capabilities and setup-sensitive analysis inputs in cache identity through the plan digest. Validated in Phase 11: Capability-Driven Analysis Plan.
- [x] Report missing or unsupported requested capabilities as clear diagnostics or structured warnings. Validated in Phase 11: Capability-Driven Analysis Plan.
- [x] Let rule authors read resolved import facts and unresolved import reasons through typed SDK fact views. Validated in Phase 12: Resolved Imports and Module Relationships.
- [x] Resolve TS/JS imports through project-aware resolver setup including relative paths, package metadata, and `tsconfig` aliases where available. Validated in Phase 12: Resolved Imports and Module Relationships.
- [x] Resolve Go imports through Go package/module metadata where setup is available. Validated in Phase 12: Resolved Imports and Module Relationships.
- [x] Expose file, package, module, and dependency relationships for architecture rules through typed module graph facts. Validated in Phase 12: Resolved Imports and Module Relationships.
- [x] Let rule authors read symbol, definition, and reference facts through typed SDK fact views. Validated in Phase 13: Symbols and References.
- [x] Populate Go symbols and references from typed package information where setup is available. Validated in Phase 13: Symbols and References.
- [x] Populate TS/JS symbols and references from Oxc semantic facts where setup is available. Validated in Phase 13: Symbols and References.
- [x] Expose symbol/reference precision tiers and stable IDs suitable for diagnostics and cache restore. Validated in Phase 13: Symbols and References.
- [x] **SAE-FND-01**: polint has a private analysis kernel facade with provider manifests for existing source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics providers, preserving current behavior. Validated in Phase 20: Private Analysis Kernel Facade.
- [x] **SAE-FND-02**: Existing fact families carry internal provenance, precision, confidence, validation status, stable-key metadata, and deterministic merge validation. Validated in Phase 21: Provenance, Precision, and Validation Metadata.
- [x] **SAE-FND-03**: polint has an internal evaluation harness MVP with deterministic expected/observed JSON, matchers, metrics, and native fixtures for kernel, provenance, cache, and extension invariants. Validated in Phase 22: Internal Evaluation Harness MVP.

### Active

- [ ] **SAE-FND-04**: polint records input snapshots, typed cache keys, provider output metadata, cache stats, and lifecycle/toolchain/rule/model digest inputs needed for correct cache invalidation.
- [ ] **SAE-FND-05**: Existing cheap fact layers persist through a conservative layer cache with dependency indexes, change sets, hit/miss reporting, and stale-reuse safeguards.
- [ ] **SAE-FND-06**: Rule macro metadata generates rule manifests, `polint inspect rule --format json` is available as an intentional CLI surface, and the first `polint test` fixture runner proves public-SDK rule behavior.
- [ ] **SAE-SEM-01**: The semantic index includes scopes, richer imports, resolution facts, aliases, generated-symbol hooks, unresolved references, stable export identities, and language-owned Go and TS/JS providers.
- [ ] **SAE-SEM-02**: The module/package/topology graph models workspace roots, packages/projects/source sets, declared requirements, lockfile/tool-resolved edges, import-to-package facts, and repo topology overlays for Go and TS/JS.
- [ ] **SAE-SEM-03**: polint has a private semantic MIR and normalized place identity for Go and TS/JS function bodies, with deterministic lowering snapshots and explicit unsupported operations.
- [ ] **SAE-SEM-04**: polint builds local CFG, dominance, postdominance, and control-dependence facts over MIR for supported Go and TS/JS constructs.
- [ ] **SAE-SEM-05**: polint records direct call-site, direct target, and unresolved-call facts with call indexes and debug snapshots while keeping public whole-program call graph views unsupported.
- [ ] **SAE-INT-01**: polint has a P0 abstract-domain kernel with lattice/transfer traits, deterministic worklist solving, and first local domains for reachability, nilness/nullishness, truthiness, constants, simple strings, and cheap initializedness.
- [ ] **SAE-INT-02**: polint has a summary kernel with summary keys, typed summary domains, local/direct summaries, control effects, return/TITO, memory-touch approximations, resource/external effects, and summary metadata.
- [ ] **SAE-INT-03**: polint has an internal demand-query layer, summary SCC scheduling/cache, extension-aware cache quarantine, and query trace/debug output for expensive analyses.
- [ ] **SAE-INT-04**: polint has a repo-local Rust extension/provider sink with typed sinks, declared read sets, validation, precision ceilings, provenance, activation status, fixture requirements, and cache-key participation.
- [ ] **SAE-INT-05**: polint models framework entrypoints, lifecycle callbacks, dispatch, jobs, CLIs, MCP tools/resources/prompts, tests, generated dispatch, and trust boundaries with Go and TS/JS defaults plus extension overlays.
- [ ] **SAE-PREC-01**: polint has a P0 type/value/place/alias substrate with declared/inferred/narrowed type facts, value/allocation facts, access-path facts, local narrowing, and explicit alias statuses.
- [ ] **SAE-PREC-02**: polint has opt-in refined call graph providers over direct calls, entrypoints, summaries, type/value facts, function tokens, receiver types, and bounded points-to constraints with explicit unresolved and budget-exceeded statuses.
- [ ] **SAE-PREC-03**: polint has local and summary-projected data-flow facts, source/sink/sanitizer/barrier model sinks, budgets, unknown/havoc facts, and query-scoped path search.
- [ ] **SAE-PREC-04**: polint has internal slicing, path explanation, structured evidence nodes/edges, ranked paths, summary expansion handles, provenance-rich diagnostic evidence, and JSON/SARIF evidence rendering.
- [ ] **SAE-PROM-01**: polint has external benchmark adapters and promotion gates that record default-vs-extension deltas, runtime, memory, cache reuse, unknown counts, graph/path metrics, and accepted/rejected extension facts.
- [ ] **SAE-PROM-02**: Validated typed SDK query views and agent ergonomics are promoted only where contracts are proven, including bounded query builders and stable JSON for accepted public commands.

### Out of Scope

- Becoming a comprehensive bundled lint ruleset - the product value is custom rule infrastructure.
- Replacing existing language linters or formatters - users should keep ESLint, Biome, golangci-lint, rustfmt, and similar tools.
- Full Go type checking in the first pass - leave a trait boundary for a future `go/packages` or `go/analysis` sidecar.
- Full dynamic branch coverage in the first pass - design the model so exact coverage can be added later.
- Fully automatic compilation/loading of repo-local Rust rules in the first pass - scaffolding, SDK, and native registration are sufficient for v1.
- Perfect semantic precision in the first implementation of every capability - v1.2 should expose precision tiers and useful facts incrementally instead of pretending all dynamic or setup-sensitive behavior is exact.
- Public broad analysis APIs before validation - v1.2 should keep new analysis modules private by default and promote only deliberately scoped SDK/CLI contracts.
- Replacing current user behavior while building the static-analysis engine substrate - existing CLI, SDK, cache, and rule behavior must keep working unless a phase explicitly changes a reviewed contract.
- Running user test suites inside polint by default - coverage should be imported from reports that users or CI already produce.
- Exposing raw language-tool output as the public SDK - rule authors should consume normalized polint facts, not raw `go/packages`, Oxc, Python, javac, Maven, Gradle, or coverage report structures.
- Python and Java parity before Go and TS/JS capability coverage - Go and TS/JS should prove the complete model first; Python and Java start with declared subsets and expand later.

## Context

- The implementation target is Rust 2024 on stable Rust. Current local toolchain check: `rustc 1.94.0` and `cargo 1.94.0`.
- Current crate checks with `cargo search` on 2026-04-28 found compatible latest versions for the requested baseline: `clap 4.6.1`, `serde 1.0.228`, `serde_json 1.0.149`, `toml 1.1.2+spec-1.1.0`, `anyhow 1.0.102`, `thiserror 2.0.18`, `rayon 1.12.0`, `ignore 0.4.25`, `globset 0.4.18`, `petgraph 0.8.3`, `tree-sitter 0.26.8`, `tree-sitter-go 0.25.0`, Oxc `0.128.0`, `oxc_resolver 11.19.1`, `insta 1.47.2`, `assert_cmd 2.2.1`, `predicates 3.1.4`, `tempfile 3.27.0`, and `proptest 1.11.0`.
- The initial project prompt lives at `docs/INITIAL_PROMPT.md`.
- The source repository on GitHub is **`https://github.com/emilwareus/polint`**; this checkout and GSD planning live under **`/Users/emilwareus/Development/exlint`** on branch `main` until the local directory is renamed.
- The public CLI and crate names use the **`polint-*`** prefix, consistent with the repository name.
- Phase 1 completed on 2026-04-28 through GSD plan execution and verification on `main`.
- Phase 2 completed on 2026-04-28 through GSD plan execution and verification on `main`.
- Phase 3 completed on 2026-04-28 through GSD plan execution and verification on `main`, closing deterministic discovery, core facts/runner, and the Phase 3 diagnostic contract without claiming Go/TS semantic extraction, cache/performance, production SARIF, or broad CLI hardening.
- Phase 4 completed on 2026-04-29 through GSD plan execution, code review fixes, and verification on `main`, closing parser-backed Go facts and Go CLI integration coverage without claiming full Go type checking or production debug-command hardening.
- Phase 5 completed on 2026-04-30 through GSD plan execution, code review fixes, and verification on `main`, closing parser-backed TS/JS facts and TS CLI integration coverage without claiming TypeScript semantic type checking, production module resolution, or final debug-command hardening.
- Phase 6 completed on 2026-05-01 through GSD plan execution, code review fixes, verification, and security on `main`, closing the public SDK authoring surface, all eight requested example rules, CLI fixture proof, and representative rule-family snapshots without claiming cache/performance, production SARIF/CI hardening, experimental debug-command expansion, or automatic repo-local Rust rule loading.
- Phase 7 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing the disableable hash cache, cached parser/fact metadata, deterministic Rayon-backed execution, repeated-run output proof, and internal timing metadata without claiming benchmark-grade speedups.
- Phase 8 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing SARIF-like output, final exit semantics, and CI-facing behavior without claiming certified SARIF. Later cleanup removed experimental debug commands from the public CLI surface.
- Phase 9 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing README, examples, mixed/example CLI smoke tests, final release verification, and v1 requirement traceability without claiming crates.io publishing, release tags, exact Go semantics, dynamic branch coverage, or fully automatic repo-local Rust rule compilation beyond the documented Cargo integration.
- v1.0 MVP was audited, archived, tagged, and closed on 2026-05-02.
- Quick task 260502-ehi removed all product built-in policy rules from the CLI while keeping example policies as external rule code.
- Quick task 260502-qsd made every example self-contained, with one local Rust rule crate under `examples/<name>/.polint/rules/` and no shared example rule pack.
- Phase 12 completed on 2026-05-11 through GSD plan execution, code review fixes, full workspace regression, clippy, and verification on `main`, closing resolved imports and module relationship facts for Go and TS/JS without claiming symbols, call graph, CFG, dataflow, type checking, or project-level graph caching.
- v1.2 Static Analysis Engine Implementation requirements are defined in `.planning/REQUIREMENTS.md`.
- v1.2 Static Analysis Engine Implementation roadmap is defined in `.planning/ROADMAP.md`.
- Static-analysis engine research and implementation sequencing live in `research/ROADMAP.md`.
- Phase 20 completed on 2026-05-16 through GSD plan execution, code review, full workspace regression, and verification, closing the private analysis kernel facade and provider manifest foundation without promoting a public SDK or CLI provider surface.
- Phase 21 completed on 2026-05-17 through GSD plan execution, code review, full workspace regression, and verification, closing provenance/precision/validation sidecar metadata and deterministic merge validation for current fact families without promoting a public metadata surface.
- Phase 22 completed on 2026-05-17 through GSD plan execution, code review, post-review determinism fix, full workspace regression, and verification, closing the internal evaluation harness MVP without promoting a public eval CLI, SDK, runner, schema, or check JSON surface.

## Constraints

- **Stack**: Rust workspace with Rust 2024 edition - required by the prompt and fits performance/static-analysis needs.
- **Language support**: multi-language framework. Adapters today: Go (tree-sitter) and TypeScript/JavaScript (Oxc). New languages plug in through the adapter contract.
- **Parser choices**: tree-sitter-go for Go and Oxc for TS/JS - requested baseline and current crate ecosystem fit.
- **Performance**: Use deterministic parallelism and avoid cloning large source strings - large repo support is a core requirement.
- **Reliability**: Parser errors and rule panics should become diagnostics or controlled internal errors, not crashes.
- **Truthfulness**: Heuristic rules must say they are heuristic and must not claim exact coverage.
- **Repository layout**: Product code and GSD planning documents live together in the repository root on `main`.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use the `polint` binary/crate prefix and **`emilwareus/polint`** as the GitHub repository | The product is polint end-to-end; the Rust crates stay `polint-*`. | Accepted (repo rename) |
| Build a smaller complete v1 instead of shallow full breadth | The prompt explicitly prefers working, tested functionality over fake or broad shallow features. | Accepted in Phase 9 |
| Start with a hash-based cache, not Salsa | The prompt allows Salsa to remain behind an abstraction if it slows delivery. A content/config/rule hash cache is simpler to ship safely. | Accepted in Phase 7 |
| Treat repo-local rule auto-compilation as future work | Full auto-compilation is not required for v1; scaffolding and explicit Cargo integration are enough. | Accepted in Phase 6 |
| Use in-repo GSD planning on `main` | GSD runs in this checkout on `main`; avoid worktrees. | Accepted in Phase 1 |
| Close Phase 2 around the first usable CLI loop without overclaiming later commands | `init`, `new-rule`, `check`, config loading, discovery, and JSON output are verified, while experimental debug command ideas remain unpromoted. | Accepted in Phase 2 |
| Treat Phase 3 stable IDs as deterministic within a run | File discovery now sorts root-relative paths before `AnalysisDb::add_file`, and cross-run externally visible identity remains fingerprint-based where needed. | Accepted in Phase 3 |
| Snapshot the JSON diagnostic renderer output directly | Workspace-wide `serde_json/preserve_order` can change `Value` object reserialization order; parseability is still verified separately while snapshots pin CLI-facing renderer output. | Accepted in Phase 3 |
| Keep Go analysis syntax-first and explicit about heuristics | Phase 4 uses tree-sitter facts and conservative error-path heuristics, while full Go type checking and exact coverage remain out of scope for the first pass. | Accepted in Phase 4 |
| Keep TypeScript analysis syntax-first and explicit about heuristics | Phase 5 uses Oxc syntax facts for TS/JS parsing, declarations, JSX, literals, calls, complexity, and import relationship proof, while TypeScript type checking and production module resolution remain out of scope. | Accepted in Phase 5 |
| Keep the SDK additive and rule-author focused | Phase 6 exposes borrowed `RuleCtx` helpers and SDK prelude exports without replacing the core rule contract or adding a query engine. | Accepted in Phase 6 |
| Keep policy rules out of the shipped CLI and examples self-contained | The product is a framework, not a bundled policy pack; each example owns one local rule under `examples/<name>/.polint/rules/` and runs it through a native rule host instead of relying on built-in CLI policies. | Accepted in quick tasks 260502-ehi and 260502-qsd |
| Keep heuristic rule claims explicit and bounded | Phase 6 Go branch/test heuristics include `heuristic` wording and evidence labels, avoiding claims of exact semantic coverage. | Accepted in Phase 6 |
| Keep scaffolded repo-local Rust rules honest and safe | Phase 6 hardens `polint new-rule` against unsafe names and overwrite, while leaving automatic dynamic loading to later phases. | Accepted in Phase 6 |
| Cache parser facts, not full source text | Phase 7 stores diagnostics and extracted fact metadata keyed by content/config/rule/schema inputs, while avoiding cached full source or AST payloads. | Accepted in Phase 7 |
| Use deterministic merge boundaries around Rayon work | Phase 7 parallelizes file reads, adapter parsing, and rule execution where safe, then sorts or restores through deterministic boundaries before emitting output. | Accepted in Phase 7 |
| Treat timing output as local metadata, not benchmarks | Phase 7 timing checks assert only shape/order/nonnegative values and no fixed speedup claims. | Accepted in Phase 7 |
| Keep CI output SARIF-like, not certified SARIF | Phase 8 emits useful SARIF-shaped JSON for CI while avoiding conformance claims beyond the implemented fields. | Accepted in Phase 8 |
| Make README and examples the v1 user-facing documentation surface | Phase 9 completed concise command-oriented docs and examples instead of creating a separate docs site or publishing automation. | Accepted in Phase 9 |
| Fulfill capability promises instead of removing them | The v1.1 milestone should make declared capabilities operational through public facts, setup validation, cache semantics, docs, and external-consumer tests. | Accepted in Phase 11 for planning/cache/diagnostic semantics; remaining fact families continue in later v1.1 phases |
| Keep Go and TS/JS as full-coverage targets before Python and Java parity | The current adapters are the proving ground for the complete capability model; Python and Java should enter through explicit subsets and expand after the model is proven. | Accepted for v1.2 sequencing |
| Own the public fact model even when adapters use language-native tooling | Rule authors should consume normalized polint facts while adapters may use Oxc, `go/packages`, Python tooling, javac, JavaParser, coverage.py, LCOV, or JaCoCo behind the boundary. | Accepted for v1.2 sequencing |
| Build the static-analysis substrate before promoting broad public graph/query APIs | The research roadmap shows kernel, provenance, evaluation, cache, MIR, CFG, summaries, extensions, and benchmark gates must precede stable public advanced query surfaces. | Accepted for v1.2 |

## Next Milestone Goals

v1.2 Static Analysis Engine Implementation is active. See `.planning/REQUIREMENTS.md` and `.planning/ROADMAP.md` for scoped requirements and phase mapping. `research/ROADMAP.md` remains the source of truth for the implementation order.

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? -> Move to Out of Scope with reason
2. Requirements validated? -> Move to Validated with phase reference
3. New requirements emerged? -> Add to Active
4. Decisions to log? -> Add to Key Decisions
5. "What This Is" still accurate? -> Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check - still the right priority?
3. Audit Out of Scope - reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-17 after completing Phase 22 Internal Evaluation Harness MVP*
