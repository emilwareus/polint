# Retrospective

## Milestone: v1.0 - MVP

**Shipped:** 2026-05-02
**Phases:** 9
**Plans:** 32

### What Was Built

- Rust 2024 workspace and CLI.
- Config, discovery, diagnostics, and core facts.
- Go and TypeScript/JavaScript adapters.
- SDK and example rules.
- Cache/performance support, CI output, and graph commands.
- README, examples, and release verification.

### What Worked

- Phase-by-phase GSD kept scope explicit.
- Honest heuristic and experimental wording avoided overclaiming.
- Final audit caught CLI contract details like cwd-based execution and profile output format.

### What Was Inefficient

- Some older summary frontmatter was incomplete and required audit reconciliation against phase verification.
- STATE and ROADMAP helpers left small wording drift that needed manual correction.

### Patterns Established

- Work directly on `main`; no GSD worktrees.
- Built-in `examples/...` rules are SDK dogfood, not a comprehensive lint pack.
- Release readiness means docs plus `cargo fmt -- --check`, clippy `-D warnings`, and `cargo test --workspace`.

### Key Lessons

- Prefer phase verification files as source of truth when old summaries omit requirement IDs.
- Keep command examples aligned with the actual CLI contract.
- Archive milestone artifacts before deleting live requirements.

## Milestone: v1.2 - Static Analysis Engine Implementation

**Shipped:** 2026-05-27
**Phases:** 22
**Plans:** 136

### What Was Built

- Private analysis kernel, provider manifests, provenance metadata, and deterministic validation.
- Internal evaluation harness, input snapshots, cache-key vocabulary, and persistent layer cache.
- Semantic index, module/package topology, semantic MIR, CFG/control dependence, and direct-call facts.
- Abstract domains, direct and SCC summaries, demand queries, extension quarantine, and Rust provider sink.
- Framework/trust-boundary facts, type/value/place/alias substrate, refined calls, data flow, slicing, evidence bundles, benchmark gates, and promoted SDK/query ergonomics.

### What Worked

- Keeping advanced analysis families private until verified preserved SDK/API discipline while allowing deep internal iteration.
- Native eval fixtures and public no-leak proofs gave a repeatable closure pattern for every new fact family.
- Cache identity work paid down correctness risk early enough for later providers to plug into the same model.

### What Was Inefficient

- The milestone generated many closeout artifacts and legacy quick-task records; the final archive needed a separate reconciliation pass.
- Several phase verification files were missing or stale even though the implementation had passed, which made milestone audit more expensive.
- Nyquist validation coverage lagged behind phase completion for much of the milestone.

### Patterns Established

- New analysis families should carry stable keys, provenance/precision/status metadata, deterministic debug/eval rows, cache inputs, validation, and public no-leak proof from the first implementation.
- Public SDK/query promotion should be treated as a separate, gated decision after internal fixtures and benchmarks prove the contract.
- Setup-missing, unsupported, unknown, ambiguous, and budget-exceeded states should stay explicit rather than being hidden behind overconfident facts.

### Key Lessons

- Do closeout artifact reconciliation before the final milestone audit, not after it fails.
- Keep phase `VERIFICATION.md` files current at execution time; reconstructing them later is possible but wasteful.
- Archive phase directories at milestone close to keep active planning context small for the next cycle.

## Cross-Milestone Trends

| Milestone | Main Pattern | Watch Next |
|-----------|--------------|------------|
| v1.0 | Scope discipline and verification-first closure worked well. | Start v1.1 from fresh requirements instead of extending stale v1 files. |
| v1.2 | Private-first advanced analysis with eval/cache/no-leak gates scaled across many fact families. | Start the next milestone from fresh requirements and decide which validated internals are worth public promotion. |
