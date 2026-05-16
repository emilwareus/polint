# Analysis Kernel Standard

Use this vocabulary when comparing implementations and designing polint internals.

## Core Terms

| Term | Meaning |
|---|---|
| Fact family | A typed group of related facts, such as files, imports, symbols, references, entrypoints, CFG nodes, call sites, call edges, data-flow steps, summaries, effects, diagnostics. |
| Fact row | One typed fact inside a family. It should have a run-local ID and, when cacheable or extension-visible, a stable key. |
| Layer | A named snapshot of facts produced by one provider from declared inputs. |
| Provider | A native analyzer, derived analyzer, extension, model loader, or synthetic provider that emits one or more fact families. |
| Provider manifest | The provider's declared inputs, outputs, schema versions, language scope, parameters, precision ceiling, and cache behavior. |
| Capability | A rule-facing promise that a typed fact view can be built, or a controlled diagnostic explaining why it cannot. |
| Provenance | Where a fact came from and which inputs or parent facts support it. |
| Precision | The semantic approximation category of a fact. Precision is not confidence. |
| Confidence | The host's belief that this particular fact/model binding is correct. Confidence is not soundness. |
| Validation | The checks passed before a fact can influence rules or downstream providers. |
| Stable key | Deterministic identity for a fact/entity across runs, independent of dense in-memory IDs. |
| Run ID | Dense numeric ID used for memory-efficient in-run storage and joins. |
| Unknown fact | A first-class fact representing unresolved or unsupported behavior, not a silent absence. |

## Fact Envelope

The public SDK should not force rule authors to manipulate this envelope for simple rules. Internally, every promotable fact should be representable this way.

```text
FactEnvelope:
    family: FactFamily
    stable_key: StableKey
    run_id: RunId
    payload: TypedPayload
    span: Optional[SourceSpan]
    evidence: EvidenceSet
    provenance_id: ProvenanceId
    precision: Precision
    confidence: Confidence
    validation: ValidationStatus
    layer_id: LayerId
    dependencies: FactDependencySet
```

For performance, do not wrap every current Rust struct in a heavy object. Store typed facts in compact family-specific vectors and keep metadata in side tables keyed by `(family, run_id)` or stable key.

## Fact Layers

Suggested layer kinds:

| Layer kind | Meaning | Examples |
|---|---|---|
| Source | Raw discovered files and content digests. | `SourceFiles` |
| Native syntax | Facts from parser adapters. | Go packages/functions/imports, TS classes/components/literals |
| Native semantic | Facts from language-aware native analysis. | resolved imports, symbols, references |
| Derived | Facts computed from other facts. | module graph, metrics, direct call edges |
| Relation/fixpoint | Facts computed by recursive relation iteration. | reachability, call graph closure, data-flow summaries |
| Extension | Facts emitted by repo-local Rust extensions. | entrypoints, framework call edges, sources/sinks |
| Synthetic | Host-generated placeholder/summary facts. | unknown call, unresolved import, missing setup |
| External import | Facts imported from external indexes or generated metadata. | SCIP, Kythe-like symbols, framework manifests |
| Diagnostic | Findings and capability/setup errors. | rule diagnostics, validation diagnostics |

Every layer should have a manifest:

```text
LayerManifest:
    layer_id: "native.go.syntax.v1"
    provider_id: "polint.go.syntax"
    provider_version: "0.1.0"
    provider_kind: native | derived | extension | relation | external
    inputs: [FactFamily]
    outputs: [FactFamily]
    language_scope: [go | ts | js | java | python | any]
    parameters_digest: Hash
    schema_versions: Map[FactFamily, SchemaVersion]
    precision_ceiling: Precision
    cache_policy: none | per_file | per_package | per_layer | fixpoint
    merge_policy: append_only | union | keyed_replace | overlay | suppress
    validation_policy: ValidationPolicy
```

## Precision

Precision should affect downstream behavior. It must not be just documentation.

