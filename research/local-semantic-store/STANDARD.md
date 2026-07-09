# Standard: Local Semantic Store Research

Date: 2026-07-07

## Vocabulary

- **Local semantic store**: the embedded offline persistence layer for facts,
  summaries, graph indexes, evidence metadata, and search manifests.
- **Canonical store**: the authoritative local index used by analysis providers
  and query surfaces. For Static Analysis 2.0 this is SQLite/rusqlite.
- **Payload**: a larger serialized body such as a function summary, evidence
  bundle, or immutable package-summary artifact.
- **Manifest**: a small structured record containing identity, schema version,
  digests, provenance, validation status, and payload references.
- **Stable ID**: deterministic identity derived from semantic inputs, not from
  parser object IDs or insertion order.
- **Precision**: exact/setup-aware/syntax/conservative/heuristic/unknown style
  accuracy tier already used across polint facts.
- **Status**: complete/found/not_found/unknown/budget_exceeded/unsupported style
  result state.
- **Provenance**: source of the fact or edge: native, summary, extension, model,
  query, synthetic, imported, or future registry.
- **Registry-ready seam**: local manifest fields that make remote distribution
  possible later without building networked infrastructure now.

## Required Decision Fields

Every local-store technology recommendation must record:

- primary workload fit;
- install and build risk;
- durability and crash behavior;
- concurrent-reader and writer behavior;
- migration story;
- index/query capability;
- graph traversal capability;
- pruning/compaction story;
- deterministic-output risk;
- remote-summary artifact fit;
- reason accepted or rejected.

## Required Implementation Fields

Every proposed store table or index family must state:

- owner/provider;
- stable identity inputs;
- behavior-affecting digests;
- precision/status/provenance fields;
- validation gate;
- invalidation dependency;
- public exposure, if any;
- whether it can be recomputed from source.

## Public Surface Rule

SQLite tables, SQL queries, internal graph edges, provider IDs, and payload
formats are not public API.

Public surfaces are:

- typed SDK views intentionally exported under `polint::sdk`;
- `polint check` diagnostics;
- `polint review` evidence and review output;
- future `polint graph` JSON envelopes once explicitly promoted;
- documented export formats, if added later.

## Honesty Rule

Unknown, unsupported, setup-missing, heuristic, model-derived, and
budget-exceeded facts must remain visible through persistence and query
results. The store must never turn missing information into an empty result
without status metadata.
