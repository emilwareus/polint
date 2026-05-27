---
status: passed
phase: 34-rust-extension-provider-sink
source:
  - 34-01-SUMMARY.md
  - 34-02-SUMMARY.md
  - 34-03-SUMMARY.md
  - 34-04-SUMMARY.md
  - 34-05-SUMMARY.md
  - 34-06-SUMMARY.md
started: 2026-05-23T06:42:56Z
updated: 2026-05-24T05:37:33Z
---

## Tests

### 1. Extension Discovery and Input Snapshot
expected: A repo with `.polint/extensions/<name>/Cargo.toml` is detected deterministically. Its manifest, source digest, dependency digest, and activation status appear in the internal input snapshot as an extension provider component. A repo without extensions still reports the stable absent extension component.
result: [passed]

### 2. Extension Host and Protocol Failure Handling
expected: The extension host runs repo-local Rust extensions through versioned `handshake` and `run-provider` commands using explicit process arguments. Unknown protocol fields, invalid JSON, nonzero exits, and timeouts become controlled `polint/extension` diagnostics instead of crashes.
result: [passed]

### 3. Extension Sink Validation and Metadata
expected: Extension candidate facts are split into accepted and rejected rows before merge. Accepted rows carry `polint.extension.<extension>.<provider>` metadata, precision, confidence, evidence, and payload digest; rejected rows stay audit-only and do not enter normal native fact stores.
result: [passed]

### 4. Kernel Integration and Public No-Leak
expected: `polint.extensions` runs once in the private kernel provider sequence after native summary providers and before metrics, records deterministic provider output information, and normal public CLI/help/SDK/README surfaces do not expose internal protocol or sink marker names.
result: [passed]

### 5. Extension Cache Identity and Quarantine
expected: Extension cache identity includes extension source/dependency/manifest/protocol/options/read/input digests, and extension-influenced layer keys participate in existing quarantine behavior. Native-only cache nodes are not quarantined by absent or unrelated extension digests.
result: [passed]

### 6. Real Extension Eval Fixture
expected: The real `extension/real-sink` eval fixture runs a repo-local extension binary, observes `extension.real_sink_active = true`, records one accepted extension fact and one rejected fact, reports default-vs-extension delta evidence, and passes the extension eval and native fixture coverage tests.
result: [passed]

## Summary

total: 6
passed: 6
issues: 0
pending: 0
skipped: 0
blocked: 0

## Validation Commands

- `cargo test --lib -p polint -- scc direct_summaries large_stdout wire_fact_span extension eval_extension provider_order no_leak --nocapture`
- `cargo test -p polint --test cli -- extension_no_leak --nocapture`
- `cargo clippy -p polint -- -D warnings`

## Gaps

[none]
