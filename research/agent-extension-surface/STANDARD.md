# Research Standard: Analysis Extension Systems

Use this structure when comparing extension systems for polint.

## Identity

- Name:
- Domain:
- Language:
- Extension unit:
- Discovery mechanism:
- Execution boundary:
- Public API stability:

## Lifecycle

- When extension code is loaded.
- What input facts/state it receives.
- What output it can produce.
- Whether it runs once, per file, per compilation unit, per project, or on demand.
- Whether it supports multi-pass scan -> model -> analyze flows.
- Whether it can affect downstream analysis facts, only diagnostics, or both.

## Accuracy Impact

- What kinds of false positives it can reduce.
- What kinds of false negatives it can reduce.
- What precision claims it can make.
- What unknowns or ambiguities it exposes.
- Whether it tracks provenance for user-authored/generated facts.

## Complexity And Cost

- Per-file cost.
- Whole-project cost.
- Incremental invalidation model.
- Serialization cost, if any.
- Runtime isolation cost.
- Memory risks.
- Failure behavior on panic/crash/malformed output.

## Validation

- Unit fixture model.
- Integration fixture model.
- Golden outputs.
- Before/after accuracy deltas.
- Schema or API validation.
- Version compatibility validation.
- Determinism validation.

## Fit For Polint

- What to copy.
- What to avoid.
- Required SDK shape.
- Required engine invariants.
- Public/private API boundary implications.
- AI-agent authoring implications.
