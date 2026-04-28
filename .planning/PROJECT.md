# exlint

## What This Is

exlint is a high-performance Rust framework for writing repo-local static-analysis rules. It initially supports Go and TypeScript/JavaScript and gives rule authors reusable infrastructure for file discovery, parsing, facts, graphs, diagnostics, rule testing, CI output, and eventually sandboxed Wasm plugins.

The product is for engineering teams using AI-assisted development who need executable project-specific policies instead of repeating local conventions in prompts. It is not a replacement for ESLint, Ruff, Biome, golangci-lint, or formatters; it is a framework for checks that those generic tools cannot know.

## Core Value

Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

## Requirements

### Validated

(None yet - ship to validate)

### Active

- [ ] Create a compiling Rust 2024 workspace with clear crate boundaries for CLI, config, diagnostics, filesystem, cache, core analysis, SDK, Go, TS, graph, rules, and plugin support.
- [ ] Provide a CLI centered on custom rules: `polint init`, `polint new-rule`, `polint check`, `polint test-rules`, `polint profile-rules`, `polint explain`, and graph export commands.
- [ ] Load `.polint.toml` with include/exclude globs, profiles, rule paths, severity overrides, language settings, and sane defaults when config is missing.
- [ ] Discover files quickly with `.gitignore` support and deterministic ordering.
- [ ] Define stable analysis facts and IDs for files, spans, functions, imports, branches, tests, coverage, graphs, and rule execution.
- [ ] Provide ergonomic diagnostics with human, JSON, and SARIF-like renderers, deterministic sorting, fingerprints, labels, evidence, and suggested fixes.
- [ ] Implement Go analysis with tree-sitter-go: packages, imports, functions, methods, tests, branch obligations, import graph, CFG basics, and cyclomatic complexity.
- [ ] Implement TypeScript/JavaScript analysis with Oxc: imports/exports, functions, classes, JSX attributes, string literals, component heuristics, and cyclomatic complexity.
- [ ] Ship example rules that dogfood the same SDK users will use, including Go complexity, TS complexity, Go import boundaries, TS raw color detection, Go branch obligations, Go test suite size, and Go assertion-after-action.
- [ ] Provide a public `polint-sdk` with a clean `Rule` trait, capability declarations, high-level `RuleCtx` queries, and helpers for reporting diagnostics.
- [ ] Support repo-local Rust rule scaffolding through `polint new-rule` and document the native registration path.
- [ ] Add a hash-based cache that can be disabled, plus deterministic parallel parsing/rule execution and per-rule profiling.
- [ ] Add SARIF-like CI output, fail thresholds, exit code semantics, and a GitHub Actions example.
- [ ] Add a Wasm plugin skeleton with WIT files and Wasmtime host boundaries, clearly marked experimental if not complete.
- [ ] Provide meaningful unit, integration, snapshot, and property tests for the core behavior.
- [ ] Write a README that explains the goal, non-goals, quickstart, custom rule authoring, CI usage, and roadmap.

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
| Use `polint` as the binary/crate prefix inside the `exlint` repository | The prompt explicitly suggests `polint`; keeping the repo as `exlint` preserves the GitHub repository name already created. | - Pending |
| Build a smaller complete v1 instead of shallow full breadth | The prompt explicitly prefers working, tested functionality over fake or broad shallow features. | - Pending |
| Start with a hash-based cache, not Salsa | The prompt allows Salsa to remain behind an abstraction if it slows delivery. A content/config/rule hash cache is simpler to ship safely. | - Pending |
| Treat repo-local rule auto-compilation as future/experimental | The prompt requires SDK and scaffolding first, with Wasm skeleton acceptable for the first implementation. | - Pending |
| Use in-repo GSD planning on `main` | The user wants to use GSD directly in `/Users/emilwareus/Development/exlint` and avoid worktrees. | - Pending |

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
*Last updated: 2026-04-28 after initialization*
