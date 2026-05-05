# Research Summary: exlint

## Key Findings

**Stack:** Rust 2024 with clap, serde, toml, ignore, globset, rayon, petgraph, tree-sitter-go, and Oxc is the right stack for this product.

**Table stakes:** The v1 must compile, run `polint init`, generate a repo-local rule skeleton, check Go/TS fixtures, render human and JSON diagnostics, and demonstrate the SDK through real example rules.

**Watch out for:** Avoid overbuilding secondary surfaces before SDK/rules mature, avoid misleading heuristic diagnostics, and enforce deterministic output despite parallelism.

## Prescriptive Direction

- Start with a working CLI and deterministic core model.
- Treat Go and TS adapters as fact extractors, not full semantic analyzers.
- Put user-facing ergonomics in `polint-sdk`.
- Build example rules only when they prove a reusable API.
- Make the README explicit that this complements normal linters.

## Current Versions Checked

See `STACK.md` for the `cargo search` results checked on 2026-04-28.
