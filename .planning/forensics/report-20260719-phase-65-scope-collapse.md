# Forensic Report: Phase 65 Scope Collapse

**Generated:** 2026-07-19
**Problem:** Phase 65 produced an 85,000-line, 154-file pull request whose CI critical path reached one hour and whose review/fix loop continued expanding after the phase was declared complete.

## Executive finding

The SQLite direction was sound, but the delivery unit was not. Phase 65 combined four different projects—kernel identity repair, exact invalidation modeling, durable generation storage, and cross-platform runtime hardening—then used an unbounded review rule that treated every transitive finding as part of the same pull request.

The failure was therefore not “review found many bugs.” The phase was already too large before review, and review then added a second project that was larger than the planned implementation.

## Evidence summary

### Planned phase

- The roadmap combined `STORE-04`, `STORE-05`, `META-01`, and `META-04` in one phase.
- The phase plan contained 19 strictly sequential plans.
- Plans 01–15 changed kernel identities, snapshots, provider keys, query keys, dependency vocabulary, layer metadata, and validation handoffs.
- The relational schema and generation lifecycle did not begin until Plans 16–18.
- At phase completion (`8392cc47`), the branch already contained 59 commits, 119 changed files, 36,456 additions, and 2,551 deletions relative to `origin/main`.

### Post-completion review expansion

- Review continued through eight recorded iterations.
- The historical review reports contain more than forty numbered correctness, security, reliability, and performance findings.
- The post-completion range added 67 commits and changed 95 files, with 50,329 additions and 3,592 deletions relative to the phase-complete commit.
- `go/semantic/process.rs` alone gained 20,505 lines after phase completion.
- `go/semantic/process.rs`, `go/semantic/windows.rs`, `go/semantic/local_fs.rs`, and the anchored repository-filesystem modules were not named in any of the original 19 plans.

### Final pull request and CI

- Final branch scope: 126 commits, 154 files, 85,404 additions, and 4,762 deletions.
- The latest CI workflow used 21 jobs and approximately 4.72 runner-hours.
- The Windows library job reached its 60-minute cap.
- Ordinary Go semantic fixture execution was deliberately limited to one concurrent test, with a 45-minute permit wait.
- The “non-fixture” Windows job still ran heavy evaluation suites outside the `eval::fixtures` module.
- The dedicated “smoke” benchmark compiled a second optimized test profile and performed twelve complete analysis runs.

## Anomalies

### 1. Scope drift — confidence: high

The phase goal was durable metadata mirroring. The implementation expanded into a repository-wide identity migration before the database work, then into Go toolchain closure sealing, process-tree containment, descriptor-anchored filesystem operations, and platform-specific runtime security after completion.

This is not incidental file drift. The largest post-completion files were absent from every plan.

### 2. Research-readiness failure — confidence: high

The research correctly selected SQLite and complete-generation publication, but assigned high confidence to reusing the existing kernel identity vocabulary. It did not first prove that `InputSnapshot`, provider outcomes, layer dependencies, query keys, and validation metadata were complete and canonical enough to persist.

The roadmap then marked Phase 65 with “research flags: none.” Execution showed that the supposedly reusable vocabulary required fifteen plans of prerequisite redesign.

### 3. Plan-size failure — confidence: high

Nineteen sequential plans and more than one hundred anticipated files should have triggered mandatory decomposition. Instead, plan completeness was treated as evidence that one phase was executable.

The first safe delivery boundary should have been a readiness audit, followed by separate identity, generation-lifecycle, metadata-family, and enablement pull requests.

### 4. Unbounded review loop — confidence: high

The review rule was effectively “fix every valid finding and continue until nothing new appears.” Each fix widened the reviewed implementation, which exposed additional transitive surfaces and produced further findings.

Findings that pre-existed the metadata-store feature or belonged to independent Go-runtime and filesystem threat models were implemented in the same branch instead of being recorded as separate work.

### 5. Verification optimized for truth count, not deliverability — confidence: high

Verification celebrated 19 plans, 80 plan truths, and 82 security findings/checks while reporting no need for human verification. It did not gate changed-file count, handwritten line count, number of sequential plans, reviewability, or CI latency.

Artifact completeness therefore hid delivery failure rather than preventing it.

### 6. Test-regression acceptance — confidence: high

The phase introduced global serialization to keep per-fixture wall-clock budgets stable. That decision traded CI throughput for benchmark stability inside the normal correctness suite.

The review fix report itself recorded seven direct-call fixtures taking 286.86 seconds, but this was accepted as passing evidence rather than treated as a CI architecture failure.

### 7. Repeated-edit loop — confidence: high

After phase completion, `go/semantic/process.rs` appeared in 18 commits, `analysis_kernel/mod.rs` in 15, store tests in 13, and store generation code in 12. Repeated fixes in the same trust-boundary files indicate a self-expanding convergence loop rather than bounded remediation.

### 8. Missing-artifact check — no anomaly

All 19 plans and summaries, verification, security, review, and review-fix artifacts exist. The failure was not skipped GSD ceremony; it was that the ceremony had no size or scope brake.

### 9. Interruption state — confidence: medium

The worktree currently contains uncommitted changes in CI, Go process containment, module graph, and symbol graph files. These are later CI-remediation attempts, not the cause of the original scope collapse. They must not be mistaken for a clean restart base.

## Root cause

The research answered “what would a maximally defensive semantic store require?” but not “what is the smallest independently valuable change we can safely review and merge?” The plan converted every missing prerequisite into in-phase work, and the review loop converted every transitive defect into same-PR work.

No mechanism forced a stop when the plan reached 19 parts, the implementation reached 119 files, review crossed into unplanned runtime subsystems, or CI exceeded the product’s feedback-time target.

## Corrective actions

1. Treat the current branch as research evidence, not as a merge candidate.
2. Restart from `origin/main`; do not cherry-pick implementation commits wholesale.
3. Merge only the postmortem, retained design decisions, finding triage, and restart plan.
4. Begin with an identity-readiness audit that changes no production behavior.
5. Limit each implementation PR to one storage invariant or one prerequisite subsystem.
6. Route valid out-of-scope review findings into independently prioritized bug work.
7. Require a human split decision when a plan exceeds three implementation tasks, fifteen files, 2,500 handwritten lines, or a five-minute required-CI critical path.
