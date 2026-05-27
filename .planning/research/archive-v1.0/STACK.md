# Stack Research: polint

## Recommended Stack

| Area | Choice | Version Checked | Rationale | Confidence |
|------|--------|-----------------|-----------|------------|
| CLI | `clap` with derive | 4.6.1 | Mature Rust CLI parser with strong help text support. | High |
| Serialization | `serde`, `serde_json`, `toml` | serde 1.0.228, serde_json 1.0.149, toml 1.1.2+spec-1.1.0 | Required for config, diagnostics, cache metadata, JSON output, and SARIF-like output. | High |
| Errors | `anyhow`, `thiserror` | anyhow 1.0.102, thiserror 2.0.18 | Application errors in CLI, typed errors in libraries. | High |
| Tracing | `tracing`, `tracing-subscriber` | tracing 0.1.44, subscriber 0.3.23 | Structured internal logging and future profiling integration. | High |
| Parallelism | `rayon` | 1.12.0 | Straightforward parallel parsing and rule execution. | High |
| File discovery | `ignore`, `globset` | ignore 0.4.25, globset 0.4.18 | Fast walking with `.gitignore` support and reliable glob matching. | High |
| Internal relations | `petgraph` | 0.8.3 | Internal representation for relationship facts behind the SDK; no public CLI/export contract. | High |
| Go parsing | `tree-sitter`, `tree-sitter-go` | tree-sitter 0.26.8, tree-sitter-go 0.25.0 | Practical syntax extraction without needing Go type checking in v1. | High |
| TS/JS parsing | Oxc crates | 0.128.0 | Rust-native high-performance JS/TS parser ecosystem. | Medium |
| Import resolution | `oxc_resolver` | 11.19.1 | Useful for future TS import-resolution precision. Initial v1 can start with syntactic imports. | Medium |
| Tests | `insta`, `assert_cmd`, `predicates`, `tempfile`, `pretty_assertions`, `proptest` | Current versions checked | Covers snapshots, CLI integration, fixtures, diffs, and invariants. | High |

## What Not To Use First

- Salsa as a hard dependency for v1 query infrastructure. Keep a cache abstraction and ship the hash-based cache first.
- Full Go semantic analysis in the first pass. Avoid depending on a sidecar until syntax/fact extraction is stable.
- A custom JS/TS parser. Oxc is the right default.
- Leaking full AST dumps through public rule APIs. Prefer stable IDs and incremental facts on the host.

## Version Notes

The dependency versions above were checked with `cargo search` on 2026-04-28 before implementation started.
