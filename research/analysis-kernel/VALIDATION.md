# Kernel Validation Model

Validation is how polint lets agents extend analysis without making the engine arbitrary or unsafe.

## Validation Principles

1. Invalid facts must not affect rules.
2. Rejected facts should become diagnostics with enough evidence for an agent to fix them.
3. Validation must be deterministic.
4. Additive facts are lower risk than negative/suppressing facts.
5. Precision claims must be checked against evidence and provider trust.
6. Setup gaps should block dependent capabilities, not produce placeholder facts.

## Gates

### Schema Gate

Checks:

- required fields;
- enum values;
- schema version compatibility;
- family/provider output declaration;
- language scope;
- precision field present;
- stable key present when required.

### Referential Gate

Checks:

- file IDs or stable file keys exist;
- spans are within file bounds;
- symbol IDs exist;
- call-site IDs exist;
- data-flow nodes exist;
- parent fact refs exist.

### Selector Gate

For extension facts that bind by selector:

- selector parses;
- selector matches at least one entity unless zero-match is explicitly allowed;
- match cardinality is recorded;
- broad matches can be warnings or errors depending on family;
- language/package scope is respected.

### Access-Path Gate

For summaries, sources, sinks, barriers, and effects:

- argument index exists;
- receiver exists when referenced;
- return slot exists when referenced;
- field/element paths respect maximum depth;
- wildcard paths are marked lossy;
- path kind matches language semantics.

### Precision Gate

Checks:

- extension cannot claim `Exact` unless host can verify the fact;
- generated facts default to `GeneratedUnvalidated` or `Heuristic`;
- barriers/sanitizers require stricter validation than sources/sinks;
- native heuristics are labeled as heuristics.

### Merge Gate

Checks:

- stable-key uniqueness;
- duplicate normalization;
- exact conflicts fail;
- exact facts beat heuristic shadows;
- suppressions only apply to declared suppressible families;
- merge output sorted deterministically.

### Fixture Gate

Risky extension facts should require fixtures before promotion:

- one positive fixture proving the fact creates expected analysis output;
- one negative fixture proving it does not over-match or hide output;
- cache invalidation fixture for extension source/options changes.

This should be mandatory for sanitizers, barriers, suppressions, and broad framework summaries.

## Validation Status Matrix

| Status | Can affect rules? | Can feed downstream providers? | Notes |
|---|---:|---:|---|
| `NativeTrusted` | Yes | Yes | Native provider and invariants passed. |
| `SchemaValid` | No by itself | No by itself | Shape is valid but not resolved. |
| `Resolved` | Usually | Usually | Resolved against current facts. |
| `FixtureValidated` | Yes | Yes | Strongest extension status. |
| `WarningAccepted` | Yes, with precision label | Yes, with precision label | Should appear in debug/delta output. |
| `Rejected` | No | No | Emit diagnostic. |
| `Failed` | No | No | Provider failure; block dependents. |

## Diagnostics

Validation diagnostics should use stable rule IDs:

```text
polint/capability
polint/kernel/schema
polint/kernel/reference
polint/kernel/selector
polint/kernel/precision
polint/kernel/merge-conflict
polint/kernel/cache
polint/kernel/provider-failed
```

Example:

```text
polint/kernel/selector
Extension repo.fastify emitted entrypoint selector "app.route"
but it matched 47 symbols across 12 files. Add package/file scope or fixture coverage.
```

## What To Validate In Tests

- provider output is deterministic across runs;
- cache output digest is stable;
- duplicate identical facts dedupe;
- duplicate conflicting facts reject;
- extension facts cannot reference nonexistent files/spans/symbols;
- extension precision ceiling is enforced;
- failed provider blocks dependent rules;
- rejected facts do not appear in SDK views;
- debug reports include provenance and validation status.

