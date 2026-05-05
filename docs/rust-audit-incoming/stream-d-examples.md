# Stream D — examples / rule packs (Rust audit)

Aggregated review of every `examples/**/*.polint/rules/**/*.rs` crate against **polint SDK** usage (`crates/polint/src/sdk/mod.rs`) and **Rust best practices** (`.agents/skills/rust-best-practices/SKILL.md`, with Apollo handbook chapters as reference).

**Scope:** 11 example rule-pack crates, 24 `.rs` files. **Clippy:** `cargo clippy -p polint-example-* --all-targets -- -D warnings` passes for all listed workspace members.

---

## By theme

### 1. SDK usage (generally correct)

- **Prelude:** Rule implementations consistently use `use polint::sdk::prelude::*;` and return `Result<()>` (anyhow), matching the documented authoring surface.
- **Entrypoints:** All `main.rs` binaries use `polint::runner::run_cli(...)` with `std::sync::Arc` and `std::process::ExitCode` — the expected shape.
- **No `polint::core` leakage:** Examples do not reach around the SDK prelude into `core` (appropriate for copy-pastable rule packs).
- **Narrow import exception:** `multiple-rules/.../glob.rs` imports only `RuleOptions` from the prelude — good minimal surface for a shared helper module.

### 2. Panics / `unwrap` / `expect`

- **No** `unwrap()`, `expect()`, or `panic!` in example rule code paths.
- Uses of `unwrap_or` / `unwrap_or_else` are limited to **option fallbacks** (e.g. `ctx.options().max`, glob fallback) — acceptable and idiomatic here.

### 3. Duplicated patterns (maintenance / teaching cost)

Almost every standalone example embeds the same trio:

- `glob_matches` + `build_one` (globset) + `file_in_scope`

**Only** `examples/multiple-rules/.polint/rules/src/glob.rs` factors this out for reuse within that crate. The rest copy the same ~15–20 lines into each rule file. That matches “one self-contained example” teaching, but it **drifts** when one copy changes (see `RuleOptions::allow` below).

**Near-duplicate rule logic:** `no_raw_colors`–style scanning appears in `basic`, `ts-design-tokens`, `custom-rule-ts`, and `multiple-rules` (TS color/token rules). `ts-design-tokens` is the richer variant (extra color forms, overlap dedupe); the others are structurally the same loop over string literals + JSX attributes.

### 4. `RuleOptions` semantics — `file_in_scope` inconsistency

`RuleOptions` includes `files`, `allow_files`, and `allow` (among other fields). Examples disagree on whether **`allow` applies to file paths** when deciding scope:

| Example / module | Honors `options.allow` (per-file path list) in `file_in_scope`? |
|------------------|-------------------------------------------------------------------|
| `basic/no_raw_colors.rs` | Yes |
| `ts-design-tokens/no_raw_colors.rs` | Yes |
| `multiple-rules/glob.rs` (shared) | Yes |
| `custom-rule-ts`, `go-*`, `ts-complexity`, `config-denied-literal` (file scope helper) | **No** |

So the same TOML field can suppress a file for some example rules but not others. That is easy to misread as “polint handles allow globally” when it is purely rule-implemented today. **Recommendation:** either document that examples are illustrative only, or align all `file_in_scope` helpers with the same three clauses (`files`, `allow_files`, **`allow`**).

**Note:** `config-denied-literal` uses `options.allow` for **literal text** allowlisting (`literal_allowed`), not file paths — that is a separate, correct use of the field.

### 5. Error handling / types

- Rules use **`anyhow::Result`** via the prelude — aligned with SDK docs (“ordinary rule implementations”).
- No `thiserror` in example packs (appropriate: rules are leaf binaries / small crates, not published libraries).

### 6. Clippy and style

- Workspace **edition 2024** and `edition.workspace = true` on all example `Cargo.toml` files — aligned with root workspace.
- Example `Cargo.toml` pattern is consistent: `version.workspace`, `edition.workspace`, `publish = false`, `polint.workspace = true`, and `globset.workspace = true` where needed.
- No Clippy `-D warnings` findings on the audited example packages.

Minor style notes (not failing Clippy today):

- **`rule_id.clone()`** in hot loops: idiomatic given `Diagnostic` APIs want owned `String` for the rule id; could reduce churn with a single `let id = self.meta().id.clone()` or shared `&str` → `to_string` only when building diagnostics if APIs ever accept `impl Into<String>`.
- **`is_raw_color`:** `to_ascii_lowercase()` allocates a `String` per call; acceptable for examples, but handbook performance guidance would suggest ASCII-only checks or `as_bytes()` for hex detection where possible.
- **`glob_matches`:** `format!("./{value}")` allocates per match attempt; again fine for demos, but a shared cached/normalized path could avoid churn in large repos.

### 7. API / behavior nits

- **Import boundaries:** `forbidden_imports` matching uses `glob_matches` plus `import.path.contains(pattern)` fallback — duplicated consistently between `go-import-boundaries` and `multiple-rules/go_import_boundaries.rs`.
- **`config-denied-literal`:** `RuleMeta` description mentions “regex” but matching is **substring** (`literal.value.contains(deny)`). Wording and behavior are slightly out of sync.
- **Branch-test examples:** `custom-rule-go/require_error_branch_tests` (empty related tests) vs `go-branch-obligations` (fuzzy evidence matching) show two policies; both state heuristic limits in diagnostics — good honesty with AGENTS “truthfulness” intent.

---

## By example (short)

| Example crate | Files | Notes |
|---------------|-------|--------|
| **basic** | `main.rs`, `no_raw_colors.rs` | Canonical prelude + runner; full `file_in_scope` including `allow`. |
| **ts-design-tokens** | same layout | Stronger color detection + dedupe; full `file_in_scope`. |
| **multiple-rules** | `main`, `glob`, `go_import_boundaries`, `no_raw_colors` | Only example with shared `glob` module; import + color rules. |
| **go-import-boundaries** | `main`, `go_import_boundaries.rs` | `file_in_scope` omits `options.allow` (file list). |
| **go-complexity** | `main`, `go_complexity.rs` | Default `max` via `unwrap_or(12)`; scope helper without `allow`. |
| **ts-complexity** | `main`, `ts_complexity.rs` | Same pattern as go-complexity; TS filter. |
| **go-test-quality** | `main`, `go_test_quality.rs` | Heuristic scoring; `unwrap_or(24)`; scope without `allow`. |
| **go-branch-obligations** | `main`, `go_branch_obligations.rs` | Branch facts + evidence terms; scope without `allow`. |
| **custom-rule-go** | `main`, `require_error_branch_tests.rs` | Simpler heuristic than branch-obligations; scope without `allow`. |
| **custom-rule-ts** | `main`, `no_product_hex_colors.rs` | Same structural pattern as basic colors; scope without `allow`. |
| **config-denied-literal** | `main`, `no_denied_literals.rs` | Deny list + literal `allow`; file scope without path `allow`. |

---

## Summary

Example rule code **matches the intended SDK entrypoints** (prelude + `run_cli`), **passes strict Clippy** on current workspace settings, and **avoids panicking** helpers. The main gaps are **copy-pasted glob/scope helpers** that have **diverged on `RuleOptions::allow` for files**, and small **doc/behavior** mismatches (denied “regex” wording). Tightening `file_in_scope` consistency and optionally centralizing glob helpers (or documenting why examples stay duplicated) would make the set easier to trust as reference implementations.
