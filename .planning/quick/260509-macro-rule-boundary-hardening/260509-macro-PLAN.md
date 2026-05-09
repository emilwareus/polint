# Quick Task 260509-macro: Harden rule macro boundary

**Date:** 2026-05-09
**Mode:** quick local execution

## Goal

Tighten the static capability derivation macro so its compile-time checks match the public rule-authoring contract more closely before final review.

## Tasks

- [x] Validate the first rule parameter is a mutable RuleCtx reference instead of relying on downstream type errors.
- [x] Validate rule functions return RuleResult instead of any non-empty return type.
- [x] Reject non-plain rule functions and discarded `RuleResult<T>` values.
- [x] Require placeholder lifetimes on `RuleCtx` and fact-view parameters.
- [x] Restrict qualified fact-view paths to canonical polint SDK paths, while keeping prelude/unqualified usage ergonomic.
- [x] Add tests for rejected bad ctx/return/fact paths and rerun focused/full verification.
- [x] Fix public rustdoc links after making the rule internals private.
