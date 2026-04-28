# Feature Research: exlint

## Table Stakes

- Workspace and crate structure that compiles under Rust 2024.
- CLI commands for init, check, new-rule, test-rules, profile-rules, explain, and graph exports.
- `.polint.toml` config with profiles, include/exclude globs, rule paths, severity overrides, and language settings.
- Fast deterministic file discovery that respects `.gitignore`.
- Core facts: files, spans, functions, imports, branches, tests, coverage placeholders, graphs, and stable IDs.
- Diagnostics rendered as human text and JSON, with stable fingerprints and deterministic sorting.
- Go syntax extraction for packages, imports, functions, methods, tests, branch obligations, and cyclomatic complexity.
- TS/JS syntax extraction for imports, functions, classes, JSX attributes, string literals, and cyclomatic complexity.
- Example rules that prove the SDK and analysis model.
- Tests for config, discovery, extraction, diagnostics, CLI behavior, and snapshot outputs.
- README with quickstart, non-goals, custom rule authoring, and CI guidance.

## Differentiators

- Rule authoring as code inside a repository, not a giant config DSL.
- Capability-gated facts so expensive analysis is computed only when rules ask for it.
- Diagnostics designed for both humans and AI agents.
- Branch obligation facts that can later connect to dynamic coverage.
- Clean migration path from native built-ins to sandboxed Wasm plugins.

## Anti-Features

- A comprehensive bundled ruleset competing with existing linters.
- Misleading certainty from heuristic test-evidence rules.
- Requiring users to understand parser internals for common rules.
- Plugin APIs that expose unstable internal AST details.
- Nondeterministic output caused by parallel execution.

## Dependencies Between Features

- CLI check depends on config, file discovery, diagnostics, core facts, and rule runner.
- Language adapters depend on core IDs, spans, and source file models.
- Example rules depend on SDK ergonomics and adapter facts.
- Caching depends on stable file/config/rule hashes.
- SARIF output depends on stable diagnostics.
- Plugin skeleton depends on stable SDK concepts but not full repo-local compilation.
