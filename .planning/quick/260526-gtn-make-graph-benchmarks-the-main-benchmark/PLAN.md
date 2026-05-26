---
status: in_progress
date: 2026-05-26
---

# Quick Task: Make Graph Benchmarks The Main Benchmark

Goal: Add one Go graph benchmark suite and one TypeScript/JavaScript graph
benchmark suite to the internal evaluation harness, making graph/call-edge
quality the primary benchmark path for this PR.

Constraints:
- Do not commit.
- Stay within currently supported languages: Go and TypeScript/JavaScript.
- Keep benchmark repositories under ignored `research/evaluation-harness/repos/`.
- Preserve public API discipline; eval surfaces remain internal.

Implementation plan:
1. Add graph-specific external adapters for Go x/tools callgraph fixtures and
   Jelly JS/TS callgraph fixtures.
2. Add supported suite manifests for both graph suites with deterministic fast
   and nightly/release tiers.
3. Update evaluation-harness docs and roadmap text so graph benchmarks are the
   main benchmark track and vulnerability suites are secondary.
4. Add focused Rust tests for manifest parsing, deterministic enumeration, safe
   target preparation, and suite-native graph metrics.
5. Run the relevant test subset and report remaining limitations.
