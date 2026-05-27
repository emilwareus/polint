---
status: complete
quick_id: 260527-a8t
slug: fix-generated-go-rule-scaffold-heuristic
date: 2026-05-27
---

# Summary

Fixed the final PR review finding for generated Go rules:

- Generated Go `polint new-rule` diagnostics now disclose heuristic behavior in the diagnostic message.
- The Go scaffold regression test now asserts the generated module contains the heuristic wording.

Verification:

- `cargo fmt --all --check`
- `cargo test -p polint --test cli new_rule_go_creates_sdk_oriented_skeleton --locked -- --exact`
- `cargo test -p polint --test cli new_rule_go_generates_fixture_that_test_can_run --locked -- --exact`
