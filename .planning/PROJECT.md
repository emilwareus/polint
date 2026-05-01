# exlint

## What This Is

exlint is a high-performance Rust framework for writing repo-local static-analysis rules. It initially supports Go and TypeScript/JavaScript and gives rule authors reusable infrastructure for file discovery, parsing, facts, graphs, diagnostics, rule testing, CI output, and eventually sandboxed Wasm plugins.

The product is for engineering teams using AI-assisted development who need executable project-specific policies instead of repeating local conventions in prompts. It is not a replacement for ESLint, Ruff, Biome, golangci-lint, or formatters; it is a framework for checks that those generic tools cannot know.

## Core Value

Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

## Requirements

### Validated

- [x] Create a compiling Rust 2024 workspace with clear crate boundaries for CLI, config, diagnostics, filesystem, cache, core analysis, SDK, Go, TS, graph, rules, and plugin support. Validated in Phase 1: Workspace Foundation.
- [x] Provide the first CLI loop for `polint init`, `polint new-rule`, and `polint check` with profiles, output formats, cache disabling, and fail thresholds. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Load `.polint.toml` with include/exclude globs, profiles, rule paths, severity overrides, language settings, and sane defaults when config is missing. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Discover Go, TS, TSX, JS, and JSX files with `.gitignore`, include glob, and exclude glob support. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Render `polint check` diagnostics as parseable JSON. Validated in Phase 2: CLI, Config, and Discovery.
- [x] Make file discovery output deterministic by sorting normalized root-relative paths after `.gitignore`, include/exclude, and language filtering, with deterministic `AnalysisDb` file ID insertion. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Define stable v1 core fact models and deterministic in-run IDs for files, spans, functions, imports, branch obligations, tests, coverage placeholders, and analysis database accessors. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Run rules through the core registry with capability declarations, severity/options, deterministic sequential/parallel output, deduplication, and panic/error containment. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Provide the Phase 3 diagnostic contract: severities, labels, evidence, suggestions/fixes, help text, stable fingerprints, deterministic sort/dedupe, and human/JSON rendering coverage. Validated in Phase 3: Core Facts and Diagnostics.
- [x] Implement Go analysis with tree-sitter-go for packages, imports, functions, methods, tests, branch obligations, import graph facts, and cyclomatic complexity foundations. Validated in Phase 4: Go Adapter.
- [x] Implement TypeScript/JavaScript analysis with Oxc for parser diagnostics, imports/exports, functions, classes, JSX attributes, string literals, component heuristics, cyclomatic complexity, and import graph facts. Validated in Phase 5: TypeScript Adapter.
- [x] Provide a public `polint-sdk` with a clean `Rule` trait, capability declarations, high-level `RuleCtx` queries, and helpers for reporting diagnostics. Validated in Phase 6: SDK and Example Rules.
- [x] Ship example rules that dogfood the same SDK users will use, including Go complexity, TS complexity, Go import boundaries, TS raw color detection, Go branch obligations, Go test suite size, Go assertion-after-action, and configured denied literals. Validated in Phase 6: SDK and Example Rules.
- [x] Support repo-local Rust rule scaffolding through `polint new-rule` and document the native registration boundary without claiming dynamic loading. Validated in Phase 6: SDK and Example Rules.
- [x] Add a hash-based cache that can be disabled, deterministic parallel file/parser/rule execution, and per-rule profiling output. Validated in Phase 7: Cache and Performance.
- [x] Finish the remaining CLI surface for custom rules: `polint test-rules`, `polint profile-rules`, `polint explain`, graph export commands, and final exit-code semantics. Validated in Phase 8: CI Output and Graph Commands.
- [x] Keep file discovery and execution scalable enough for v1 through deterministic parallel reads/parsing/rule execution and cache support. Validated in Phase 7: Cache and Performance.
- [x] Harden SARIF-like diagnostics, fail thresholds, exit code semantics, and CI-facing command behavior. Validated in Phase 8: CI Output and Graph Commands.
- [x] Add a Wasm plugin skeleton with WIT files and Wasmtime host boundaries, clearly marked experimental. Validated in Phase 9: Plugin Skeleton.
- [x] Provide meaningful unit, integration, snapshot, and property tests for the core behavior. Validated across Phases 1-10, with final traceability closed in Phase 10: Docs, Examples, and Release Hardening.
- [x] Write a README that explains the goal, non-goals, quickstart, custom rule authoring, CI usage, and roadmap. Validated in Phase 10: Docs, Examples, and Release Hardening.

### Active

