# Go Abstract Domains

## Recommended First Domains

| Domain | Recommendation |
|---|---|
| Nilness | P0, local and summary-based. |
| Constants | P0, literals and enum-like constants. |
| Truthiness | Not a general domain; Go conditions are boolean. |
| Intervals | P1, especially loop guards and index checks. |
| Initializedness | P1, useful for definite assignment and partial struct facts. |
| Typestate/resource | P1, high value for `Close`, `Unlock`, transactions, contexts. |
| Shape | P2, mostly struct field presence/state and map key facts. |

## Semantic Inputs

Use official Go tooling where available:

- module roots and workspaces from the existing Go lifecycle model;
- package/type facts from `go/types` or `golang.org/x/tools` where semantic
  mode is enabled;
- build tags, package patterns, and test inclusion as cache inputs.

Do not require a repo-root `go.mod`. The existing lifecycle contract already
requires monorepo support.

## Nilness

Go nilness should model:

- nil constants;
- pointer, interface, map, slice, chan, function nil values;
- `if x == nil` / `if x != nil`;
- assignment and return propagation;
- type assertions and interface unwrapping with precision labels;
- `err != nil` as a high-priority trace partition.

Unknown calls should invalidate facts for places they may mutate unless summary
or alias facts prove otherwise.

## Numeric Ranges

Intervals are enough for the first Go numeric domain:

- loop induction variables;
- array/slice/string index bounds;
- `len` and `cap` relationships;
- integer literal thresholds;
- comparison guards.

Go integer overflow semantics are type-specific and must not be treated as
unbounded mathematical integers when exactness is claimed.

## Typestate

Useful repo-local policies:

- `Open` / `Close`;
- `Lock` / `Unlock`;
- `Begin` / `Commit` / `Rollback`;
- `context.WithCancel` result must have cancel called;
- response/request body close obligations;
- builder-like method sequences.

These should be extension-friendly finite state machines with summaries.

## Transfer Pitfalls

- `defer` changes resource state at function exit, not at statement position.
- `panic` and `recover` require abrupt edges.
- goroutines and channels need summary/effect facts before concurrency claims.
- interface dynamic dispatch depends on type and call graph precision.
- reflection and unsafe should produce explicit unknown facts.

## First Go Slice

1. Lower branches, assignments, calls, returns, defer, panic.
2. Implement nilness and constants over local places.
3. Add `err != nil` partitioning.
4. Add summaries for return nilness and constants.
5. Add resource typestate extension fixtures for `Close`.
