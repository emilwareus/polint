# Store Contract Sketch

Date: 2026-07-07

This is an implementation sketch, not a public API.

## Contract

The local semantic store persists validated analysis facts and indexes for one
workspace/config/toolchain identity. It provides deterministic reads for
providers, rule execution, review evidence, and future graph/search commands.

The store must be rebuildable from source and imported summaries. Corruption,
future schema versions, invalid manifests, or failed validation must produce a
controlled diagnostic and allow safe rebuild.

## Non-Contract

The following are not public contracts:

- SQLite file layout;
- table names;
- SQL queries;
- internal row IDs;
- provider generation IDs;
- raw graph edge tables;
- payload binary encoding;
- search index storage details.

## Required Row Metadata

Every persisted fact-like row must be reachable from metadata containing:

- fact family;
- stable key;
- provider ID;
- provider schema version;
- source path or package identity when applicable;
- source digest or package/version digest;
- precision;
- confidence when applicable;
- status;
- provenance;
- validation status;
- input/layer key;
- created store generation.

## Commit Protocol

1. Provider computes facts in memory or temporary staging tables.
2. Provider output is validated.
3. Store writes the new provider generation in one transaction.
4. Store updates adjacency/search manifests only after core fact commit.
5. Old generation is retained until dependent indexes are complete or marked
   stale.
6. Query surfaces read only complete generations.

## Invalidation

Invalidation is digest-based:

- file content digest;
- config digest;
- language/toolchain digest;
- package lock/module digest;
- provider schema digest;
- extension/model digest;
- dependency summary digest.

Do not invalidate analysis layers because a rule pack changed unless the rule
pack affects requested capabilities, analysis settings, model facts, or
extension facts. Rule execution results have their own digest boundary.

## Query Result Rules

All query results must:

- sort deterministically;
- use repo-relative paths;
- include precision/status/provenance;
- include unknown and budget regions;
- avoid raw source bodies by default;
- preserve evidence links to facts/spans;
- expose stable IDs, not database row IDs.

## Security And Privacy Rules

The local store must not leak:

- absolute workspace paths in public JSON;
- source bodies unless the command explicitly renders snippets;
- environment variables;
- toolchain cache paths;
- local user names;
- unvalidated extension facts as trusted native facts.

Future remote registry exports must exclude app-private source text by default
and should export dependency summaries only after trust/export policy is
designed.
