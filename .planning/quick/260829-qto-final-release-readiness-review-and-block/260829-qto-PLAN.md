---
status: complete
task: final release-readiness review and blocker fixes for machine-global rule-host store
---

# Quick Task 260829-qto Plan

1. Audit the four commits against fingerprint, gate, restore, degradation, behavior, stamp, Windows, and test contracts using `origin/main` as the release base.
2. Fix and regression-test every confirmed CRITICAL/HIGH defect without widening the public API or changing successful rule-host output.
3. Run the requested formatting, Clippy, and focused unit-test gates; write the evidence-backed release verdict to the mandated review path and commit the completed review work without pushing.
