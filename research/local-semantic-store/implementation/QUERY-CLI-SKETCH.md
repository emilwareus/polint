# Query CLI Sketch

Date: 2026-07-07

This document sketches the future local graph CLI. It is not locked as a public
contract yet.

## Product Role

`polint graph` is an exploratory query surface for humans and AI agents. It
answers codebase-understanding questions and returns evidence.

It is not the CI gate. When a graph query should become a policy, users promote
it into a repo-local Rust rule consumed by `polint check` and `polint review`.

## Commands

```bash
polint graph used-by --at src/api.ts:42:15 --format json
polint graph used-by --symbol symbol:... --format json
polint graph neighbors --symbol symbol:... --edge refs,calls,flows --depth 1
polint graph callers --symbol function:...
polint graph callees --symbol function:...
polint graph path --from function:... --to function:... --edge calls --max-depth 8
polint graph taint --source "http.request.*" --sink "sql.query" --barrier "sanitize.*"
polint graph search "sanitize email" --kind symbol,evidence,summary
```

## Shared Flags

```text
--format human|json|jsonl
--path <glob>
--include-tests
--min-precision exact|setup-aware|syntax|conservative|heuristic
--provenance native,summary,extension,model,query,synthetic
--unknowns include|only|exclude
--max-depth <n>
--max-paths <n>
--limit <n>
```

## JSON Envelope

```json
{
  "version": 1,
  "schema": "polint.graph.result",
  "tool": "polint",
  "command": "used-by",
  "query": {
    "selector": "src/api.ts:42:15"
  },
  "status": "complete",
  "precision": "setup-aware",
  "nodes": [],
  "edges": [],
  "paths": [],
  "findings": [],
  "unknowns": [],
  "summary": {
    "node_count": 0,
    "edge_count": 0,
    "path_count": 0
  }
}
```

## Status Values

Recommended status vocabulary:

- `complete`
- `partial`
- `not_found`
- `unknown`
- `budget_exceeded`
- `unsupported`
- `setup_missing`

## Node Fields

```json
{
  "id": "symbol:...",
  "kind": "function",
  "label": "handleRequest",
  "path": "src/api.ts",
  "span": {"start": {"line": 42, "column": 1}, "end": {"line": 51, "column": 2}},
  "precision": "exact",
  "status": "complete",
  "provenance": "native"
}
```

## Edge Fields

```json
{
  "id": "edge:...",
  "kind": "calls",
  "from": "function:...",
  "to": "function:...",
  "span": {"path": "src/api.ts", "start": {"line": 45, "column": 10}},
  "precision": "setup-aware",
  "status": "complete",
  "provenance": "native",
  "evidence": ["fact:..."]
}
```

## Non-Goals

- no public SQL;
- no public Cypher/QL/SPARQL;
- no raw CFG/MIR/provider/solver IDs;
- no whole-program soundness claim;
- no external graph DB requirement;
- no CI pass/fail semantics;
- no replacement for editor LSP.
