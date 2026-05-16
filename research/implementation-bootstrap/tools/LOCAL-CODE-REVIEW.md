# Local Code Review Notes

This file preserves the highest-signal local source review points for future
implementation agents.

## Public Boundary

`crates/polint/src/lib.rs`:

- public: `runner`, `sdk`, `rule`, `run_main`;
- internal: `analysis_plan`, `cache`, `core`, `go`, `ts`, `module_graph`,
  `symbol_graph`, etc.;
- hidden bench feature: `_bench`.

Implication: add `analysis` as internal. Do not expose semantic engine internals
through root re-exports.

## Core IDs And Facts

`crates/polint/src/core/mod.rs`:

- good small IDs at the top of the file;
- `FunctionFact.calls` is just `Vec<String>`;
- `AnalysisDb` stores many fact families and indexes;
- `facts_for_file()` clones file facts for cache serialization;
- `restore_file_facts()` remaps function and branch IDs.

Implication: use the ID style, but introduce semantic sub-stores and stable keys
for cross-file/interprocedural facts.

## SDK Views

`crates/polint/src/sdk/facts.rs`:

- fact views are `Copy` wrappers around `&AnalysisDb`;
- methods return slices/iterators;
- reserved views exist for `Cfg`, `CallGraph`, `DataFlow`.

Implication: this is the right public view pattern, but keep semantic views
empty/unsupported until promotion gates are met.

## Macro Capability Mapping

`crates/polint-macros/src/lib.rs`:

- canonical fact paths enforced;
- placeholder lifetime required;
- view type maps to capability string.

Implication: new public fact views require macro capability mapping and tests.
Do not add alternate rule-author paths.

## Graph Builders

`symbol_graph/model.rs` and `module_graph/model.rs`:

- use draft records;
- deterministic maps/sets;
- finish sorting;
- validation/collision diagnostics;
- replacement into `AnalysisDb`.

Implication: new semantic builders should follow this shape.

## Cache Keys

`cache/keys.rs`:

- deterministic manual encoders;
- config/rule/options digest;
- avoids serializer ordering assumptions.

Implication: semantic cache keys should use the same style with more inputs.

## Adapters

`go/adapter.rs` and `ts/adapter.rs`:

- per-file local database;
- parallel map;
- sort by file ID before merge;
- cache per file;
- parser AST/allocator lifetimes local to file analysis.

Implication: MIR lowering can use this shape. Do not store parser AST references
in semantic facts.
