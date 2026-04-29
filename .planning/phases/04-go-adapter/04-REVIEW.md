---
phase: 04-go-adapter
reviewed: 2026-04-29T06:03:22Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - crates/polint-cli/tests/cli.rs
  - crates/polint-core/src/lib.rs
  - crates/polint-go/Cargo.toml
  - crates/polint-go/src/lib.rs
  - examples/go-branch-obligations/authorize.go
  - tests/fixtures/go/clean/payment.go
  - tests/fixtures/go/clean/payment_test.go
  - tests/fixtures/go/failing/payment.go
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 04: Code Review Report

**Reviewed:** 2026-04-29T06:03:22Z
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Reviewed the Go adapter, core fact model changes, CLI coverage, Go example, and Go fixtures. `Cargo.lock` was loaded from the mandatory context but excluded from review as a lock file. No security issues were found. The main concerns are correctness bugs in Go branch-obligation extraction that can produce false positives, miss real error paths, or attach facts to the wrong loop.

## Warnings

### WR-01: If Error-Path Classification Marks the Wrong Edge

**File:** `crates/polint-go/src/lib.rs:692`
**Issue:** `push_if_branches` classifies both the true and false edges from the same condition text, and the true edge passes the whole `if_statement` node into `is_go_error_path_heuristic`. For `if err != nil { return err }`, both edges can be marked as error paths because the condition contains `err != nil`. For `if ok { return nil } else { return ErrDenied }`, the true edge can be marked as the error path because the whole `if` subtree includes the `else` return, while the false edge receives `None` for body inspection. Consumers of `BranchObligation.is_error_path` can therefore report missing tests for the wrong branch and skip the branch that actually returns the error.
**Fix:**
```rust
let true_body = node
    .child_by_field_name("consequence")
    .or_else(|| node.child_by_field_name("body"));
let false_body = node.child_by_field_name("alternative");

let true_is_error = branch_body_returns_error(source, true_body)
    || condition_implies_error_edge(&condition, "true");
let false_is_error = branch_body_returns_error(source, false_body)
    || condition_implies_error_edge(&condition, "false");
```
Use branch-specific body nodes and polarity-aware condition matching, for example `err != nil` implies only the true edge while `err == nil` implies only the false edge unless body inspection proves otherwise.

### WR-02: Switch, Case, and Loop Branches Ignore Direct Error Returns

**File:** `crates/polint-go/src/lib.rs:732`
**Issue:** The non-`if` branch helpers call `is_go_error_path_heuristic` with `function_returns_error` hardcoded to `false` at lines 732, 767, 794, and 813. That disables return-body inspection for switch/case/loop branches, so common Go code like `case amount < 0: return ErrInvalid` in a function returning `error` is not marked as an error path unless the case condition text itself contains an error-looking word.
**Fix:**
```rust
fn push_case_branch(
    db: &mut AnalysisDb,
    target: BranchTarget<'_>,
    source: &str,
    node: Node<'_>,
    edge_label: &str,
    function_returns_error: bool,
) {
    // ...
    push_branch(
        db,
        target,
        trimmed_span(target.file, source, start, end),
        condition.clone(),
        edge_label,
        is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
    );
}
```
Thread `function_returns_error` from `extract_branches` into the concrete branch helpers. Avoid applying subtree inspection to aggregate switch nodes if that would mark the whole switch just because one case returns an error.

### WR-03: Outer `for` Loops Can Be Misclassified as Inner Range Loops

**File:** `crates/polint-go/src/lib.rs:779`
**Issue:** `push_for_branch` uses `first_named_descendant` to find a `range_clause`. Because that searches the entire subtree, an ordinary outer loop containing an inner `for range` loop will be recorded as a range branch using the inner loop's condition. The inner loop will also be visited separately, creating incorrect or duplicate branch obligations.
**Fix:**
```rust
let direct_range = node
    .child_by_field_name("clause")
    .filter(|child| child.kind() == "range_clause")
    .or_else(|| first_named_child(node, "range_clause"));
```
Only inspect direct children or the tree-sitter field for the loop clause when deciding whether the current `for_statement` is a range loop.

---

_Reviewed: 2026-04-29T06:03:22Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
