# Rust Lessons For The Kernel

This is not a Rust-language-support plan. It captures kernel lessons from rustc
that polint should copy for all languages.

## Core Lessons

rustc MIR dataflow uses:

- a small `Analysis` trait;
- a `JoinSemiLattice` domain;
- deterministic fixpoint iteration;
- early and primary statement/terminator effects;
- call-return edge effects;
- switch edge effects;
- `ResultsCursor` and `ResultsVisitor`.

This shape is ideal for polint's internal domain kernel.

## Move Paths And Places

rustc tracks initialization over canonical move paths, not strings. polint
should similarly intern places/access paths:

```text
root + projections
```

This supports:

- initializedness;
- nilness;
- shape;
- typestate;
- alias/points-to;
- data flow;
- summaries.

## Maybe/Definitely Queries

rustc often tracks "maybe initialized" and "maybe uninitialized" separately.
Definite facts are derived by complement/intersection. This is a robust pattern
for domains where under- and over-approximations answer different rule queries.

## Edge-Specific Effects

Do not apply all effects at a terminator uniformly:

- call return place is initialized only on success edge;
- unwind/panic/exception edge has separate effects;
- switch cases refine discriminants;
- cleanup/defer/finally edges change resource state.

This is directly relevant to Go `defer`, TS/JS `throw`/`await`, Python
exceptions/context managers, and JVM exceptions.

## Permissions / Loans

rustc's borrow checker gathers loans and uses place conflict queries. polint
should not copy Rust borrowing as a universal model, but the pattern is useful:

```text
resource/permission id
  + affected place
  + active-at-location bitset
  + conflict query
```

This can model locks, transactions, file handles, mutable aliases, and framework
resource scopes.

## What Not To Copy

- Do not expose rustc-like internals publicly.
- Do not make Polonius/Datafrog the first relation engine.
- Do not assume Rust ownership semantics apply to Go/JS/Python.

Copy the dataflow architecture, not the language-specific rules.
