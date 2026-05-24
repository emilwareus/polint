---
status: complete
created: 2026-05-24
workflow: gsd-quick
---

# Fix Deep Review Entrypoint Issues

## Objective

Fix the deep-review findings around framework entrypoint false positives, Express route-chain handling, Go test over-reporting, and weak eval assertions.

## Tasks

1. [x] Make TS source fallback ignore comments/strings and require known framework receiver variables.
2. [x] Add Express route-chain path extraction coverage.
3. [x] Tighten Go test entrypoint detection and precision.
4. [x] Include handler names in eval observed payloads and assert representative handler/trigger facts.
5. [x] Run focused tests, eval fixture, formatting, clippy.
