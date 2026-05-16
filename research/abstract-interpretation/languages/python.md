# Python Abstract Domains

## State Of The Art Pattern

The strongest current references split into two families:

- Pyright, mypy, Pyre, and Ty focus on type representation and flow-sensitive
  narrowing.
- Pysa focuses on taint/access-path summaries.
- pytype's bytecode VM is powerful but too heavy and harder to make
  deterministic for polint's first version.

The recommended path for polint is Ty/Ruff-style Rust-native type
representation, Pyright-style narrowing coverage, and Pysa-style taint summaries
only when rules request that capability.

## Recommended Domains

```text
PythonState =
  Type
  x LiteralSet
  x NoneDomain
  x Truthiness
  x CallableSignature
  x TypedDictShape
  x ObjectAttrShape
  x ExceptionFlow
  x AccessPathTaint
```

## Narrowing

Support in order:

1. `x is None` / `x is not None`;
2. literal equality and membership;
3. `isinstance`;
4. `callable`;
5. `TypeGuard` and `TypeIs`;
6. `TypedDict` required-key and tag narrowing;
7. pattern matching singleton/value patterns;
8. known decorators.

Use intersection constraints for `TypeIs` and `isinstance`; use replacement
constraints for `TypeGuard` positive branches.

```python
def apply_constraint(old_type, constraint):
    if constraint.kind == "intersect":
        return intersect(old_type, constraint.type)
    if constraint.kind == "replace":
        return constraint.type
```

## TypedDict And Shape

Split precise `TypedDictShape` from heuristic object attributes:

- required keys;
- optional keys;
- readonly keys;
- tag/discriminant key;
- extra/closed status;
- maybe-undefined fields.

Do not treat arbitrary dicts as precise shapes unless an extension/model
declares a validated invariant.

## Decorators

Special-case:

- `staticmethod`;
- `classmethod`;
- `property`;
- `overload`;
- `final`;
- `override`;
- `dataclass`;
- `dataclass_transform`;
- `contextmanager`;
- `TypeGuard`;
- `TypeIs`.

Unknown decorators should preserve raw signature where possible and mark the
decorated signature unknown or declared external.

## Imports And Metadata

Use official/runtime metadata for environment discovery only:

- interpreter version;
- `sysconfig`;
- site-packages;
- `.pth`;
- installed distribution metadata;
- `py.typed`;
- typeshed/stubs.

Do not import or execute analyzed modules.

## Exceptions

Python exceptions should be represented in CFG edges and coarse effect facts.
Precise exception type propagation is heuristic unless a model/summary provides
it.

## First Python Slice

Python support is not first in current polint adapters, but when added:

1. Build parser/semantic index;
2. implement `None`, literal, and `isinstance` narrowing;
3. add TypedDict shape facts;
4. add known decorator models;
5. add Pysa-style access-path summaries only for taint/dataflow rules.
