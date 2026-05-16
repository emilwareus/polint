# Extension Domain Contract

polint's product advantage is that agents and teams can write Rust code that
improves analysis accuracy for a specific repository. That power needs a strict
contract: extensions emit validated products into kernel-owned sinks; they do
not mutate kernel stores directly.

## Product State Model

Use a hybrid product:

```rust
pub(crate) struct ProductState {
    core: CoreDomains,
    extension_slots: ExtensionDomainSlots,
    meta: StateMeta,
}

pub(crate) struct CoreDomains {
    reachability: ReachabilityDomain,
    nilness: NilnessDomain,
    truthiness: TruthinessDomain,
    constants: ConstantsDomain,
    strings: StringDomain,
    ranges: IntervalDomain,
    initialized: InitDomain,
    shape: ShapeDomain,
    typestate: TypestateDomain,
    predicates: PathPredicateDomain,
}
```

Fixed core slots keep P0/P1 domains fast. Registry-backed extension slots allow
future law-checked products without changing the core struct for every
repo-specific analysis.

First version recommendation: support model extensions before arbitrary domain
extensions.

## Extension Classes

| Class | Outputs | First Version? |
|---|---|---|
| Guard model | predicate refinements for built-in domains | yes |
| Summary model | domain summary payloads for known functions/frameworks | yes |
| Typestate model | finite state machines over resources/objects | yes |
| Reducer model | selected reductions between built-in domains | limited |
| Domain extension | new domain slot with lattice/transfer/summary payload | later |

Domain extensions are powerful, but they require registry-backed product state,
domain law harness, summary algebra, cache identity, deterministic scheduling,
and process policy before public use.

## Manifest

Each extension manifest declares:

```text
extension id and version
product kinds
input fact families
output fact families
merge policy
precision/status defaults
validation suite id
cache-relevant config
source/artifact/lock/toolchain/features/target digests
external model data digests
suppressive output flag
resource limits
isolation mode
```

Registration returns typed sink handles only. Extensions do not receive raw
stores, caches, parser ASTs, or diagnostic writers.

## Deterministic Sink Contract

All extension output must be canonical:

- stable IDs;
- sorted fact batches;
- stable summary ordering;
- stable merge ordering;
- no filesystem-order dependence;
- no clock/random/env-dependent output unless declared as an input digest;
- byte-identical output across worker counts.

Invalid, unsorted, duplicate, non-canonical, or schema-mismatched batches are
rejected before merge.

## Merge Policy

Every fact family declares exactly one merge policy:

| Policy | Use |
|---|---|
| `Join` | Additive possible facts, sources, sinks, extra summaries. |
| `MeetForPrecision` | Guard refinements that are stronger but still conservative. |
| `ConservativeTopOnConflict` | Conflicting value facts where safe fallback is unknown. |
| `RejectConflict` | Facts where conflict means invalid extension/model. |

Never use last-writer-wins.

Suppressive products such as sanitizers, barriers, suppressions, "not a source,"
or "safe sink" need stricter review and stronger evidence than additive
products.

## Validation Layers

Validation has three layers:

1. **Manifest/static validation:** schema, declared capabilities, dependency
   graph, merge policy, cache inputs, suppressive outputs.
2. **Fixture/property validation:** lattice laws, transfer monotonicity,
   widening convergence, stable serialization, stable digests, merge conflicts,
   cache invalidation, expected fact output.
3. **Runtime quarantine:** timeout, panic, abort, invalid output, nondeterminism,
   non-monotone sampled transfer, or resource overuse disables only that
   component and emits `polint/extension`.

Sampled law checks are necessary but not proof. Core domains still need small
manually reviewed implementations and focused fixtures.

## Rust Execution Isolation

Validation alone cannot make arbitrary Rust deterministic or safe. Public
extension execution needs an isolation policy.

| Mode | Capability | Use |
|---|---|---|
| Trusted in-process crate | Maximum speed and Rust expressiveness | built-ins and explicitly trusted workspace extensions. |
| Subprocess provider | Crash containment, resource limits, stable protocol | recommended first external-agent extension mode. |
| WASM provider | Stronger isolation but narrower Rust surface | future option if safety dominates expressiveness. |

Untrusted repo-local Rust should run out of process with a narrow protocol:
kernel sends read-only semantic snapshots and receives canonical fact batches.

## Cache Keys

Extension cache identity includes:

- manifest;
- source digest;
- Cargo.lock digest;
- artifact digest;
- rustc/toolchain version;
- target triple;
- feature flags;
- build profile;
- proc macro/generated code digests when applicable;
- validation digest;
- validation schema version;
- merge policy version;
- budget/limit config;
- external model data digests.

## Diagnostic Wording

`DeclaredExternal` and extension-provided facts are not automatically
`ExactSemantic`. Diagnostic wording must be gated by precision/status. A rule
should not say "must" when its key fact depends on an unvalidated or heuristic
extension product.
