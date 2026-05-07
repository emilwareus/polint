<!-- GSD:project-start source:PROJECT.md -->
## Project

**polint**

polint is a high-performance Rust framework for writing repo-local static-analysis rules across multiple languages. Adapters today cover **Go** (tree-sitter) and **TypeScript / JavaScript** (Oxc); more languages can be added through the same adapter contract. polint gives rule authors reusable infrastructure for file discovery, parsing, facts, graphs, diagnostics, rule testing, and CI output.

The product is for engineering teams using AI-assisted development who need executable project-specific policies instead of repeating local conventions in prompts. It is not a replacement for ESLint, Ruff, Biome, golangci-lint, or formatters; it is a framework for checks that those generic tools cannot know.

**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

### Constraints

- **Stack**: Rust workspace with Rust 2024 edition - required by the prompt and fits performance/static-analysis needs.
- **Language support**: multi-language framework. Today: Go (tree-sitter) and TypeScript/JavaScript (Oxc). New languages added through the adapter contract.
- **Parser choices**: tree-sitter-go for Go and Oxc for TS/JS - requested baseline and current crate ecosystem fit.
- **Performance**: Use deterministic parallelism and avoid cloning large source strings - large repo support is a core requirement.
- **Reliability**: Parser errors and rule panics should become diagnostics or controlled internal errors, not crashes.
- **Truthfulness**: Heuristic rules must say they are heuristic and must not claim exact coverage.
- **Repository layout**: Product code and GSD planning documents live together in the repository root on `main`.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack
| Area | Choice | Version Checked | Rationale | Confidence |
|------|--------|-----------------|-----------|------------|
| CLI | `clap` with derive | 4.6.1 | Mature Rust CLI parser with strong help text support. | High |
| Serialization | `serde`, `serde_json`, `toml` | serde 1.0.228, serde_json 1.0.149, toml 1.1.2+spec-1.1.0 | Required for config, diagnostics, cache metadata, JSON output, and SARIF-like output. | High |
| Errors | `anyhow`, `thiserror` | anyhow 1.0.102, thiserror 2.0.18 | Application errors in CLI, typed errors in libraries. | High |
| Tracing | `tracing`, `tracing-subscriber` | tracing 0.1.44, subscriber 0.3.23 | Structured internal logging and future profiling integration. | High |
| Parallelism | `rayon` | 1.12.0 | Straightforward parallel parsing and rule execution. | High |
| File discovery | `ignore`, `globset` | ignore 0.4.25, globset 0.4.18 | Fast walking with `.gitignore` support and reliable glob matching. | High |
| Graphs | `petgraph` | 0.8.3 | Suitable for import graphs, call graph skeletons, CFG representation, and DOT export. | High |
| Go parsing | `tree-sitter`, `tree-sitter-go` | tree-sitter 0.26.8, tree-sitter-go 0.25.0 | Practical syntax extraction without needing Go type checking in v1. | High |
| TS/JS parsing | Oxc crates | 0.129.0 | Rust-native high-performance JS/TS parser ecosystem. | Medium |
| Import resolution | `oxc_resolver` | 11.19.1 | Useful for future TS import graph precision. Initial v1 can start with syntactic imports. | Medium |
| Tests | `insta`, `assert_cmd`, `predicates`, `tempfile`, `pretty_assertions`, `proptest` | Current versions checked | Covers snapshots, CLI integration, fixtures, diffs, and invariants. | High |
## What Not To Use First
- Salsa as a hard dependency for v1 query infrastructure. Keep a cache abstraction and ship the hash-based cache first.
- Full Go semantic analysis in the first pass. Avoid depending on a sidecar until syntax/fact extraction is stable.
- A custom JS/TS parser. Oxc is the right default.
- Leaking full AST dumps through public rule APIs. Prefer stable IDs and incremental facts on the host.
## Version Notes
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

### Public API and visibility

Everything **public** is a **liability**: semver, documentation, stability expectations, and review surface all attach to names users and tools can import. Default to the **narrowest** visibility that still works (`private` → `pub(super)` / `pub(in path)` → `pub(crate)` → `pub` only when crossing an intentional boundary).

- **Supported rule-author surface:** `polint::sdk` (including `prelude` and `scope`) and `polint::runner`, plus what those modules deliberately document and re-export. Treat `core`, `cache`, `config`, `fs`, `go`, `ts`, `graph`, `cli`, and other crate-root modules as **implementation detail** unless a change explicitly promotes something to the SDK.
- **Inside `crates/polint`:** use **`pub(crate)`** for anything shared across internal modules but not meant for downstream crates. Use bare **`pub`** only when a name must be visible outside its defining module *and* that visibility is intentional (e.g. items in `sdk` / `runner`, or the unstable **`polint::_bench`** tree behind **`feature = "bench"`** for `polint-bench` only).
- **`pub use` re-exports:** treat each one as widening the API; prefer small, curated surfaces over large barrel re-exports.
- **Linting:** the workspace enables **`unreachable_pub`**. If it fires, **fix visibility** (usually `pub` → `pub(crate)` or tightening module `pub`) rather than weakening the lint, unless there is a documented false positive.

Bench-only and internal hooks should stay namespaced (**`_bench`**, **`#[doc(hidden)]`**) and must not be presented as a supported extension API for rule packs.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase. For **crate visibility and public API discipline**, follow the Conventions above and [`docs/API-VISIBILITY-PLAN.md`](docs/API-VISIBILITY-PLAN.md) when tightening `pub` / `pub(crate)` boundaries.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

## Rule Authoring Platform Contract

Repo-local rules must be treated as external consumers of polint, even when they
live in `examples/` inside this repository.

- Rule code should import `polint::sdk::prelude::*` and register through
  `polint::runner::run_cli`; do not make examples depend on `polint::core`,
  `go`, `ts`, `config`, parser adapters, test helpers, or other internal modules.
- Examples should demonstrate composition of public facts from `RuleCtx`, not
  call one-off helpers that solve only the example.
- When adding a rule-authoring feature, add at least one temp-repo style test
  that behaves like an outside user: generated `.polint/rules`, public SDK
  imports only, real facts consumed, and a diagnostic asserted through
  `polint check --format json`.
- Keep capability names honest. Do not expose or advertise a capability as a
  provided fact family until a rule can read the underlying facts through the
  public SDK.
- If a rule needs custom config, preserve it through `RuleOptions::settings`
  rather than overloading unrelated fields like `allow`, `deny`, or `max`.
- Config and resolved rule options that can affect rule behavior must
  participate in deterministic cache digests, with regression tests for new
  fields.
- Document new public facts under `docs/facts/`, including limits and heuristic
  behavior.



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
