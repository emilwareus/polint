---
status: complete
---

# Summary: Static Analysis 2.0 Product Vision

Updated `research/static-analysis-2.0/README.md` with a clearer product vision:
custom Rust rules and agentic review remain the core offer, while the long-term
promise is a semantic layer for AI agents to understand, explore, and improve
large codebases.

Also clarified Q14 in `OPEN-QUESTIONS.md`: no remote package-summary registry
in the initial build, but the local summary format must keep registry-ready
boundaries for package identity, content addressing, provenance, validation, and
trust.

Verification:
- `git diff --check`
- no trailing whitespace in touched files