No active v1 requirements remain after Phase 10 verification. Future work is tracked under v2 requirements and out-of-scope notes.

### Out of Scope

- Becoming a comprehensive built-in lint ruleset - the product value is custom rule infrastructure.
- Replacing existing language linters or formatters - users should keep ESLint, Biome, golangci-lint, rustfmt, and similar tools.
- Full Go type checking in the first pass - leave a trait boundary for a future `go/packages` or `go/analysis` sidecar.
- Full dynamic branch coverage in the first pass - design the model so exact coverage can be added later.
- Fully automatic compilation/loading of repo-local Rust rules in the first pass - scaffolding, SDK, native registration, and Wasm skeleton are sufficient for v1.
- Passing huge AST JSON blobs to plugins - plugin APIs should use stable IDs and host queries.

## Context

- The implementation target is Rust 2024 on stable Rust. Current local toolchain check: `rustc 1.94.0` and `cargo 1.94.0`.
- Current crate checks with `cargo search` on 2026-04-28 found compatible latest versions for the requested baseline: `clap 4.6.1`, `serde 1.0.228`, `serde_json 1.0.149`, `toml 1.1.2+spec-1.1.0`, `anyhow 1.0.102`, `thiserror 2.0.18`, `rayon 1.12.0`, `ignore 0.4.25`, `globset 0.4.18`, `petgraph 0.8.3`, `tree-sitter 0.26.8`, `tree-sitter-go 0.25.0`, Oxc `0.128.0`, `oxc_resolver 11.19.1`, `wasmtime 44.0.0`, `wit-bindgen 0.57.1`, `insta 1.47.2`, `assert_cmd 2.2.1`, `predicates 3.1.4`, `tempfile 3.27.0`, and `proptest 1.11.0`.
- The initial project prompt lives at `docs/INITIAL_PROMPT.md`.
- The source repository and GSD planning both live at `/Users/emilwareus/Development/exlint` on branch `main`.
- The suggested project name in the prompt is `polint`, so the binary and crate names use `polint-*` while the repository remains `exlint`.
- Phase 1 completed on 2026-04-28 through GSD plan execution and verification on `main`.
- Phase 2 completed on 2026-04-28 through GSD plan execution and verification on `main`.
- Phase 3 completed on 2026-04-28 through GSD plan execution and verification on `main`, closing deterministic discovery, core facts/runner, and the Phase 3 diagnostic contract without claiming Go/TS semantic extraction, cache/performance, production SARIF, or broad CLI hardening.
- Phase 4 completed on 2026-04-29 through GSD plan execution, code review fixes, and verification on `main`, closing parser-backed Go facts and Go CLI integration coverage without claiming full Go type checking or production graph command hardening.
- Phase 5 completed on 2026-04-30 through GSD plan execution, code review fixes, and verification on `main`, closing parser-backed TS/JS facts and TS CLI integration coverage without claiming TypeScript semantic type checking, production module resolution, or final graph command hardening.
- Phase 6 completed on 2026-05-01 through GSD plan execution, code review fixes, verification, and security on `main`, closing the public SDK authoring surface, all eight requested example rules, CLI fixture proof, and representative rule-family snapshots without claiming cache/performance, production SARIF/CI hardening, graph command expansion, plugin loading, or automatic repo-local Rust rule loading.
- Phase 7 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing the disableable hash cache, cached parser/fact metadata, deterministic Rayon-backed execution, repeated-run output proof, and `profile-rules` timing rows without claiming benchmark-grade speedups.
- Phase 8 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing SARIF-like output, final exit semantics, explain/test/profile commands, deterministic DOT graph commands, and CI-facing behavior without claiming certified SARIF.
- Phase 9 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing the experimental WIT plugin boundary, structured manifest validation, optional Wasmtime component-byte validation, and honest plugin docs without claiming `polint check` plugin execution.
- Phase 10 completed on 2026-05-01 through GSD plan execution, code review, verification, and security on `main`, closing README, examples, mixed/example CLI smoke tests, final release verification, and v1 requirement traceability without claiming crates.io publishing, release tags, exact Go semantics, dynamic branch coverage, or automatic repo-local Wasm compilation.

## Constraints

