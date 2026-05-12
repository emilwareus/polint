# Analysis Roadmap

polint is a **facts-first**, **multi-language** analyzer: rules read stable extractions (literals, imports, functions, branches, tests, …) through a shared fact model. Today's adapters cover **Go** (tree-sitter) and **TypeScript / JavaScript** (Oxc); the same contract is the entry point for additional languages and for deeper analysis features.

The long-term target is a complete, agent-consumable static-analysis graph of
the codebase: modules, symbols, references, calls, CFGs, test/coverage evidence,
dataflow, taint, and interprocedural summaries. The sequence below favors
truthful, useful slices over broad placeholder analysis.

The table lists **shipped** scenarios (with example repos that prove them) and **planned** work in rough dependency order. Heuristic and future typed rules should state their precision tier in messaging so teams know what they are enforcing.

| Status | Scenario | Notes | Examples |
|---|---|---|---|
| Shipped | TS/JS string & template literals | Span-backed literal facts | [basic](../examples/basic/README.md), [ts-design-tokens](../examples/ts-design-tokens/README.md), [config-denied-literal](../examples/config-denied-literal/README.md), [custom-rule-ts](../examples/custom-rule-ts/README.md) |
| Shipped | JSX / TSX attributes | Name / value facts | [basic](../examples/basic/README.md), [ts-design-tokens](../examples/ts-design-tokens/README.md), [custom-rule-ts](../examples/custom-rule-ts/README.md), [multiple-rules](../examples/multiple-rules/README.md) |
| Shipped | Config-driven deny lists | `[[rules.config]]` → `deny`, globs | [config-denied-literal](../examples/config-denied-literal/README.md) |
| Shipped | Go import paths & boundaries | Import facts + `forbidden_imports` | [go-import-boundaries](../examples/go-import-boundaries/README.md), [multiple-rules](../examples/multiple-rules/README.md) |
| Shipped | Cyclomatic complexity (Go) | Per-function metric | [go-complexity](../examples/go-complexity/README.md) |
| Shipped | Cyclomatic complexity (TS/JS) | Per-function metric | [ts-complexity](../examples/ts-complexity/README.md) |
| Shipped | Go branch / error-path obligations | Heuristic branch facts | [go-branch-obligations](../examples/go-branch-obligations/README.md) |
| Shipped | Go branch policy + test evidence | Branches + test facts (heuristic) | [custom-rule-go](../examples/custom-rule-go/README.md) |
| Shipped | Go test maintainability | Test facts, assertions, thresholds via config | [go-test-quality](../examples/go-test-quality/README.md) |
| Shipped | Several rules in one pack | One `.polint/rules/Cargo.toml`, module per rule | [multiple-rules](../examples/multiple-rules/README.md) |
| Shipped | Minimal TSX starter | Single rule, single diagnostic | [basic](../examples/basic/README.md) |
| Shipped | CLI: checks, JSON/SARIF, cache, comment ignores | Stable user workflows only | `polint --help` |
| Shipped | Comment ignores and ignore statistics | Inline suppression comments plus `polint ignores --stat --shortstat --filter ...` for humans and agents | [comment-ignores](../examples/comment-ignores/README.md), [docs](IGNORE-COMMENTS.md) |
| Planned | Scope-accurate module resolution | Path mapping, package exports, build tags / conditions | — |
| Planned | Symbol / binding resolution | Definitions, references, re-exports; stable symbol IDs | — |
| Planned | Type-aware analysis | TS semantic layer; Go `go/types` (or equivalent); syntax vs typed rule tiers | — |
| Planned | Resolved call graph | Caller → callee symbols; approximate virtual/dynamic dispatch | — |
| Planned | General intra-procedural CFG | First-class per-function graph, not only branch heuristics | — |
| Planned | Dataflow | Def-use / SSA-style IR; value propagation where types exist | — |
| Planned | Interprocedural analysis | Summaries; whole-program or scoped modes; finer-grained invalidation | — |
| Planned | Taint / source–sink tracking | On top of dataflow + configurable sources/sinks | — |
| Planned | Alias / points-to (conservative) | Stronger security-style rules when needed | — |
| Planned | Higher-level rule API | Composable queries, stability guarantees, richer diagnostics provenance | — |
