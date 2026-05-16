# Standard: Rule SDK, Query Ergonomics, And Agent Authoring

Use this standard when reviewing new rule SDK APIs, model formats, provider
extension APIs, query builders, fixture runners, or agent-facing commands.

## Vocabulary

| Term | Meaning |
|---|---|
| Rule | Rust function that reads supported typed fact views and emits diagnostics. |
| Rule pack | Repo-local Rust crate that registers one or more rules. |
| Rule manifest | Engine-generated description of a rule: id, metadata, typed fact requirements, capability plan, options, fixability, precision notes, and SDK version. |
| Fact view | Public typed read-only SDK view such as `Imports<'_>`, `Symbols<'_>`, `Calls<'_>`, or `DataFlow<'_>`. |
| Query builder | Typed helper over fact views, such as `calls.named(...)`, `flow.sources(...).to(...).paths()`, or architecture-boundary builders. |
| RuleCtx | Narrow per-rule context for diagnostics, options, source/path metadata, setup/capability status, and future structured fixes. It must not become a broad fact database handle. |
| Model | Declarative data fact about API/framework behavior under a fixed engine, such as source, sink, sanitizer, barrier, summary, propagator, entrypoint, or generated-client mapping. |
| Model pack | Versioned collection of declarative models with target analysis family, package/language applicability, provenance, validation, and tests. |
| Provider extension | Process-isolated Rust code that emits validated facts or changes analysis lifecycle/semantics under a versioned protocol. |
| Summary | Reusable function/API behavior fact, such as argument-to-return flow, side effect, resource effect, or control effect. It may be native, modeled, or provider-emitted. |
| Fixture | Isolated test repository used to assert diagnostics, facts, model deltas, provider outputs, or fixes. |
| Agent inspect tool | Machine-readable command that exposes facts, unknowns, capability plans, rule manifests, model matches, evidence, and default-vs-extended deltas. |

## Required API Review Questions

Every rule SDK or query API must answer:

- Can a simple rule be written as a plain `#[polint::rule]` function?
- Are all required capabilities derived from typed parameters or explicit
  metadata rather than hidden `RuleCtx` calls?
- Can an agent inspect the generated manifest before running the rule?
- Does every fact family document precision: syntactic, resolved, type-aware,
  interprocedural, heuristic, experimental, or unsupported?
- Is the query bounded by default, or can it accidentally return an unbounded
  graph/path set?
- Are diagnostic spans separate from match context spans?
- Does the API expose structured evidence for path/graph-backed diagnostics?
- Is there a required fixture path for positive and negative examples?
- Are rule options schema-checked and cache-keyed?
- Are models separated from provider extensions?
- Are generated or heuristic artifacts labeled differently from handwritten
  validated artifacts?
- Can the engine explain why a rule did not run because setup/capabilities were
  missing?
- Can the agent compare default mode against model/provider-extended mode?

## Artifact Selection Matrix

Use a rule when:

- the policy is expressible from existing public fact views;
- the output is a diagnostic, severity, message, evidence, and optional fix;
- the logic is repo-specific but does not need new analysis facts.

Use a model when:

- the behavior is about an existing API, framework, package, generated client,
  source, sink, sanitizer, barrier, entrypoint, summary, or propagator;
- the behavior benefits multiple rules;
- declarative data is enough and the engine semantics are unchanged.

Use a provider extension when:

- the required fact family does not exist;
- a repo framework/lifecycle needs code to recover facts;
- transfer functions, call resolution, dispatch modeling, generated-source
  mapping, or precision policy must change;
- declarative models cannot express the behavior.

Use a summary when:

- the reusable semantic fact is about function/API behavior such as
  argument-to-return flow, side effects, exits, sanitization, resource effects,
  or callback invocation.

Use a benchmark fixture when:

- the agent is unsure;
- a false positive or false negative needs proof;
- a model/provider/rule changes precision, recall, runtime, or cache behavior.

## Complexity And Cost Reporting

Rule documentation should state:

```text
fact views requested
analysis families triggered
local vs interprocedural behavior
expected query bounds
path/evidence limits
cache inputs
known precision limits
```

Avoid hiding expensive capabilities behind innocent method names. A rule that
requests global data flow or all call paths should make that cost visible in the
manifest and diagnostics.

## Testing Standard

Every generated rule should start with:

- one positive fixture;
- one negative fixture;
- normalized JSON diagnostic snapshot;
- inline expectation markers;
- explicit rule options if any.

Every model pack should have:

- default-vs-modeled delta test;
- matched model snapshot;
- diagnostic or fact snapshot;
- stale/dead model diagnostics when possible.

Every provider extension should have:

- protocol handshake test;
- emitted fact snapshot;
- consumer-rule test;
- failure-mode test;
- determinism test;
- cache invalidation test for provider code and model inputs.