| Precision | Meaning |
|---|---|
| Exact | Directly proven by syntax or semantic adapter under declared setup. |
| Conservative | Over-approximation intended to avoid false negatives. May include extra facts. |
| UnderApprox | Under-approximation intended to avoid false positives. May miss facts. |
| Heuristic | Pattern-based or model-based with known gaps. |
| Lossy | Derived after collapsing detail, such as wildcard access paths or summarized traces. |
| UserAsserted | Manually or extension asserted. Requires provenance and validation. |
| GeneratedUnvalidated | Agent or generator emitted, not fixture-validated. Should not power risky suppressions. |
| Unknown | Explicitly unresolved or unsupported. |

Never let extension authors claim `Exact` by assertion alone. `Exact` must be host-verifiable for the fact family.

## Confidence

Confidence is about trust in a specific fact binding:

```text
Confidence:
    level: high | medium | low
    score: optional float
    reason: "resolved unique symbol" | "selector matched 7 overloads" | "fixture validated"
```

A fact can be conservative and high-confidence. A manual model can be heuristic. A generated model can become high-confidence after validation.

## Provenance

Recommended shape:

```text
Provenance:
    origin: parser | native | derived | builtin_model | extension | agent_generated | external | synthetic
    provider_id: string
    provider_version: string
    layer_id: LayerId
    algorithm: string
    algorithm_version: string
    source_digest: Hash
    config_digest: Hash
    extension_digest: optional Hash
    parents: [FactRef]
    evidence: EvidenceSet
    assumptions: [string]
```

Provenance should be cheap to ignore in simple rules but available in debug output, JSON, SARIF, extension diff reports, and high-stakes rules.

## Validation Status

Suggested statuses:

| Status | Meaning |
|---|---|
| NativeTrusted | Host-native provider emitted this fact and passed internal invariants. |
| SchemaValid | Shape and required fields are valid. |
| Resolved | Selectors/spans/entities resolve against existing facts. |
| FixtureValidated | Extension/model has passed declared fixtures. |
| WarningAccepted | Fact is valid but has warnings, such as broad selector match. |
| Rejected | Fact failed validation and must not affect rules. |
| Failed | Provider failed; dependent capabilities should be setup-missing or blocked. |

## Capability Status

Current polint has `Supported`, `Unsupported`, and `SetupMissing`. The kernel should eventually distinguish:

```text
Supported
Unsupported
SetupMissing
Partial
ExtensionFailed
ValidationFailed
BlockedByDependency
TimedOut
BudgetExceeded
```

Rules should not execute against placeholder facts when a required hard capability is not supported.

## Merge Policies

Default merge policy: normalized set union.

Rules:

- native facts cannot be deleted by normal extensions in the first implementation;
- exact conflicts are hard validation errors;
- identical duplicate facts merge provenance and evidence;
- extension facts can augment native facts after validation;
- suppression/neutral facts must be specific to a family and higher risk than additive facts;
- "last writer wins" is forbidden;
- output order must be deterministic.

## Cache Key Units

Use separate digests:

```text
SourceDigest           = hash(relative_path, content)
ShapeDigest            = hash(exported/imported semantic shape)
ConfigDigest           = hash(.polint.toml normalized)
LanguageLifecycleDigest= hash(go module roots, build tags, tsconfig, package roots)
RuleDigest             = hash(enabled rule code/options)
ExtensionDigest        = hash(extension source, Cargo.lock, protocol, options)
ProviderDigest         = hash(provider id, version, schema, algorithm, precision knobs)
PlanDigest             = hash(required fact families and provider choices)
LayerInputDigest       = hash(input layer output digests)
LayerOutputDigest      = hash(normalized output facts and metadata)
```

Parser caches should not depend on `RuleDigest` unless rule configuration changes parser behavior. Today polint includes rule and plan hash in per-file adapter cache keys; that is safe but over-invalidates.

