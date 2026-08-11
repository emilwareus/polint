# T-ARCH-DOC LAND

## Result

The landed architecture is now documented in the repository root. This tranche
changes documentation and swarm state only; it does not modify Rust code, public
imports, generated artifacts, or the integration branch's code head.

## Documentation

- `ARCHITECTURE.md` documents the eight-crate graph and ownership boundaries,
  the facade/composition root and supported public API, facts/MIR/contracts,
  stable identity and interning, provider planning/execution, Go and TS/JS
  lifecycle and graph adapters, caching/digests/determinism, errors/precision,
  extension guidance, validation, and enduring non-goals.
- `AGENTS.md` now points its managed architecture section to the root document
  and retains the visibility/public-surface guidance.

The root document describes the implementation currently present. It does not
promote private providers, stores, graph algorithms, or partial capabilities to
the rule-author API, and it does not claim that ship preparation is complete.

## Validation

Documentation/state checks for this tranche:

- `git diff --check` — PASS
- JSON parse and state-transition invariants — PASS
- root architecture pointer and required section checks — PASS
- eight-crate dependency structural checks (`cargo metadata --no-deps`) — PASS
- the prior public-surface acceptance remains recorded in the T-SPLIT gate log
- the 44 pre-existing untracked artifacts remain byte-identical and untracked

The code-parent acceptance gate remains recorded at
`.swarm/gate-logs/T-SPLIT-acceptance-92b4b021.log`. The final tip gate and
`.swarm/READY-TO-SHIP.md` remain T-SHIP-PREP work; they are not asserted here.

## State transition

- `T-ARCH-DOC`: `READY` -> `MERGED` / complete.
- `T-SHIP-PREP`: `BLOCKED` -> `READY`.
- `integration_head` remains the code parent `92b4b021`; this documentation
  commit must not be represented as a new code head.
- This LAND record and the state update do not invent a self-referential land
  SHA. A later bookkeeping sync may record the documentation commit if needed.

## Scope protections

- Branch: `static-analysis-architecture-review`.
- No push, worktree operation, `main` operation, or Rust source change.
- The original 44 untracked paths are preserved exactly; this record is a new
  intentional LAND artifact.
