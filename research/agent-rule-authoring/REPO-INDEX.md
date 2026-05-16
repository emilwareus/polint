# Repository Index

Implementation repositories were cloned under
`research/agent-rule-authoring/repos/`, which is ignored by the repository root
`.gitignore` via `research/*/repos/`.

## Repositories Inspected

| Repository | Commit inspected | Key paths | Main lesson |
|---|---:|---|---|
| CodeQL, <https://github.com/github/codeql> | `a84332ac150e` | `javascript/ql/src/Security`, `javascript/ql/lib/semmle/javascript/security`, `python/ql/lib/semmle/python/dataflow`, `docs` | Thin query files over reusable libraries, `path-problem`, source/sink/barrier configuration, model packs. |
| Semgrep, <https://github.com/semgrep/semgrep> | `db2be62416a2` | `AGENTS.md`, `CHANGELOG.md`, rule/test docs via official docs | Code-shaped matching, metavariable constraints, taint vocabulary, inline tests, fix snapshots, capability boundary lessons. |
| ESLint, <https://github.com/eslint/eslint> | `6616856f28fa` | `lib/rules`, `lib/rule-tester`, `lib/linter`, `docs/src/use/migrate-to-8.0.0.md` | Rule metadata, `context.report`, fixability/suggestions metadata, `RuleTester` diagnostics. |
| Go tools, <https://github.com/golang/tools> | `a3954b5c7496` | `go/analysis`, `go/analysis/analysistest` | Analyzer dependency contracts, result/fact types, suggested fixes, source-comment tests. |
| Joern, <https://github.com/joernio/joern> | `da77724000f5` | `semanticcpg`, `dataflowengineoss`, `console` | Fluent traversals, `reachableByFlows`, custom flow semantics, path evidence, graph API risks. |
| Pyre/Pysa, <https://github.com/facebook/pyre-check> | `34af3721bc04` | `source/interprocedural_analyses`, Pysa docs | Model files/generators, taint config, expected/unexpected models, model exploration. |
| Ruff, <https://github.com/astral-sh/ruff> | `409c13f3ec50` | `crates/ruff_linter`, `crates/ruff_db`, `docs/preview.md`, `docs/versioning.md` | Rust rule implementation discipline, preview/stability gates, fix applicability. |
| OpenRewrite, <https://github.com/openrewrite/rewrite> | `0f600f466394` | `rewrite-test`, `rewrite-java-test`, recipe implementations | Before/after fixtures, recipe metadata, visitor ergonomics, dry-run/diff workflow. |

## Tool-Specific Findings

### CodeQL

CodeQL's useful authoring pattern is a thin query file plus thick reusable
libraries. Production path queries often contain metadata, imports, a flow graph
module, `flowPath(source, sink)`, and a final select. Polint should copy:

- first-class diagnostic kinds, especially path diagnostics;
- reusable flow/query builders;
- metadata discipline;
- model packs as scoped data extensions;
- exact fixture tests.

Polint should not copy:

- a full QL language as the first public authoring surface;
- dense tuple-heavy model rows without typed validation;
- global extra flow steps that affect every rule without visible scope.

### Semgrep

Semgrep wins on the first five minutes. Code-shaped patterns, metavariables,
focus metavariables, taint vocabulary, and inline tests make rule authoring
approachable. Polint should copy:

- simple scaffolding;
- separate match context from diagnostic span;
- bind-then-filter query ergonomics;
- taint/source/sink/sanitizer domain vocabulary;
- inline `expect`/`ok` test markers;
- deterministic fixes with expected fixed output.

Polint should not copy YAML as the primary power surface. Rust typed facts and
repo-local analysis extensions are the differentiator.

### ESLint And typescript-eslint

ESLint's rule metadata is a public contract. ESLint also enforces fix/suggestion
metadata when a rule emits fixes. typescript-eslint shows how typed metadata can
infer options and message IDs.

Polint should generate and expose a manifest with:

- id, name, docs, tags, severity, messages;
- option schema;
- fact capabilities and precision levels;
- fixability and applicability;
- stability;
- SDK version.

### Go `analysis`

Go's analyzer API has the best explicit dependency contract: `Requires`,
`ResultType`, and `FactTypes`. Polint should not make users hand-write those for
normal rules, but the macro should produce an equivalent manifest from typed
parameters. The manifest must be inspectable.

### Joern And CPG Systems

Joern proves fluent graph traversal and path evidence are powerful. It also
shows the danger of exposing the whole graph as the user API. Polint should
provide typed relationship views and bounded path queries:

```rust
flow.sources(user_input()).to(sink.argument(0)).paths().limit(5)
calls.possible_targets(call_site).with_precision_at_least(...)
```

Do not expose raw graph nodes and edge kinds as stable API until the schema is
deliberate.

### Pysa

Pysa's model files and model explorer are directly relevant. Polint should
support declarative model packs with strict validation and explain commands.
Expected/unexpected model tests are especially important for agents.

### Ruff And Clippy

Ruff and Clippy show Rust-native analysis can be very fast and disciplined, but
their extension model is first-party/internal. Polint must keep repo-local
authoring as the product surface. Copy their:

- preview gates;
- fix applicability;
- snapshot/UI tests;
- stable diagnostics.

Do not copy compiler-internal authoring APIs.

### OpenRewrite

OpenRewrite is the best reference for future fixes/rewrites: recipes have
metadata, visitors, options, and before/after tests. Polint should reserve API
space for structured fixes and dry-run diffs even if the first SDK is
diagnostics-only.
