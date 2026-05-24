---
status: complete
created: 2026-05-24
workflow: gsd-quick
---

# Fix Phase 35 Review Findings

## Objective

Resolve the deep-review findings for framework entrypoint behavior and tests.

## Tasks

1. [x] Resolve framework entrypoint targets from handler arguments instead of defaulting to the registration caller.
2. [x] Extract route/tool/test/command string metadata from real source call text when MIR argument places do not preserve literal values.
3. [x] Include full entrypoint, trust-boundary, dispatch-edge, and unresolved-framework payloads in the provider output digest.
4. [x] Prevent provider error diagnostics from leaking framework-internal marker strings through public JSON output.
5. [x] Add regression coverage for realistic source-backed recognizer behavior and no-leak failure output.
6. [x] Run targeted tests, clippy, formatting, and the relevant eval fixture.
