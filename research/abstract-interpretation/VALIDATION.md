# Validation Plan

## Reference Validation

The research used three evidence types:

1. Primary papers and official docs downloaded into `papers/`.
2. Local sparse clones of implementation repositories under `repos/`.
3. Parallel subagent reports cross-checking theory, tools, languages,
   extension design, and benchmarks.

Known correction:

- An attempted Clang developer manual URL returned 404 and was removed. The
  folder now contains the official Static Analyzer overview and DebugChecks
  pages instead.

## Domain Validation Gates

Before enabling a domain:

| Gate | Requirement |
|---|---|
| Lattice laws | property tests for reflexivity, antisymmetry, transitivity, join idempotence, commutativity, associativity, and upper-bound laws. |
| Transfer monotonicity | sampled and fixture-based tests that transfer preserves ordering. |
| Widening safety | widening over-approximates and converges on loop samples. |
| Serialization | stable round trips and stable digests. |
| Determinism | identical output across worker counts, file orders, and repeated runs. |
| Cache coverage | config, lifecycle, extension, domain version, and summary dependencies affect digests. |
| Precision docs | each public fact view documents unsupported constructs and heuristic behavior. |

## Extension Validation Gates

Agent-authored or user-authored Rust extensions must pass:

- manifest/schema validation;
- capability declaration checks;
- domain law tests if they define a domain;
- transfer monotonicity samples if they define transfer hooks;
- merge policy validation;
- suppressive model review gate for sanitizers/barriers/suppressions;
- deterministic fact-batch output;
- canonical sink ordering and stable IDs;
- panic capture and failure diagnostics;
- timeout/resource-limit enforcement;
- execution isolation policy for untrusted Rust;
- cache-key inclusion for source, config, Cargo.lock, artifact, and model data.

If an extension fails at runtime, polint should disable only that component,
emit `polint/extension` diagnostics, and keep the kernel alive.

For public agent-authored Rust extensions, validation must be paired with
isolation. The recommended first external mode is a subprocess provider that
receives read-only semantic snapshots and returns canonical fact batches. Native
in-process Rust is acceptable only for built-ins or explicitly trusted
workspace extensions.

## Fact Validation

Use fact assertions:

```text
polint-expect: nilness(x) = non_null
polint-expect: constants(route) contains "/users/:id"
polint-expect: range(i) <= [0, len(xs)-1]
polint-expect: typestate(conn) = closed at exit
```

Fact assertions should include precision expectations:

```text
precision: exact-local | summary-based | heuristic | unknown-top
```

## Diagnostic Validation

Diagnostics generated from heuristic facts must not use "must" language unless
the domain can justify it. A diagnostic should be able to explain:

- facts used;
- precision labels;
- summaries used;
- extension models used;
- where widening/top happened;
- unsupported constructs that could affect the result.

## Benchmark Validation

Track:

- precision/recall;
- top rate;
- unsupported/budget rate;
- runtime;
- memory;
- cache hit behavior;
- deterministic output;
- default-vs-extension deltas.

The extension delta is product-critical. The harness should show whether an
agent-authored model improved precision, hid uncertainty, or introduced unsafe
suppression.

## Implementation Readiness Checklist

- [x] Theory sources identified.
- [x] Production analyzers inspected.
- [x] Language-specific domain strategies written.
- [x] Extension validation strategy written.
- [x] Benchmark strategy written.
- [ ] Native `polint-expect` fact fixture format designed.
- [ ] First P0 domain implementation planned.
- [ ] Domain law test helpers implemented.
- [ ] Extension manifest/schema implemented.
- [ ] MIR-shape fixture assertions implemented.
- [ ] Summary algebra and caller-place substitution fixtures implemented.
- [ ] Extension execution isolation mode implemented.
