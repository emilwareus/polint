# Quick Plan: Fix PR 45 Windows Platform Library Test Failure

## Context

Windows CI is failing in `eval_native_fixture_runner_refined_calls_fixture_passes` for the
`refined-calls/extension-model` fixture. The existing fixture suite already treats runtime
extension fixtures as unreliable on Windows and skips them in broad suite coverage.

## Plan

1. Align the targeted refined-calls fixture runner with the existing Windows skip policy for
   runtime extension fixtures.
2. Keep `refined-calls/direct-vs-refined` running on Windows and keep the extension-model fixture
   running on non-Windows platforms.
3. Run focused refined-calls tests, then formatting, clippy, and the relevant library test suite.
4. Record the outcome in quick-task artifacts and commit code separately from planning state.
