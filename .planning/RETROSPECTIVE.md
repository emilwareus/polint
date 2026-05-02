# Retrospective

## Milestone: v1.0 - MVP

**Shipped:** 2026-05-02
**Phases:** 10
**Plans:** 35

### What Was Built

- Rust 2024 workspace and CLI.
- Config, discovery, diagnostics, and core facts.
- Go and TypeScript/JavaScript adapters.
- SDK and example rules.
- Cache/performance support, CI output, and graph commands.
- Experimental plugin skeleton.
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

## Cross-Milestone Trends

| Milestone | Main Pattern | Watch Next |
|-----------|--------------|------------|
| v1.0 | Scope discipline and verification-first closure worked well. | Start v1.1 from fresh requirements instead of extending stale v1 files. |
