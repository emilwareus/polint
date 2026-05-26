# Paper And Source Index

Only sources for currently supported benchmark languages belong in this index.
polint currently supports Go and TypeScript / JavaScript.

## Downloaded Papers

| Source | Local Path | Why It Matters |
|---|---|---|
| SecBench.js: An Executable Security Benchmark Suite for Server-Side JavaScript, ICSE 2023 | `papers/secbench-js-icse-2023.pdf` | External executable JS package vulnerability benchmark model. |

## Online Sources

| Source | URL | Why It Matters |
|---|---|---|
| Go x/tools repository | <https://github.com/golang/tools> | Official Go callgraph APIs and RTA call-edge fixtures used as the primary Go graph benchmark. |
| Jelly repository | <https://github.com/cs-au-dk/jelly> | JS/TS call graph JSON fixtures used as the primary JS/TS graph benchmark. |
| SecBench.js repository | <https://github.com/SecBench/SecBench.js> | Suite source and executable test layout. |
| SecBench.js publication page | <https://publications.cispa.saarland/3909/> | Paper metadata and benchmark framing. |
| gosec repository | <https://github.com/securego/gosec> | Go security analyzer samples and competitor baseline. |
| CodeQL repository | <https://github.com/github/codeql> | Go and JS/TS query-test taxonomy for reference only. |

## Revalidation Notes

- Re-check suite commits, license terms, and published result provenance before
  claiming comparison numbers.
- Do not import results for unsupported languages into polint benchmark tables.
