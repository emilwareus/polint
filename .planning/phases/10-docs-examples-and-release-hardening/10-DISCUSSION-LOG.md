# Phase 10: Docs, Examples, and Release Hardening - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-01
**Phase:** 10-docs-examples-and-release-hardening
**Areas discussed:** README and user path, Examples directory, Fixtures and tests, Release hardening

---

## README and user path

| Option | Description | Selected |
|--------|-------------|----------|
| Complete README as primary v1 docs | Keep one concise user-facing README with quickstart, config, SDK, testing, CI, examples, roadmap, and honesty notes. | ✓ |
| Split into docs site | Add a larger documentation hierarchy. | |
| Minimal polish only | Leave most explanation to existing snippets. | |

**User's choice:** `[auto]` Complete README as primary v1 docs.
**Notes:** Recommended because Phase 10 success criteria and `FND-03` explicitly name README coverage.

---

## Examples directory

| Option | Description | Selected |
|--------|-------------|----------|
| Expand existing `examples/` layout | Keep current directories and add missing README/config/source proof. | ✓ |
| Move examples into crates | Treat examples as crate-internal tests only. | |
| Add a broad demo app | Create a larger synthetic application. | |

**User's choice:** `[auto]` Expand existing `examples/` layout.
**Notes:** Recommended because the roadmap names `examples/` specifically and the repo already has the required directory shape.

---

## Fixtures and tests

| Option | Description | Selected |
|--------|-------------|----------|
| Add focused CLI fixture/example smoke tests | Reuse `assert_cmd`, `tempfile`, parsed JSON, and existing fixtures. | ✓ |
| Add large new fixture suite | Create broad fixture trees for many scenarios. | |
| Docs-only verification | Trust docs without executable proof. | |

**User's choice:** `[auto]` Add focused CLI fixture/example smoke tests.
**Notes:** Recommended because mixed fixtures exist but need explicit integration proof, and Phase 10 success criteria require Go, TS, and mixed repositories.

---

## Release hardening

| Option | Description | Selected |
|--------|-------------|----------|
| Honest docs plus standard Rust verification | Close with fmt, clippy, workspace tests, and documented future work. | ✓ |
| Add publishing automation | Add release tagging/crates.io/package infrastructure. | |
| Add placeholder future features | Create TODO-only stubs for future roadmap items. | |

**User's choice:** `[auto]` Honest docs plus standard Rust verification.
**Notes:** Recommended because the project constraint is to avoid fake functionality and prefer a smaller working v1.

---

## the agent's Discretion

- Exact README ordering and wording.
- Whether examples need `.polint.toml`, README updates, source files, or focused tests.
- Exact test names and placement for mixed fixtures and example smoke coverage.

## Deferred Ideas

- crates.io publication, release tags, binary packaging, and generated changelogs.
- Automatic repo-local Rust/Wasm rule compilation and artifact caching.
- Exact Go semantic sidecar, dynamic branch coverage, and additional language adapters.
