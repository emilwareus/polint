# Decision 001: Typed Rust Rules, Not A Query DSL First

## Status

Recommended.

## Context

Polint needs a public authoring surface for repo-local static-analysis policies
and AI-agent authored artifacts. Candidate approaches:

1. CodeQL-like query language.
2. Semgrep-like YAML/pattern DSL.
3. Joern-like raw graph traversal shell.
4. Visitor API over parser/compiler internals.
5. Typed Rust rules over public fact views, with model packs and provider
   extensions for analysis improvements.

## Decision

Use option 5 as the first public surface.

Rules are:

```rust
#[polint::rule]
fn rule(ctx: &mut RuleCtx<'_>, facts: SomeFacts<'_>) -> RuleResult
```

The macro derives capabilities and emits a manifest. `RuleCtx` stays narrow.
Normal rules consume typed fact views and domain query builders.

Declarative model packs describe API/framework behavior. Provider extensions
are process-isolated Rust code for new facts or semantics. Raw graph/query
surfaces remain internal or future preview features.

## Rationale

Typed Rust rules align with the product:

- repo-local executable policy;
- AI agents can generate, compile, test, and repair Rust;
- capability derivation stays type-based;
- public API can be curated through `polint::sdk::prelude::*`;
- performance and memory behavior remain native;
- advanced users can move from rules to models/providers without leaving Rust.

A new query language would be expensive and freeze many semantics too early.
YAML patterns are fast but not enough for max-capability analysis extensions.
Raw graph traversal would expose unstable internals and make ordinary rules too
hard.

## Consequences

Positive:

- preserves current architectural direction;
- keeps public SDK typed and reviewable;
- allows compile-time checks;
- supports agents writing real code;
- avoids a new language/runtime;
- keeps graph/fixpoint internals private.

Negative:

- higher authoring friction than YAML or QL snippets;
- requires excellent scaffolding and test errors;
- Rust compile times matter;
- macro manifest generation becomes a public contract;
- future query builders must be designed carefully.

## Required Mitigations

- `polint new-rule` generates a compiling rule and fixtures.
- `polint test` runs real fixture cases and snapshots.
- `polint inspect rule` exposes derived capabilities.
- `polint facts list/sample` and `polint unknowns` expose engine knowledge.
- Fact docs state precision and limitations.
- Preview gates protect unstable fact views and query builders.

## Revisit Criteria

Revisit if:

- agents consistently fail to produce correct Rust rules despite scaffolding;
- compile times make iteration unacceptable;
- typed query builders become too verbose for common policies;
- users demand portable declarative rules more than repo-local Rust power.

Possible future additions:

- a small declarative convenience layer that compiles to Rust rules;
- an experimental graph query/debug shell;
- model-pack generators;
- visual rule/evidence editors.
