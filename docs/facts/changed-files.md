# Changed-File Facts

`ChangedFiles<'_>` is the diff-to-target-ref fact view used by **review rules**.
A rule requests it on a `#[polint::rule(..., kind = "review")]` function exactly
like any other typed fact view; the same parameter is how polint derives the
`changeset` capability.

The view is **empty under `polint check`**. It is populated only by
`polint review <ref>`, which computes a diff between `HEAD` and the merge-base
with the target ref (or the explicit `a...b` range) and injects it into the
analysis database before rules run. This is how "fire only when this path
changed" or "restrict real analysis to changed code" is expressed as ordinary
Rust instead of TOML.

## `ChangedFiles<'_>` query methods

| Method | Returns | Meaning |
|--------|---------|---------|
| `iter()` | `impl Iterator<Item = ChangedFileRef<'_>>` | Changed files, in deterministic path-sorted order. |
| `is_empty()` | `bool` | `true` when no files changed (always `true` under `polint check`). |
| `contains_path(path)` | `bool` | Exact repo-relative, `/`-normalized path match. |
| `matches_glob(glob)` | `bool` | `true` when any changed path matches the glob. |
| `lines_for(path)` | `&[(u32, u32)]` | New-side changed line ranges for `path` (`&[]` if absent or deleted). |

## `ChangedFileRef<'_>` (one changed entry)

| Method | Returns | Meaning |
|--------|---------|---------|
| `path()` | `&str` | Repo-relative, `/`-normalized path, identical in form to `Diagnostic.file`. |
| `status()` | `ChangeStatus` | How the file changed. |
| `lines()` | `&[(u32, u32)]` | New-side changed line ranges (inclusive, 1-based); empty when deleted. |
| `matches_glob(glob)` | `bool` | Whether this file's path matches the glob. |
| `is_added()` / `is_modified()` / `is_deleted()` / `is_renamed()` | `bool` | Status predicates. |

## `ChangeStatus`

| Variant | Meaning |
|---------|---------|
| `Added` | New file on the working side (also used for copies). |
| `Modified` | Existed on both sides; content changed (also type-changes). |
| `Deleted` | Removed on the working side. Carries no new-side line ranges. |
| `Renamed` | Renamed; the carried `path()` is the **new-side** path. |

## Limits

- **Line ranges are new-side, 1-based, inclusive.** A pure-deletion hunk
  contributes no new-side range. Deleted files carry an empty range list.
- **Binary files** appear as changed entries with empty line ranges (git emits
  no textual hunks for them).
- **Path form is load-bearing.** Changed paths are repo-relative and
  `/`-normalized so they compare equal to `Diagnostic.file`
  (`SourceFile.relative_path`). The default finding-level diff gate in
  `polint review` matches diagnostics against these paths and line ranges, so a
  normalization mismatch would silently drop findings.
- The view is **intentionally absent from `polint facts list` / `polint facts
  sample`**. The changeset is host-injected and cannot be sampled from a plain
  `polint facts sample` run (no diff is present there), so listing it would be
  dishonest. It is documented here instead.

## Small Rule Shape

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "review/migrations",
    description = "Migrations changed — a DB owner must review.",
    severity = "warn",
    kind = "review"
)]
fn migrations(ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    for changed in changes.iter() {
        if changed.matches_glob("db/migrations/**") {
            let line = changed.lines().first().map(|&(lo, _)| lo).unwrap_or(1);
            ctx.report(Diagnostic::warning(
                rule_id.clone(),
                changed.path().to_string(),
                DiagnosticRange::point(line, 1),
                "Migration changed: a DB owner must review.",
            ));
        }
    }
    Ok(())
}
```
