# Rust Architecture Notes

## Keep The Product API Small

The current library boundary is good:

```text
public:
  polint::sdk
  polint::runner
  polint::rule macro

internal:
  core
  cache
  analysis_plan
  adapters
  module_graph
  symbol_graph
```

The semantic kernel should join the internal list. Do not export internal
providers, stores, sinks, or fact rows until a public SDK view is intentionally
designed.

## Avoid A Mega `core` Module

`core/mod.rs` currently contains IDs, facts, `AnalysisDb`, capabilities, rules,
rule context, and execution helpers. It is already a high-change module. The
semantic engine should not be implemented there.

Preferred ownership:

```rust
pub struct AnalysisDb {
    // existing syntax/module/symbol facts
    semantic: Option<crate::analysis::SemanticStore>,
}
```

Preferred implementation:

```text
analysis/store.rs
analysis/mir/*
analysis/domains/*
analysis/summaries/*
```

This keeps `AnalysisDb` as the product's fact database while keeping the
semantic algorithms modular.

## Static Native Providers, Boundary Dynamic Extensions

Native providers are known at compile time. Use enum dispatch first:

```rust
pub(crate) enum ProviderId {
    Mir,
    Places,
    DirectCalls,
    P0Domains,
    DirectSummaries,
}
```

This gives:

- simple profiling;
- clear dependency tests;
- no object-safety constraints;
- no vtable overhead in hot scheduling loops;
- easier refactoring while internal.

Extensions are different. At the repo-local model boundary, type erasure or a
subprocess protocol may be useful. That belongs in `analysis/extensions`, not in
native provider internals.

## Store Shape

Prefer this pattern:

```rust
pub(crate) struct CallStore {
    sites: Vec<CallSiteFact>,
    site_meta: Vec<FactMeta>,
    targets: Vec<DirectCallTargetFact>,
    targets_by_site: BTreeMap<CallSiteId, Vec<usize>>,
    sites_by_caller: BTreeMap<FunctionId, Vec<usize>>,
}
```

Reasons:

- facts are cache/export friendly;
- IDs are cheap;
- indexes can be rebuilt and tested;
- SDK views can borrow slices/indexes;
- metadata can evolve independently.

Use `BTreeMap` first for deterministic ordering. Consider `HashMap` only after
profiling a real benchmark and preserving deterministic output at the boundary.

## Stable Keys

The symbol graph already has a good length-prefixed stable-key encoder. Create a
general internal helper instead of copying slightly different encoders into each
semantic fact family.

Requirements:

- length-prefix every variable part;
- normalize path separators;
- include fact family/schema;
- include language;
- include owning stable entity where possible;
- include source span or local ordinal when needed;
- keep a debug string available at least in tests and diagnostics.

## Error Policy

Use:

- `thiserror` for `analysis::error::AnalysisError`;
- diagnostics for unsupported/setup-missing/user-facing uncertainty;
- `anyhow` only at CLI/runner/setup boundaries.

This matters because semantic providers will need recoverable engine-level
distinctions. `anyhow::Error` would erase whether a provider had missing inputs,
invalid facts, unsupported semantics, cache mismatch, or extension rejection.

## Comments And Documentation

Follow the local Rust skill:

- public APIs need doc comments;
- internal comments should explain why, not restate what;
- complex invariants belong near validators and in research docs;
- TODOs should be avoided unless tied to a tracked implementation issue.

For this first implementation, prefer concise module-level docs in
`analysis/mod.rs` and `analysis/*/mod.rs` explaining invariants and non-goals.

## Unsafe Policy

The workspace forbids unsafe code. The proposed architecture needs no unsafe.
If a future optimization seems to require unsafe, the burden of proof should be
extremely high and should include benchmark evidence plus an architecture
decision record.
