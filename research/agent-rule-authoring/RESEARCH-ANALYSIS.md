# Research Analysis

## The Design Space

Static-analysis rule authoring systems fall into five families.

| Family | Examples | Strength | Weakness |
|---|---|---|---|
| Declarative query language | CodeQL, Datalog/Doop/Souffle | Powerful reusable logic, relational joins, path queries. | New language/runtime, steep learning curve, hard public compatibility. |
| Pattern DSL/YAML | Semgrep | Fast first rule, code-shaped matching, easy tests. | Limited for max-capability semantic analysis unless many features are added. |
| Visitor/lint API | ESLint, Clippy, go/analysis, Error Prone | Familiar plugin style, metadata, diagnostics, fixes. | Can expose parser/compiler internals and become language-specific. |
| Fluent graph API | Joern CPGQL, ArchUnit-style APIs | Composable relationship queries, good exploration. | Raw graph schemas leak internals and can produce unbounded queries. |
| Typed fact views | Current polint direction | Rust-native, capability-derived, stable fact boundaries, good for agents. | Needs strong scaffolding, tests, manifests, and query helpers to avoid friction. |

Polint should use typed fact views as the primary surface, then add small
domain-specific query builders on top. This gives enough structure for agents
to generate code and enough control for the engine to validate capabilities,
precision, provenance, and cache inputs.

## Why Not A QL First

CodeQL is a mature and powerful system, but a QL-like language would force
polint to build:

- a compiler/interpreter;
- language libraries;
- query optimizer;
- library/package system;
- path-query UI;
- model pack semantics;
- testing framework;
- compatibility story for the query language.

Those investments are not wrong forever, but they are the wrong first public
surface. Polint already has a Rust rule-pack model and macro-derived fact views.
Agents can generate Rust, run Cargo, and repair compiler errors. A typed Rust
surface also fits repo-local max-capability extensions.

Copy from CodeQL:

- metadata discipline;
- path-problem diagnostics;
- thin rule over thick library;
- source/sink/sanitizer/barrier configuration;
- model packs;
- exact tests.

Avoid from CodeQL first:

- a full query language;
- exposing query-library internals as the public API;
- dense tuple-based model rows.

## Why Not Semgrep YAML First

Semgrep has excellent authoring velocity. Its patterns are concise, tests are
simple, and taint vocabulary is familiar.

But polint's goal is not to be a portable pattern matcher. It is to be a
Rust-native analysis framework with repo-local rules and extensions. YAML can
only express shapes the engine already designed. The user explicitly wants max
capability and Rust-code extensibility.

Copy from Semgrep:

- fast scaffolding;
- code-shaped examples;
- inline `expect`/`ok` tests;
- source/sink/sanitizer/propagator vocabulary;
- focus diagnostic span;
- deterministic fixes with expected output.

Avoid:

- making YAML the primary power surface;
- implicit taint exactness defaults;
- hiding interprocedural capability boundaries.

## Why Not Raw Graph DSL First

Joern shows graph queries are powerful for security research. However, raw graph
APIs make ordinary policies harder and freeze internal schemas too early.

Better:

```rust
flow.sources(user_input()).to(shell_exec.argument(0)).paths().limit(5)
```

Worse as the first public API:

```text
start from CPG node, traverse AST edge, CFG edge, PDG edge, filter labels...
```

Polint can still store internal graphs and later expose a debug/experimental
graph query surface. Normal rules should consume typed domain views.

## Query Ergonomics Principles

### 1. Bind First, Filter Later

Semgrep metavariables and Joern traversals show a useful pattern:

```text
find candidates
bind semantic pieces
filter with predicates
report a focused span
```

Polint should support that through typed iterators/builders:

```rust
for call in calls.named("exec.Command") {
    if flow.is_reachable(source, call.argument(0)) {
        report(call.argument(0).span())
    }
}
```

### 2. Match Span Is Not Diagnostic Span

A query may match a function, a chain, or a module path, but the diagnostic
should point at the exact import, call argument, literal, field, or sink.

Diagnostic APIs should make focused reporting easy.

### 3. Capabilities Must Carry Precision

It is not enough to say a rule requires `call_graph`. It matters whether the
view is:

