# Entry 8: Python Adapter

## Goal

Add Python through the same adapter contract, starting with a declared subset
and expanding toward capability parity.

## Why

Python is a common target for repo-local policy. It should use the same polint
rule SDK, fact model, diagnostics, cache, and capability plan as Go and TS/JS.

## Difficulty

**M** for syntax facts, **L** for imports/tests/coverage, **XL/XXL** for symbol
and call precision.

## Initial Capability Tier

- syntax facts
- functions and classes
- imports
- literals
- basic branches
- pytest/unittest test facts
- coverage.py import

## Build Method

1. Add Python language detection and config.
2. Start with syntax, functions/classes, imports, literals, and basic branches.
3. Prefer a Rust parser or tree-sitter for baseline analysis if it keeps the
   engine self-contained.
4. Optionally use Python's `ast` and `symtable` through a sidecar when exact
   CPython grammar/symbol behavior is worth the setup cost.
5. Support configured interpreter or virtualenv for import resolution.
6. Consume coverage.py JSON/XML/LCOV output for coverage facts.
7. Add pytest/unittest test metrics.
8. Mark dynamic call targets and runtime imports as unresolved unless setup
   provides stronger evidence.

## Done When

- Python rules can be written with `polint::sdk::prelude::*`.
- The support matrix clearly states the Python capability tier.
- Missing interpreter/virtualenv setup produces diagnostics.
- External generated-rule tests cover the supported facts.

## Full Coverage Path

Move Python toward parity after Go and TS/JS prove the complete model.
