# Implementation Bootstrap Standard

This standard is for implementation-ready research tracks that translate
algorithm research into Rust architecture for polint.

## Required Questions

Every implementation bootstrap report should answer:

1. What existing code should be preserved?
2. What existing code should not be stretched further?
3. What should be internal only?
4. What, if anything, should become public SDK surface?
5. What IDs are dense runtime IDs and what IDs are stable cache/provenance keys?
6. What data is hot-path and must avoid dynamic dispatch/allocation churn?
7. What data is boundary/extension-facing and can use richer validation?
8. What errors should be typed versus `anyhow`?
9. What facts need provenance, precision, status, or evidence?
10. What tests must exist before public exposure?

## Vocabulary

| Term | Meaning |
| --- | --- |
| Dense ID | Small `Copy` ID assigned for in-memory indexing during one analysis run. |
| Stable key | Deterministic identity derived from semantic inputs and suitable for cache/provenance. |
| Fact store | Typed `Vec<T>` plus indexes for one fact family or closely related family. |
| Fact metadata | Sidecar record for provenance, precision, status, confidence, and validation. |
| Provider | Internal analysis step that consumes fact families and emits new fact families. |
| Provider DAG | Deterministic dependency graph of providers requested by an `AnalysisPlan`. |
| Native provider | Built into `crates/polint`, scheduled by enum or static function table. |
| Extension sink | Typed boundary where repo-local Rust model code emits candidate facts. |
| Merge gate | Validator that accepts, downgrades, rejects, or conflicts extension/native facts. |
| MIR | polint-owned semantic operation representation, not a parser AST. |
| Place | Language-normalized access path or storage location for local/summary/data-flow use. |
| P0 domains | The first local abstract domains: reachability, nullish/nilness, truthiness, constants. |

## Evidence Classes

| Class | Source |
| --- | --- |
| Local implementation fact | Direct source-code inspection in this repository. |
| Existing research fact | Prior research in `research/*`. |
| Rust design guidance | Local Rust skill or official Rust docs. |
| Recommendation | Interpretation for polint, with confidence label. |
| Decision | Proposed project direction recorded in `decisions/DECISIONS.md`. |

## Rust Design Checklist

Use this checklist for every new semantic-analysis module:

- Visibility is `pub(crate)` by default.
- Public SDK exposure is delayed until there are docs and temp-repo tests.
- Small IDs are `Copy`, `Ord`, `Hash`, `Serialize`, `Deserialize`.
- Functions accept `&str`, `&[T]`, or IDs by value where possible.
- Hot stores use `Vec<T>` plus deterministic indexes.
- Merge/order-sensitive structures use `BTreeMap`/`BTreeSet` unless profiling
  proves a hash map is needed.
- Parser AST references do not escape adapter-owned lifetimes.
- `anyhow` stays at CLI/runner/setup boundaries; kernel errors use
  `thiserror`.
- Native hot-path scheduling uses enums/static dispatch first; dynamic dispatch
  is allowed at extension/process boundaries.
- `unsafe` remains unnecessary and forbidden.
- Cache keys include schema/domain/provider/lifecycle/extension inputs.
- Every heuristic fact has explicit precision/status/provenance.
