# Bootstrap Language Scope

The first implementation should target Go and TS/JS because those adapters
already exist. The architecture must not bake in language-specific facts that
make Java/Python/Rust hard later.

## Go First Slice

Use current tree-sitter Go parsing first.

Supported:

- function declarations;
- simple assignments;
- returns;
- if statements;
- direct function calls;
- selector calls as unresolved/member calls unless a direct symbol is known.

Do not require full `go/packages` semantic loading for this first MIR slice.
Official Go toolchain integration can deepen type/call precision later.

## TS/JS First Slice

Use current Oxc parsing first.

Supported:

- function declarations and arrow functions already recognized by current facts;
- assignments;
- returns;
- if/conditional basics;
- direct calls;
- member calls as explicit call sites with receiver place;
- nullish/truthiness local narrowing.

Do not depend on TypeScript compiler services for the first MIR slice. Type-aware
resolution can be layered later.

## Future Java/Python/Rust

The bootstrap should avoid Go/TS-specific concepts in core IDs/stores:

- MIR operation kinds should be semantic, not AST-node-kind names.
- Places should support member/index/deref/call-return projections.
- Call facts should separate syntax, call kind, target status, and unresolved
  reason.
- Domains should operate over MIR operands/places, not language ASTs.

Language-specific lowering stays in `analysis/mir/lower_<language>.rs` or the
language adapter module. Normalized stores stay language-neutral.
