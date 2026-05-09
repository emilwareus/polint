---
status: complete
phase: 11-capability-driven-analysis-plan
source: [11-VERIFICATION.md]
started: 2026-05-09T09:10:09Z
updated: 2026-05-09T09:14:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Diagnostic And Human Output Clarity

expected: |
  `polint/capability` diagnostics and `polint explain plan` human output are understandable and actionable for a rule author.
result: pass
evidence: |
  A throwaway local rule pack declaring `Capabilities::new().cfg()` was run with `POLINT_RULES_TOOLCHAIN=1.95`.

  `polint explain plan` human output showed:
  - `local/needs-cfg [warn]: Needs CFG facts. (capabilities: cfg)`
  - `cfg: unsupported (rules: local/needs-cfg) - Capability is reserved for a later phase. See: docs/roadmap/00_ROADMAP.md`

  `polint check --only-rule local/needs-cfg` human output showed:
  - `error[polint/capability]: Rule `local/needs-cfg` requested unsupported capability `cfg`.`
  - `evidence rule: local/needs-cfg`
  - `evidence capability: cfg`
  - `help: Capability `cfg` is not supported in this phase; see docs/roadmap/00_ROADMAP.md.`
  - Exit code `1`.

## Summary

total: 1
passed: 1
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

None.
