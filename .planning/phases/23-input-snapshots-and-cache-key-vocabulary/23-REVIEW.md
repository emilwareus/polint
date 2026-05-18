---
phase: 23-input-snapshots-and-cache-key-vocabulary
reviewed: 2026-05-18T07:17:16Z
depth: standard
files_reviewed: 28
files_reviewed_list:
  - crates/polint/src/analysis_kernel/incremental/digest.rs
  - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
  - crates/polint/src/analysis_kernel/incremental/run_report.rs
  - crates/polint/src/analysis_kernel/incremental/stats.rs
  - crates/polint/src/analysis_kernel/metadata.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/cache/mod.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/go/adapter.rs
  - crates/polint/src/go/mod.rs
  - crates/polint/src/go/tests.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/src/ts/mod.rs
  - crates/polint/src/ts/tests.rs
  - crates/polint/tests/cli.rs
  - tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml
  - tests/eval-fixtures/cache/input-snapshots/repo/.polint.toml
  - tests/eval-fixtures/cache/input-snapshots/repo/goapp/go.mod
  - tests/eval-fixtures/cache/input-snapshots/repo/goapp/go.sum
  - tests/eval-fixtures/cache/input-snapshots/repo/goapp/payment.go
  - tests/eval-fixtures/cache/input-snapshots/repo/web/package.json
  - tests/eval-fixtures/cache/input-snapshots/repo/web/src/app.ts
  - tests/eval-fixtures/cache/input-snapshots/repo/web/tsconfig.json
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 23: Code Review Report

**Reviewed:** 2026-05-18T07:17:16Z
**Depth:** standard
**Files Reviewed:** 28
**Status:** issues_found

## Summary

Reviewed the internal input snapshot, cache key vocabulary, provider metadata, cache stats, adapter cache reporting, eval fixture observation, CLI leakage tests, and the new cache input-snapshot fixture. The main issue is that lifecycle files which exist but cannot be read are silently omitted from the input snapshot, making an unreadable input indistinguishable from an absent input.

## Warnings

### WR-01: Unreadable Lifecycle Files Are Treated As Absent

**File:** `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs:478`
**Issue:** `file_digest_component` skips any candidate lifecycle file when `fs::read` fails. For inputs such as `go.mod`, `go.sum`, `go.work`, `package.json`, lockfiles, or `tsconfig.json`, a present-but-unreadable file is then omitted from `present_paths`; if no other candidates are readable, the component becomes `Absent` with `"no lifecycle files present"`. That collapses a setup/input error into the same snapshot state as a genuinely missing file, so downstream cache/input identity can miss a real lifecycle change or setup gap.
**Fix:** Preserve unreadable files as an explicit component state instead of continuing silently. Keep details root-relative and avoid embedding absolute paths.

```rust
let mut unreadable_paths = Vec::new();

for relative_path in paths {
    let normalized = normalize_relative_path(&relative_path);
    let path = root.join(&normalized);
    if !path.is_file() {
        continue;
    }

    match fs::read(&path) {
        Ok(contents) => {
            present_paths.push(normalized.clone());
            digest_parts.push(format!("file={normalized}"));
            digest_parts.push(format!("content_hash={}", stable_hash_bytes(&contents)));
        }
        Err(error) => {
            unreadable_paths.push(format!("{normalized}:read_error={}", error.kind()));
        }
    }
}

if !unreadable_paths.is_empty() {
    return InputComponent::setup_missing(name, digest_kind, unreadable_paths);
}
```

---

_Reviewed: 2026-05-18T07:17:16Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