- **Stack**: Rust workspace with Rust 2024 edition - required by the prompt and fits performance/static-analysis needs.
- **Language support**: Go and TypeScript/JavaScript first - more languages should be addable through adapters.
- **Parser choices**: tree-sitter-go for Go and Oxc for TS/JS - requested baseline and current crate ecosystem fit.
- **Performance**: Use deterministic parallelism and avoid cloning large source strings - large repo support is a core requirement.
- **Reliability**: Parser errors and rule panics should become diagnostics or controlled internal errors, not crashes.
- **Truthfulness**: Heuristic rules must say they are heuristic and must not claim exact coverage.
- **Repository layout**: Product code and GSD planning documents live together in the repository root on `main`.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use `polint` as the binary/crate prefix inside the `exlint` repository | The prompt explicitly suggests `polint`; keeping the repo as `exlint` preserves the GitHub repository name already created. | Accepted in Phase 1 |
| Build a smaller complete v1 instead of shallow full breadth | The prompt explicitly prefers working, tested functionality over fake or broad shallow features. | Accepted in Phase 10 |
| Start with a hash-based cache, not Salsa | The prompt allows Salsa to remain behind an abstraction if it slows delivery. A content/config/rule hash cache is simpler to ship safely. | Accepted in Phase 7 |
| Treat repo-local rule auto-compilation as future/experimental | The prompt requires SDK and scaffolding first, with Wasm skeleton acceptable for the first implementation. | Accepted in Phase 9 |
| Use in-repo GSD planning on `main` | The user wants to use GSD directly in `/Users/emilwareus/Development/exlint` and avoid worktrees. | Accepted in Phase 1 |
| Close Phase 2 around the first usable CLI loop without overclaiming later commands | `init`, `new-rule`, `check`, config loading, discovery, and JSON output are verified, while explain/test/profile/graph command hardening remains scheduled later. | Accepted in Phase 2 |
| Treat Phase 3 stable IDs as deterministic within a run | File discovery now sorts root-relative paths before `AnalysisDb::add_file`, and cross-run externally visible identity remains fingerprint-based where needed. | Accepted in Phase 3 |
| Snapshot the JSON diagnostic renderer output directly | Workspace-wide `serde_json/preserve_order` can change `Value` object reserialization order; parseability is still verified separately while snapshots pin CLI-facing renderer output. | Accepted in Phase 3 |
| Keep Go analysis syntax-first and explicit about heuristics | Phase 4 uses tree-sitter facts and conservative error-path heuristics, while full Go type checking and exact coverage remain out of scope for the first pass. | Accepted in Phase 4 |
| Keep TypeScript analysis syntax-first and explicit about heuristics | Phase 5 uses Oxc syntax facts for TS/JS parsing, declarations, JSX, literals, calls, complexity, and import graph proof, while TypeScript type checking and production module resolution remain out of scope. | Accepted in Phase 5 |
| Keep the SDK additive and rule-author focused | Phase 6 exposes borrowed `RuleCtx` helpers and SDK prelude exports without replacing the core rule contract or adding a query engine. | Accepted in Phase 6 |
| Treat built-in rules as SDK dogfood examples | Phase 6 keeps the requested `examples/...` rules as registered examples with deterministic diagnostics, configuration, and tests rather than a broad lint pack. | Accepted in Phase 6 |
| Keep heuristic rule claims explicit and bounded | Phase 6 Go branch/test heuristics include `heuristic` wording and evidence labels, avoiding claims of exact semantic coverage. | Accepted in Phase 6 |
| Keep scaffolded repo-local Rust rules honest and safe | Phase 6 hardens `polint new-rule` against unsafe names and overwrite, while leaving automatic dynamic loading to later phases. | Accepted in Phase 6 |
| Cache parser facts, not full source text | Phase 7 stores diagnostics and extracted fact metadata keyed by content/config/rule/schema inputs, while avoiding cached full source or AST payloads. | Accepted in Phase 7 |
| Use deterministic merge boundaries around Rayon work | Phase 7 parallelizes file reads, adapter parsing, and rule execution where safe, then sorts or restores through deterministic boundaries before emitting output. | Accepted in Phase 7 |
| Treat timing output as local profiling metadata, not benchmarks | Phase 7 `profile-rules` reports parseable elapsed timing rows but tests only assert shape/order/nonnegative values and no fixed speedup claims. | Accepted in Phase 7 |
| Keep CI output SARIF-like, not certified SARIF | Phase 8 emits useful SARIF-shaped JSON for CI while avoiding conformance claims beyond the implemented fields. | Accepted in Phase 8 |
| Keep plugin support validate-only in v1 | Phase 9 validates manifests and optional component bytes but does not execute plugin code from `polint check`. | Accepted in Phase 9 |
| Make README and examples the v1 user-facing documentation surface | Phase 10 completed concise command-oriented docs and examples instead of creating a separate docs site or publishing automation. | Accepted in Phase 10 |

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
*Last updated: 2026-05-01 after Phase 10 verification*
