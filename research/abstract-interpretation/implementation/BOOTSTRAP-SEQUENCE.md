# Bootstrap Sequence After The Research

The research tracks create a circular dependency if interpreted as one linear
"build call graph, then data flow, then summaries" path. The correct sequence is
tiered: build cheap direct facts first, then the shared local substrates, then
refined interprocedural analyses.

## Corrected Order

1. **Kernel skeleton and fact metadata**
   - typed fact families;
   - provenance/status/precision sidecars;
   - provider DAG;
   - deterministic IDs and ordering.

2. **Evaluation harness minimum**
   - `polint-expect` fact fixtures;
   - JSON fact snapshots;
   - deterministic worker-order tests;
   - extension delta report shape.

3. **Semantic operation MIR and CFG**
   - MIR contract from `implementation/MIR-CONTRACT.md`;
   - basic blocks, terminators, typed edges;
   - edge-specific effects;
   - MIR-shape assertions.

4. **Place/value/type substrate**
   - shared `PlaceId`;
   - access paths;
   - allocation tokens;
   - declared/inferred/narrowed type envelopes;
   - abstract values for constants, nullish, truthiness, function/class/module
     values.

5. **Direct/syntactic call facts**
   - call sites;
   - direct callees;
   - unresolved call facts;
   - no refined whole-program call graph yet.

6. **P0 local abstract domains**
   - reachability;
   - nilness/nullish;
   - truthiness;
   - constants;
   - local reductions;
   - law/property tests.

7. **Minimal summary kernel slice**
   - context-insensitive direct summaries;
   - summary algebra from `implementation/SUMMARY-ALGEBRA.md`;
   - caller-place substitution;
   - unknown/havoc policy;
   - dependent summary digests.

8. **Minimal cache/invalidation slice**
   - source, config, lifecycle, semantic schema, domain version, reduction graph,
     widening policy, extension manifest, and summary dependency digests;
   - leave full incremental query research for later.

9. **Model-extension slice**
   - subprocess-style extension protocol;
   - canonical sinks;
   - guard model fixture;
   - summary model fixture;
   - suppressive output review gate.

10. **Refined call graph implementation design**
    - consume direct call facts, types/values, points-to, framework dispatch,
      summaries, and extension models;
    - define provider tiers, precision labels, merge policy, benchmark gates.

11. **Refined data-flow implementation design**
    - consume CFG, refined call graph, places, values, summaries, abstract
      domains, and extension models.

12. **P1 domains and public SDK views**
    - strings, initializedness, intervals, shapes, typestate;
    - public views only after docs, fixtures, cache tests, merge rules, and
      diagnostics exist.

## Gates Before Public Fact Views

No public `Nilness<'_>`, `Constants<'_>`, `StringValues<'_>`, or extension
domain view should ship until it has:

- fact documentation;
- precision and unsupported-semantics docs;
- inline fact fixtures;
- temp-repo rule fixture using only public SDK imports;
- cache digest regression tests;
- deterministic output tests;
- extension merge/conflict tests where extensions can affect it;
- diagnostic examples with precision-aware wording.

## Why This Avoids The Cycle

Call graph and data flow still matter, but the first implementation does not
need a refined whole-program graph. It needs direct call facts, local domains,
and simple direct summaries. Those are enough to validate the kernel, MIR,
places, facts, cache identity, and extension sinks before the expensive global
analyses are revisited.