- syntactic;
- direct-only;
- type-aware;
- summary-based;
- interprocedural;
- heuristic;
- extension-augmented;
- unsupported/setup-missing.

The manifest and fact docs should expose this.

### 4. Expensive Queries Must Be Bounded

Path and graph queries should require explicit limits:

```rust
paths().limit(5)
call_paths().max_depth(4)
reachable().budget(QueryBudget::default())
```

Unbounded traversal should be explicit and usually preview/internal.

### 5. Evidence Is Part Of The Result

Graph-backed rules should produce structured evidence:

```text
source
sink
path steps
related locations
precision/status
unknowns
model/provider provenance
```

This aligns with the slicing/evidence research and is critical for agent
debugging.

## Rules, Models, Providers, Summaries

The most important research result is that these are different artifacts.

### Rules

Rules answer:

```text
Should this repository report a diagnostic here?
```

They should be small policy functions over facts.

### Models

Models answer:

```text
What does this API/framework/package mean to the analyzer?
```

Examples:

- `Request.FormValue` returns user input;
- `exec.Command` argument 0 is a shell/process sink;
- `sanitizeHtml` return value is sanitized for HTML;
- `router.get(path, handler)` creates an HTTP entrypoint;
- argument 0 flows to return.

### Provider Extensions

Providers answer:

```text
What new validated facts can code recover from this repository?
```

Examples:

- custom router recovery;
- generated client mapping;
- build/lifecycle integration;
- custom summary inference;
- framework dispatch edge discovery.

### Summaries

Summaries answer:

```text
What reusable function behavior should callers consume?
```

They are the boundary between local and interprocedural analysis.

## Agent Authoring Requirements

Agents are good at reading code and writing code, but they need executable
feedback. Prompt-only authoring fails because the agent cannot see:

- what facts exist;
- what facts are unknown;
- which capability failed;
- whether a model matched anything;
- whether a provider changed diagnostics;
- whether precision improved or worsened;
- whether runtime/cache behavior regressed.

Therefore add inspect/explain/diff/eval commands before public provider
extensions.

## Testing And Packaging Analysis

### Tests

Use a hybrid:

- Semgrep/go-analysis inline markers for quick line-level assertions;
- Ruff/Clippy snapshots for full diagnostic output;
- OpenRewrite before/after fixtures for future fixes;
- CodeQL-style separate test packs for shareable rule/model packs;
- provider fact snapshots for extension outputs;
- default-vs-extended model/provider deltas.

### Packaging

Keep one `.polint/rules` crate per repo by default. Do not generate one Cargo
package per rule. Compile time matters for agent iteration.

Later shareable packaging can split:

- rule pack;
- model pack;
- provider extension pack;
- test pack.

But local repo authoring should remain simple.

## Accuracy Risks

| Risk | Mitigation |
|---|---|
| Rust rules are too hard compared with YAML. | Generate compileable skeletons, fixtures, manifests, and clear macro errors. |
| Derived capability hides precision. | Manifest includes precision level and setup requirements. |
| RuleCtx becomes broad and unstable. | Keep facts in typed views; no raw `AnalysisDb` public access. |
| Models silently create false positives/negatives. | Strict validation, provenance, applicability, and delta tests. |
| Provider extensions corrupt facts. | Process isolation, typed sinks, validation, deterministic fact snapshots. |
| Raw graph API freezes internals. | Expose typed relationship views first; keep graph debug/internal. |
| Fixes change behavior unexpectedly. | Structured edits, applicability, dry-run, before/after snapshots. |
| Agents generate over-broad rules. | Require positive/negative fixtures and default-vs-extended evaluation. |

## Final Technical Recommendation

Add the rule-authoring substrate in this order:

1. rule manifests;
2. inspect/explain/facts/unknowns commands;
3. fixture runner;
4. better `new-rule` scaffolding;
5. message IDs and diagnostic builders;
6. query builders over existing views;
7. preview-gated future views for calls, data flow, effects, evidence;
8. model packs;
9. provider extension tests;
10. structured fixes.

This keeps polint immediately useful for current repo-local policies while
preparing the API for the advanced analysis engine researched in previous
tracks.
