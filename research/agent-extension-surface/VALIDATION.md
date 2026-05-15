# Validation Plan

## Validate This Research

The research should be considered valid if these checks hold:

- The recommended path is consistent with `AGENTS.md` public API discipline.
- It preserves the existing `#[polint::rule]` rule authoring model.
- It does not expose `core`, parser adapters, or `AnalysisDb` mutation as public SDK.
- It accounts for call graph and data-flow research requirements.
- It explains why Rust code does not require in-process Rust dynamic libraries.
- It provides a concrete lifecycle, not only high-level plugin language.

## Validate Extension Implementations

Every extension should be validated at five levels.

### 1. Manifest Validation

- Stable extension ID.
- Version.
- SDK/protocol version.
- Declared input fact families.
- Declared output fact families.
- Determinism declaration.
- Runtime budget.
- Unsafe capabilities, if any.

### 2. Protocol Validation

- Handshake succeeds.
- Unsupported protocol versions fail clearly.
- Malformed responses are rejected.
- Unknown fact families are rejected.
- Provider outputs match declared outputs.

### 3. Fact Binding Validation

- Referenced files exist.
- Referenced symbols exist or synthetic symbols are declared.
- Referenced callsites exist.
- Access paths are well formed.
- Spans are valid.
- Language kinds match the source file language.
- Precision and provenance are present.

### 4. Fixture Validation

Use expected and unexpected facts:

```toml
[[expected.call_edges]]
callsite = "src/routes.ts:route-handler"
target = "src/handlers.ts:getUser"

[[unexpected.sources]]
symbol = "src/internal.ts:trustedConfig"
kind = "remote"
```

Fixtures should support:

- minimal source snippets;
- multi-file repos;
- generated-code examples;
- framework bootstrap examples;
- negative cases.

### 5. Delta Validation

For an extension to be trusted in CI, measure:

- resolved call edges before/after;
- unresolved calls before/after;
- new entrypoints;
- new data-flow paths;
- source/sink model coverage;
- false-positive/false-negative fixture changes;
- runtime and memory delta;
- number of validation failures.

## Precision Gates

Recommended activation policy:

| Precision | Can Affect CI By Default? | Notes |
|---|---:|---|
| `Exact` | Yes | Bound to symbols/types with deterministic evidence. |
| `ConservativeOverApprox` | Yes, if documented | May add false positives, should not hide unknowns. |
| `Heuristic` | No for error-level rules unless opted in | Good for exploration and warning-level diagnostics. |
| `UserAsserted` | Yes only with explicit trust | Useful for repo owners asserting framework semantics. |
| `GeneratedUnvalidated` | No | Can appear in reports and diffs. |
| `ValidatedByFixture` | Yes | Requires expected/unexpected fact tests. |

## Regression Suite For First Implementation

Minimum test cases:

- Extension compile failure becomes a setup diagnostic.
- Extension panic becomes a controlled diagnostic.
- Malformed fact is rejected.
- Duplicate facts dedupe deterministically.
- Extension output changes cache digest.
- A rule requesting extension-provided facts does not run if the extension failed.
- A rule sees extension-provided facts after validation.
- `--no-cache` still runs extensions but avoids analysis cache reads/writes.
- `polint extension diff` is deterministic.

## Security And Supply Chain

Defaults:

- Do not auto-fetch remote extension code.
- Do not allow network access in extension tests by default.
- Record extension source digest and Cargo.lock digest.
- Surface extension provenance in diagnostics and JSON reports.
- Let teams pin extension crates like any other repo-local code.

Process isolation is a correctness feature, not only a security feature. It prevents a bad extension from taking down the host analysis process.
