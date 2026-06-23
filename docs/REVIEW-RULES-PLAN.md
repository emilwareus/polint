# Review Rules — Implementation Plan

**Goal:** Ship `polint review` = `polint check` with rules authored as Rust code (`#[polint::rule(kind = "review")]`), diff-gated to a target branch/commit via a `ChangedFiles<'_>` fact-view and a default finding-level diff gate. No git library, no TOML rules. Implements `docs/REVIEW-RULES-RESEARCH.md` exactly.

This plan is ordered so **each task compiles and is independently committable**. Anchors below are pinned to real line numbers verified against the working tree on branch `emilwareus/polint-review-rules-research`. Where the research doc's anchors were off, the correction is called out inline as **[ANCHOR FIX]**.

Repo contract reminders (`AGENTS.md`): repo-local rules are external consumers (import `polint::sdk::prelude::*` + `polint::runner::run_cli` only); default to narrowest visibility; `unreachable_pub` is `deny`; `unsafe_code` is `forbid`. New public facts get docs under `docs/facts/`.

---

## Per-task verification (run for EVERY task)

```
cargo build -p polint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test -p polint --test public_surface_leak
# plus the task's own tests, named in each task's **Verify**
```

The leak gate (`cargo test -p polint --test public_surface_leak`) must pass on **every** task, not just T2/T5 — it shells out to build the probe and snapshot-compares the prelude. T1 (no prelude change) must keep it green untouched; T2/T5 update it deliberately.

---

## Task 1 — Rule kind designation (`kind = "check" | "review"`)

No-op until `review` exists: every existing rule defaults to `Check`, `polint check` runs `Check` rules, so behavior is unchanged. Independently committable.

**Files**
- `crates/polint/src/core/mod.rs` — add `RuleKind` enum; add `kind` field to `RuleMeta`; thread a kind filter through rule execution.
- `crates/polint-macros/src/lib.rs` — parse `kind` arg; emit `kind` in the generated `RuleMeta`.
- `crates/polint/src/sdk/mod.rs` — re-export `RuleKind` from `__private` (macro needs the path).
- All `RuleMeta { .. }` literal sites (see list below) — add `kind: RuleKind::Check`.
- `crates/polint/src/runner/mod.rs` — inner `--kind` flag + pre-filter rules by kind before execution.
- `crates/polint/src/cli/mod.rs` — pass `--kind check` to host subprocess in `run_local_rule_host` (`:2716` arg block); the outer `check` path stays Check-only.

**Changes**

1. **`RuleKind` enum** in `core/mod.rs` immediately above `RuleMeta` (`:6786`):
   ```
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum RuleKind { #[default] Check, Review }
   ```
   `Default = Check` and `Serialize/Deserialize` are load-bearing (rides inspect JSON; lets literals/`#[serde(default)]` survive).

2. **`RuleMeta`** (`core/mod.rs:6788-6792`, exactly 3 fields today: `id`, `description`, `severity`): add
   ```
   #[serde(default)]
   pub kind: RuleKind,
   ```
   `#[serde(default)]` lets pre-existing inspect JSON (no `kind`) deserialize as `Check` — protects the outer process when reading an older/host manifest.

3. **Macro `RuleArgs`** (`polint-macros/src/lib.rs:19-23`) — **[ANCHOR FIX]** research said line 139 but the struct is at 19; add field `kind: proc_macro2::TokenStream` (a `RuleKind::Check`/`RuleKind::Review` path token).

4. **Macro `parse_rule_args`** (`:139-177`): default `kind` to the `Check` path; add a `"kind"` match arm (alongside `"id"/"description"/"severity"` at `:157-167`) that maps the string `"check"`→`RuleKind::Check` path, `"review"`→`RuleKind::Review` path, else `syn::Error` ("kind must be `check` or `review`"). Mirror `parse_severity` (`:189`) style. Populate `RuleArgs.kind` in the returned struct (`:169`).

5. **Macro `expand_rule`** generated `RuleMeta { .. }` (`:98-102`): add `kind: ::polint::sdk::__private::#kind_path,` (or splice the token directly). Bind `let kind = rule_args.kind;` next to `let id/description/severity` (`:90-92`).

6. **`sdk/mod.rs __private`** (`:59`): change
   ```
   pub use crate::core::{AnalysisDb, Capabilities, RuleMeta};
   ```
   to also export `RuleKind`. **Not** prelude-exported (RuleMeta is not in the prelude either), so this does NOT touch `ALLOWED_PRELUDE`. Confirm `__private` is `#[doc(hidden)]` (it is, `:57`) so it stays off the supported surface.

7. **Update every `RuleMeta { .. }` literal** to add `kind: RuleKind::Check,` (or `..Default::default()` where the surrounding type allows). Sites (verified):
   - `crates/polint/src/rule_manifest.rs:325, 408, 427`
   - `crates/polint/src/analysis_plan.rs:863`
   - `crates/polint/src/core/mod.rs:7568` (test rule), `:9024`, `:9038`
   - `crates/polint/src/symbol_graph/go.rs:1838`
   - `crates/polint/src/module_graph/go.rs:1914`
   - `crates/polint/src/ts/tests.rs:792`
   - (the macro literal at `polint-macros/src/lib.rs:98` is handled in step 5)
   Prefer adding `..Default::default()` to provider/test literals to avoid future churn; use explicit `kind: RuleKind::Check` only where `..Default` is not in scope.

8. **Inner kind filter (runner)** — **[ANCHOR FIX]** research said "add `if meta.kind != wanted { skip }` inside `run_rules_with_capability_support` (`core:7296`)". DO NOT widen that fn: `pub fn run_rules` (`core/mod.rs:7266`) is intentionally `pub` for `polint::_bench` (comment `:7263-7264`), so changing its arity is a public-surface change. Instead **pre-filter the `rules` slice in the host's `analyze_and_run`** (`runner/mod.rs:354-394`): before building `plan_inputs` (`:367`), partition `rules` by `rule.meta().kind == wanted_kind` (default `Check`), and pass only the matching subset to `RulePlanInputs::collect`, `AnalysisPlan::from_inputs`, and `run_rules_with_capability_support` (`:385`). This keeps capability planning honest (only the selected kind's facts are planned) and leaves the core signature frozen.

9. **Inner `--kind` flag** on `runner::CheckArgs` (`runner/mod.rs:64-92`): add
   ```
   /// Which rule kind to run (internal; set by `polint review`).
   #[arg(long, value_enum, default_value_t = KindArg::Check, hide = true)]
   kind: KindArg,
   ```
   Add `#[derive(Clone, Copy, ValueEnum)] enum KindArg { Check, Review }` (mirror `FormatArg` at `:101`). Map `KindArg → RuleKind` in `analyze_and_run`. `hide = true` matches the `ignore_comments` hidden-flag pattern (`runner/mod.rs:90`).

10. **Outer host invocation**: in `run_local_rule_host` (`cli/mod.rs:2703`), the child arg vec at `:2716-2733` currently passes `check --format json --fail-on none --ignore-comments <bool>`. Append `--kind check` here (the outer `check` always runs Check rules). The `review` command (T4) adds its own call site passing `--kind review`. `run_local_rule_host_inspect` (`:2784`) is unchanged — inspect lists all rules regardless of kind so the outer process can read each rule's `kind`.

**Verify**
- `cargo test -p polint-macros` (macro parse tests at `lib.rs:421-587`; add a `kind`-parsing assertion).
- `cargo test -p polint --test cli inspect_rule_manifest_json_is_stable_for_local_rules` — **flag:** this asserts inspect-rule JSON shape (`cli.rs:1239`). Adding `kind` to `RuleMeta` adds a `"kind":"check"` field to that JSON; update the assertion/snapshot to include it.
- `cargo test -p polint core::` (rule-execution tests at `core/mod.rs:9674+` still pass with default Check).

**Done**
- `#[polint::rule(..., kind = "review")]` parses; omitting `kind` yields `Check`.
- `RuleMeta` carries `kind`, visible in `inspect rule --format json` as `"kind"`.
- `polint check` runs only `Check` rules (Review rules are filtered out in the host); no Review command exists yet, so observable behavior is unchanged.
- Leak gate green, untouched.

---

## Task 2 — `ChangedFiles<'_>` fact-view + `AnalysisDb` changeset storage

Empty-by-default: `AnalysisDb.changeset = None`, view returns nothing → compiles and rules can take the param even before any diff is wired. Independently committable. This is the **leak-gate task** (new public SDK surface).

**Files**
- `crates/polint/src/core/mod.rs` — `ChangeSetFacts` / `ChangedFile` / `ChangeStatus` types; `AnalysisDb.changeset` field; `set_changeset` / `changeset` accessors; manual `Default` impl update.
- `crates/polint/src/sdk/facts.rs` — `ChangedFiles<'a>` view + query methods + `impl_fact_view!`.
- `crates/polint/src/sdk/mod.rs` — prelude export of `ChangedFiles` (+ `ChangeStatus`).
- `crates/polint-macros/src/lib.rs` — `capability_for_type` `"ChangedFiles" => "changeset"`.
- `crates/polint/src/core/mod.rs` — `Capabilities.changeset` bool + `.changeset()` builder + `requested_names` row.
- `crates/polint/src/analysis_plan.rs` — `support_for("changeset") => Supported`.
- `crates/polint/tests/public_surface_leak.rs` — extend `ALLOWED_PRELUDE`, bump count.
- `tests/fixtures/public-surface-leak-probe/src/lib.rs` — add witnesses.
- `docs/facts/changed-files.md` — new fact doc (AGENTS.md requires docs for new public facts).

**Changes**

1. **Data types** in `core/mod.rs` (near the other fact structs; group them together). Name `ChangeSetFacts` distinctly from the incremental-cache `ChangeSet` (`analysis_kernel/incremental/change_set.rs`):
   ```
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum ChangeStatus { Added, Modified, Deleted, Renamed }

   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   pub struct ChangedFile {
       pub path: String,                       // repo-relative, '/'-normalized, matches Diagnostic.file
       pub status: ChangeStatus,
       pub new_line_ranges: Vec<(u32, u32)>,   // inclusive 1-based new-side hunks; empty for Deleted
   }

   #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
   pub struct ChangeSetFacts { pub files: Vec<ChangedFile> }
   ```
   `Debug + Clone` are required because `AnalysisDb` derives them (`core/mod.rs:654`). `Serialize/Deserialize` are required because the changeset travels outer→host as a JSON cache file (T4).

2. **`AnalysisDb` field** (`core/mod.rs:656`, struct body 657-…): add a private field
   ```
   changeset: Option<ChangeSetFacts>,
   ```
   Place it near the other top-level optional stores (e.g. after `semantic: Option<SemanticStore>` at `:705`).

3. **`AnalysisDb` manual `Default`** — **[ANCHOR FIX, not in research]** `AnalysisDb` does NOT derive `Default`; it has a hand-written `impl Default for AnalysisDb` at `core/mod.rs:820-…` (verified: `Self { files: Vec::new(), … }`). Add `changeset: None,` to that literal. Missing this = compile error.

4. **`AnalysisDb` accessors** in `impl AnalysisDb` (`:960`), next to `add_file` (`:965`):
   ```
   pub(crate) fn set_changeset(&mut self, changeset: ChangeSetFacts) { self.changeset = Some(changeset); }
   pub(crate) fn changeset(&self) -> Option<&ChangeSetFacts> { self.changeset.as_ref() }
   ```
   `pub(crate)`: only the host (runner) sets it and only the view reads it. Neither is part of the supported surface, so they stay off the leak gate.

5. **`ChangedFiles<'a>` view** in `sdk/facts.rs` (struct near the others; the `db`-field form, then `impl_fact_view!(ChangedFiles)` in the list at `:899-920`). The macro form `($ty:ident)` requires the field be named `db` (`facts.rs:882-888`):
   ```
   /// Diff-to-target-ref fact view. Empty unless `polint review` injected a changeset.
   #[derive(Clone, Copy)]
   pub struct ChangedFiles<'a> { db: &'a AnalysisDb }
   ```
   Query methods (all read `self.db.changeset()`, returning empty/false when `None`):
   - `pub fn iter(self) -> impl Iterator<Item = ChangedFileRef<'a>>` — over `files`.
   - `pub fn is_empty(self) -> bool`
   - `pub fn contains_path(self, path: &str) -> bool` — exact repo-relative match.
   - `pub fn matches_glob(self, glob: &str) -> bool` — any changed path matches (reuse `globset` or `crate::sdk::scope::glob_matches`).
   - `pub fn lines_for(self, path: &str) -> &'a [(u32, u32)]` — new-side ranges for a path (`&[]` if absent/deleted).
   Per-entry accessor type `ChangedFileRef<'a>` (a thin borrow over `&'a ChangedFile`) with: `path(self) -> &'a str`, `status(self) -> ChangeStatus`, `lines(self) -> &'a [(u32,u32)]`, `matches_glob(self, &str) -> bool`, and `is_added/is_modified/is_deleted/is_renamed(self) -> bool`. (The research example uses `c.matches_glob(..)` and `c.path()` on the iterated item; `ChangedFileRef` provides exactly that.)
   Add `impl_fact_view!(ChangedFiles);` (the no-field-arg form) after `impl_fact_view!(CoverageFacts);` (`:916`).
   `#![deny(missing_docs)]` is on `sdk` (`sdk/mod.rs:9`) → every new `pub` item in `facts.rs`/prelude reachables needs a doc comment.

6. **Prelude export** (`sdk/mod.rs:47-52`): add `ChangedFiles` (and `ChangeStatus`, since `ChangedFileRef::status()` returns it and authors will match on it) to the `pub use crate::sdk::facts::{ … }` group. `ChangeStatus` lives in `core` → also add it to the `pub use crate::core::{ … }` prelude group (`:28-39`). `ChangedFile`/`ChangedFileRef`/`ChangeSetFacts` are reachable through view methods and need NOT be prelude-exported (keep surface minimal); they stay `pub` with doc comments.
   - Decision: export exactly **`ChangedFiles` + `ChangeStatus`** through the prelude. `ChangedFileRef` is returned by `iter()` but, like other ref/iterator item types, does not need a prelude name.

7. **Macro capability map** (`polint-macros/src/lib.rs:338-365`, `capability_for_type`): add arm `"ChangedFiles" => "changeset",`. Add a matching assertion in the macro test `capability_for_type_maps_supported_fact_views` (`:471-510`): `assert_eq!(capability("ChangedFiles<'_>"), "changeset");`.

8. **`Capabilities`** (`core/mod.rs:6802-6843`): add `pub changeset: bool,` field; add builder `pub fn changeset(mut self) -> Self { self.changeset = true; self }` in `impl Capabilities` (`:6845`); add `("changeset", self.changeset),` row to `requested_names` (`:6951-6976`).

9. **`support_for`** (`analysis_plan.rs:670`): add `"changeset"` to the `Supported` arm group (the first match arm at `:672-676`, or its own arm returning `(CapabilitySupportStatus::Supported, None, None, None)` like `:677`). Changeset is always available (host injects it; empty when absent), so it is `Supported`, never blocking.

10. **`public_fact_view` (`cli/mod.rs:1745`) — DEFER (research step 7 is "optional").** Do NOT register `changeset` here. `facts list` / `facts sample` output is a public product contract with inline JSON assertions in `cli.rs`; `changeset` is host-injected and not sampleable from a plain `polint facts sample` run (no diff present), so listing it would be dishonest and would churn those tests. Note this explicitly in the fact doc.

11. **Leak gate** (`crates/polint/tests/public_surface_leak.rs`):
    - Add `"ChangedFiles"` and `"ChangeStatus"` to `ALLOWED_PRELUDE` (`:42-146`): `"ChangedFiles"` in the `crate::sdk::facts` block (after `"Cfg",` at `:121`), `"ChangeStatus"` in the `crate::core` block (alphabetical, near `"CoverageFact"` at `:50`).
    - Bump the count assertion `allowlist_has_no_duplicates_and_expected_count` (`:458-463`) from `97` to `99` (two additions). Update the comment to reference this plan as the sanctioned milestone change.
    - **This is a deliberate, sanctioned API addition** (consumers author review rules with `ChangedFiles`, exactly like existing fact-views). The top-of-file policy (`:22-29`) says sanctioned additions extend `ALLOWED_PRELUDE` in the same PR + add a probe witness. Record the promotion in `docs/API-VISIBILITY-PLAN.md` (append a review-rules entry).

12. **Probe witnesses** (`tests/fixtures/public-surface-leak-probe/src/lib.rs`, `allowlist_witness` mod): add, following the existing `<'static>`/`PhantomData` pattern (e.g. the `_assert_cfg` witness):
    ```
    fn _assert_changedfiles() -> ::core::marker::PhantomData<ChangedFiles<'static>> { ::core::marker::PhantomData }
    fn _assert_changestatus() -> ::core::marker::PhantomData<ChangeStatus> { ::core::marker::PhantomData }
    ```
    These compile only because the names reach `polint::sdk::prelude::*`.

13. **Fact doc** `docs/facts/changed-files.md`: document `ChangedFiles` query methods, the `ChangeStatus` variants, that ranges are new-side 1-based inclusive, deleted files carry no ranges, and that the view is **empty under `polint check`** (only populated by `polint review`). Note it is intentionally absent from `polint facts list`.

**Verify**
- `cargo test -p polint --test public_surface_leak` — all five tests pass with the new allowlist + witnesses + count `99`.
- `cargo test -p polint-macros capability_for_type_maps_supported_fact_views`.
- `cargo test -p polint sdk::` (prelude smoke; optionally add a `ChangedFiles<'_>` param to a smoke rule to prove it builds + reports nothing on an empty db).
- A unit test on `AnalysisDb`: `set_changeset(..)` then a `ChangedFiles::build(&db)` returns the injected files; default db → `is_empty()`.

**Done**
- A rule can declare `changes: ChangedFiles<'_>`; capability `changeset` resolves to `Supported`.
- On a db with no injected changeset, the view is empty (no panic).
- `set_changeset` stores; `changeset()` reads back.
- Leak gate green with the two sanctioned prelude additions + witnesses + count bump; promotion recorded.

---

## Task 3 — git changeset module (`std::process::Command` shell-out)

Pure data-in/data-out, unit-testable in isolation against a throwaway git repo. No wiring into commands yet. Independently committable. **No new crate dependency** — shell out to `git`, in the `go/lifecycle.rs` style (`use std::process::Command;` at `go/lifecycle.rs:10`).

**Files**
- `crates/polint/src/git/mod.rs` — new private module.
- `crates/polint/src/lib.rs` — `mod git;` declaration (crate-private; verify it is NOT `pub mod`).

**Changes**

1. **Module declaration**: add `mod git;` to `crates/polint/src/lib.rs` (keep private; leak gate forbids a `polint::git` public path — `public_surface_leak.rs:388` lists `polint::go`/`polint::ts` as forbidden namespaces; a new `git` module must stay `pub(crate)`/private so it never becomes nameable).

2. **Public-within-crate API** (`pub(crate)`):
   ```
   pub(crate) fn changeset_for_ref(root: &Path, target: &str) -> Result<ChangeSetFacts>
   ```
   Returns the `ChangeSetFacts` from T2. `root` is the repo root; `target` is the user's ref (e.g. `origin/main`, a SHA, or `a...b`).

3. **Resolution**:
   - If `target` contains `...` (three-dot range `a...b`), pass it straight to `git diff` (git computes the merge-base form natively). Otherwise resolve the merge-base: `git merge-base <target> HEAD`, capture stdout SHA. On non-zero exit or empty output → **loud error** with stderr context (`anyhow::bail!`), e.g. "could not resolve merge-base of `{target}` and HEAD — is `{target}` a valid ref?". Mirror the error style of `run_local_rule_host` (`cli/mod.rs:2753-2767`).
   - Run all `git` invocations with `Command::new(git_bin).current_dir(root)`, where `git_bin` honors an optional `POLINT_GIT` env override falling back to `"git"` (parallels `POLINT_CARGO`/`CARGO` at `cli/mod.rs:2709-2711`). Keeps tests hermetic and matches the existing override idiom.

4. **Name+status**: `git diff --name-status -z <base>` (`-z` = NUL-delimited, robust to spaces/renames). Parse each record: status letter (`A/M/D/R###/C###/T`) + path(s). Map `A→Added`, `M/T→Modified`, `D→Deleted`, `R*→Renamed` (use the new path), `C*→Added` (copy → treat new path as Added). Normalize each path: `replace('\\', "/")`, strip any `./` prefix — must match `Diagnostic.file` (which is `SourceFile.relative_path`, normalized at file ingest via `relative.to_string_lossy().replace('\\', "/")` at `fs/mod.rs:190`, mirrored by `check_path_pattern` at `cli/mod.rs:2507-2526`).

5. **New-side line ranges**: `git diff --unified=0 <base>` and parse unified-diff hunk headers `@@ -l,s +L,S @@`. For each file, collect `(L, L+S-1)` for `S>0` (the new-side added/changed lines); `S==0` (pure deletion hunk) contributes no new-side range. Associate hunks with the file from the preceding `+++ b/<path>` header (normalize the same way; `/dev/null` on the `+++` side = deleted file → no ranges). Deleted files end with empty `new_line_ranges` (consistent with the `--name-status` `D` records).

6. **Determinism**: sort `files` by `path` before returning so the serialized cache file (T4) and any diff-gated output are stable.

7. **Edge cases to handle explicitly** (research §7): merge-base failure (loud error, above); deleted files (no new-side lines); renames (new path, `Renamed`); binary files (`git diff` emits `Binary files … differ`, no hunks → file present with empty ranges); empty diff (`files` empty → review surfaces nothing).

**Verify**
- New `#[cfg(test)]` module in `git/mod.rs` using `tempfile` + real `git` (the workspace already uses `tempfile`/`assert_cmd`; the test `init`s a repo, commits a base, mutates files, and asserts `changeset_for_ref` returns expected paths/statuses/ranges). Gate the test on `git` being on PATH (skip with a clear message if absent, matching how Go sidecar tests guard their toolchain). Cases: added file (ranges = whole file), modified file (subset ranges), deleted file (empty ranges, `Deleted`), renamed file (`Renamed`, new path), `a...b` range form, bad ref (`is_err`).
- `cargo test -p polint git::`.

**Done**
- `changeset_for_ref(root, "origin/main")` and `changeset_for_ref(root, "a...b")` return correct `ChangeSetFacts` with `/`-normalized repo-relative paths.
- Bad ref → loud `Err`. Deleted/renamed/binary/empty handled.
- Module is private; leak gate green (no `polint::git` public path).

---

## Task 4 — `polint review` command (outer + inner handoff + finding-level gate)

Ties T1–T3 together end-to-end. After this, `polint review <ref>` works. Independently committable (T1–T3 already compiled).

**Files**
- `crates/polint/src/cli/mod.rs` — `Command::Review` variant + dispatch; `ReviewArgs`; `review()` fn (near-clone of `check_local_rule_hosts`); changeset serialization to cache dir; `--changed-files`/`--kind review` on the host subprocess; finding-level diff gate.
- `crates/polint/src/runner/mod.rs` — inner `--changed-files <FILE>` flag; read+`set_changeset` in `analyze_and_run`.

**Changes**

*Outer (cli/mod.rs):*

1. **`Command` enum** (`:162-188`): add
   ```
   /// Run review-kind rules against a diff to a target branch/commit.
   Review(ReviewArgs),
   ```

2. **Dispatch** (`:552-572`): add `Command::Review(args) => review(std::env::current_dir()?, &args),` (returns `Result<u8>`, like `Check` at `:571`).

3. **`ReviewArgs`** (mirror `CheckArgs` shape — read the outer `CheckArgs` in the `cli` module; it carries `paths/profile/format/color/no_cache/fail_on/only_rule/max_diagnostics/stat/shortstat/baseline/new_only/ignore_comments`). `ReviewArgs` adds:
   ```
   /// Target branch or commit to diff against (e.g. origin/main, a SHA, or a...b).
   #[arg(value_name = "REF")]
   reff: String,
   /// Surface ALL review-rule findings, not just those intersecting the diff.
   #[arg(long)]
   no_diff_gate: bool,
   /// Gate on changed FILES only (ignore line ranges) when diff-gating.
   #[arg(long)]
   whole_file: bool,
   ```
   Reuse `format/color/no_cache/fail_on/only_rule/max_diagnostics/profile/paths` exactly as `CheckArgs`. (Field named `reff` to avoid the `ref` keyword.)

4. **`review()` fn** — copy `check_local_rule_hosts` (`:2605-2701`) as the skeleton, with three deltas:
   - **Require hosts.** `review` only makes sense with local rule packs. Call `discover_local_rule_hosts(&root)` (`:2528`); if empty, bail loudly ("`polint review` requires repo-local rules under `[rules] paths`"). (Unlike `check`, there is no built-in single-process fallback for review.)
   - **Build + serialize the changeset BEFORE running hosts.** `let changeset = crate::git::changeset_for_ref(&root, &args.reff)?;` then write it as JSON to the cache dir: reuse `CacheLayout::for_repo(&root)` (used at `cli/mod.rs:2712`/`2788`) and write `serde_json::to_string(&changeset)` to `<cache_root>/review/changeset-<hash>.json` (hash the JSON via `crate::cache::stable_hash` for a stable name; create the `review/` dir). A **file**, not an env var, avoids env-size limits on large diffs (research §4).
   - **Run each host with the changeset + review kind.** Because `run_local_rule_host` (`:2703`) hardcodes its child args, add a sibling `run_local_rule_host_review(root, manifest, args, changeset_file)` — OR (preferred, less duplication) generalize `run_local_rule_host` to also accept an optional `changed_files: Option<&Path>` and a `RuleKind`, appended in the arg block (`:2716-2733`) as `--kind review --changed-files <file>` when reviewing. Keep the existing `check` call sites passing `None`/`check`. Map a `CheckArgs`-equivalent for the host call (review reuses the same inner `check` subcommand; only `--kind` + `--changed-files` differ).

5. **Finding-level diff gate** (research §3b) — apply AFTER collecting host diagnostics, at the same spot `check_local_rule_hosts` runs `apply_report_filters` (`:2642`). Unless `--no-diff-gate`, retain only diagnostics whose `file` is in the changeset, and (unless `--whole-file`) whose line span intersects a `new_line_ranges` entry for that file. Implement as a pure `Vec<Diagnostic>` post-filter helper `gate_to_changeset(diags, &changeset, whole_file) -> Vec<Diagnostic>`:
   - File match: `changeset.files.iter().any(|f| f.path == diag.file)`. (`Diagnostic.file: String`, verified at `diagnostics/mod.rs:770`.)
   - Line match: `Diagnostic.range` is a `TextRange` (`diagnostics/mod.rs:771` field; `TextRange` at `:96` with fields `start_line, start_col, end_line, end_col`, all `u32`). Intersect `[diag.range.start_line, diag.range.end_line]` with each `(lo, hi)` in `lines_for(path)` (overlap iff `start_line <= hi && lo <= end_line`).
   - `Added`/`Renamed` files with whole-file changes still match by file; `Deleted` files have no new-side lines so line-gating drops their diagnostics (a review rule normally would not fire on a deleted file anyway).
   - Then continue exactly as `check_local_rule_hosts`: `apply_report_filters(.., args.only_rule)`, baseline, render, exit code.

6. **Outer `check` path** stays Check-only — already handled in T1 step 10 (passes `--kind check`).

*Inner (runner/mod.rs):*

7. **`CheckArgs` gains** (`runner/mod.rs:64-92`):
   ```
   /// Internal: path to a JSON changeset injected by `polint review`.
   #[arg(long, value_name = "FILE", hide = true)]
   changed_files: Option<PathBuf>,
   ```
   (The `--kind` flag was added in T1 step 9.)

8. **`analyze_and_run`** (`runner/mod.rs:354-394`): after `AnalysisKernel::run(..)` returns `output` (`:376-383`) and BEFORE `run_rules_with_capability_support` (`:385`), if `args.changed_files` is `Some(path)`: read the file, `serde_json::from_str::<ChangeSetFacts>`, and `output.db.set_changeset(parsed)`. Loud error if the file is missing/malformed (it is polint-internal, so corruption is a bug). This is the injection seam the research describes (§4 step 3): `ChangedFiles::build(&output.db)` now sees the diff.

9. **`CheckArgs` literal construction sites** in `cli/mod.rs` that build the *outer* `CheckArgs` (`collect_ignore_report:2177`, `collect_diagnostics_for_baseline:2414`) are the OUTER struct — unaffected by inner-only flags. Only the host **subprocess arg vector** changes (step 4). Confirm no outer `CheckArgs` field list needs the new inner flags (it does not — the new flags exist only on `runner::CheckArgs`).

**Verify**
- New `assert_cmd` integration test in `crates/polint/tests/cli.rs` (mirror the existing local-rule-host tests, e.g. around `cli.rs:1239`/the `new-rule`+`check --format json` flows): scaffold a temp repo that is a real git repo with a `.polint/rules` pack containing one `kind = "review"` rule, commit a base, change a watched file, then run `polint review <base-sha> --format json` and assert the review diagnostic appears; run on an unrelated change and assert it does NOT (diff gate); run with `--no-diff-gate` and assert off-diff findings reappear.
- Assert `polint check` on the same repo does NOT emit the review rule's diagnostic (kind filter).
- `cargo test -p polint --test cli`.

**Done**
- `polint review origin/main` (and `a...b`, and a SHA) builds the diff, runs only Review-kind rules in the host with the changeset injected, and surfaces only diff-intersecting findings by default.
- `--no-diff-gate` and `--whole-file` behave as specified.
- `polint check` is unchanged (Check rules only, no changeset).
- Leak gate green (no new public surface in T4 — all `pub(crate)`/private).

---

## Task 5 — scaffolding, example pack, tests, docs

Author-facing surface + the realistic example + docs/skill. Independently committable.

**Files**
- `crates/polint/src/cli/mod.rs` — `new-rule` review-kind scaffolding (`rule_module_template` at `:949`, and a flag/positional to request review kind).
- `examples/review-rules/` — new example pack (a real workspace member).
- `Cargo.toml` (workspace) — add `examples/review-rules/.polint/rules` to `members`.
- `README.md` — `polint review` section.
- `.claude/skills/polint/SKILL.md` — review-rules note.
- `crates/polint/tests/cli.rs` — scaffolding test for the review template.

**Changes**

1. **`new-rule` scaffolding** (`cli/mod.rs:949-996`, `rule_module_template(language, rule_name)`): teach it to emit `kind = "review"` and a `ChangedFiles<'_>` param when the user asks for a review rule. Two options — choose the smaller:
   - **(a)** Add an optional `--review` flag to `NewRuleArgs` (`cli/mod.rs:190-196`) threaded into `rule_module_template`. When set: the generated `#[polint::rule(..)]` block (`:984-988`) gains `, kind = "review"`, the signature (`:989`) gains `changes: ChangedFiles<'_>`, and `query_example` becomes a `changes.iter()` glob-match loop calling `ctx.warn`/`ctx.report`. Keep the existing check templates intact for the no-flag path.
   - **(b)** Accept a `language` value of `review` (the positional already varies behavior at `:951-980`); add a `"review" =>` arm producing the review template. Simpler but conflates language with kind.
   Recommend **(a)** (`--review`), since review rules are orthogonal to language. Update `write_rule_fixture_skeleton` (`:838`) minimally — the generated positive/negative fixtures use `polint check`-style `polint-test.toml`; a review rule's diagnostic depends on a diff, so skip the diff-dependent auto-fixture for `--review` and print a hint that review rules are exercised via `polint review`.

2. **`examples/review-rules/`** — a real pack (IS a workspace member, like `examples/custom-rule-go/`, verified). Layout mirrors `examples/custom-rule-go/`:
   - `examples/review-rules/.polint.toml` — `[workspace] include = [...]` covering the sample sources; no profile needed (or a `review`-scoped one).
   - `examples/review-rules/.polint/rules/Cargo.toml` — copy `custom-rule-go`'s (`name = "polint-example-review-rules"`, `version.workspace = true`, `edition.workspace = true`, `publish = false`, `polint = { workspace = true }`, `[lints] workspace = true`).
   - `examples/review-rules/.polint/rules/src/main.rs` — `mod`s both rules + `polint::runner::run_cli(vec![ migrations(), public_api_change() ])`.
   - `examples/review-rules/.polint/rules/src/migrations.rs` — the **simple** review rule (research §6): `kind = "review"`, takes `ChangedFiles<'_>`, fires when a path under e.g. `db/migrations/**` changes (`for c in changes.iter() { if c.matches_glob("db/migrations/**") { ctx.report(Diagnostic::warning(..)) } }`). Use `DiagnosticRange::point(1,1)` (alias confirmed in prelude; `TextRange::point` at `diagnostics/mod.rs:104`).
   - `examples/review-rules/.polint/rules/src/public_api_change.rs` — the **complex** rule (research §6): `kind = "review"`, takes `ChangedFiles<'_>, SourceFiles<'_>, Symbols<'_>, References<'_>`; for each source whose `relative_path` is in the changeset, find exported symbols referenced from OTHER files and `ctx.warn(span, ..)`. Confirm exact `SymbolFact`/`ReferenceFact` field/method names against `core/mod.rs` (the research snippet uses `is_exported`, `primary_span`, `id`, `file`, `Symbols::for_file`, `References::to`; adjust to the real fields/iterators when writing).
   - Sample sources so both rules can fire: a `db/migrations/0001_init.sql` (or a file under a migrations dir matching the glob) and an exported symbol in one file imported by another (Go or TS, consistent with the chosen `[workspace] include`).
   - `examples/review-rules/README.md` — explain it is run with `polint review <ref>`, show the two rules, and note review rules only fire against a diff.

3. **Workspace membership** (`Cargo.toml:2-21`): add `"examples/review-rules/.polint/rules",` to `members` (near the other `examples/*`). This makes `cargo build`/`clippy`/`fmt` cover it (workspace `[lints]` `unreachable_pub = deny` + `unsafe_code = forbid` apply). Do NOT touch the `exclude` for the leak-gate probe (`Cargo.toml:25`).

4. **README.md** `polint review` section: one paragraph + a usage block (`polint review origin/main`), explaining review = check + diff gate, that rules are `#[polint::rule(kind = "review")]` Rust, the `ChangedFiles<'_>` param, the default finding-level gate + `--no-diff-gate`/`--whole-file`, and `--review` on `new-rule`. Keep claims honest (heuristic rules say heuristic).

5. **polint skill** (`.claude/skills/polint/SKILL.md`): add a short "Review rules" subsection mirroring the README (kind arg, `ChangedFiles`, `polint review <ref>`, scaffolding flag). Keep it aligned with the README text.

6. **Scaffolding test** (`crates/polint/tests/cli.rs`): add a test like the existing `new-rule` template tests (`cli.rs:724`/`447`) that runs `new-rule --review <name>` (or `new-rule review <name>`) and asserts the generated module contains `kind = "review"` and `ChangedFiles<'_>`, and that `main.rs` registers it via `run_cli`.

**Verify**
- `cargo build` (whole workspace — compiles the new example member).
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (the example obeys workspace lints).
- `cargo test -p polint --test cli` (scaffolding test).
- Manual/integration: in `examples/review-rules`, a `git`-based `polint review <base>` fires both rules on the seeded diff (can be asserted in the T4 integration test instead, reusing this pack).

**Done**
- `polint new-rule --review <name>` scaffolds a `kind = "review"` rule with a `ChangedFiles<'_>` param.
- `examples/review-rules/` is a workspace member with a simple + complex review rule and a README; it builds and lints clean.
- README.md + polint skill document `polint review`.
- Leak gate green.

---

## Test plan

- **Unit (T2):** `AnalysisDb::set_changeset`/`changeset` round-trip; `ChangedFiles` empty-by-default; `matches_glob`/`lines_for`/`contains_path` over an injected changeset.
- **Unit (T3):** `git/mod.rs` against a `tempfile` git repo — added/modified/deleted/renamed/`a...b`/bad-ref/binary/empty; path normalization equals `Diagnostic.file` form; new-side ranges correct; deterministic ordering. Skips cleanly if `git` absent.
- **Macro (T1, T2):** `parse_rule_args` accepts `kind = "review"`/`"check"`, rejects junk; `capability_for_type("ChangedFiles<'_>") == "changeset"`.
- **Integration (T4, `assert_cmd`):** real git repo + review pack → `polint review <base> --format json` fires; unrelated change → gated out; `--no-diff-gate` → reappears; `polint check` → review rule absent (kind filter). Mirror existing `cli.rs` local-rule-host tests.
- **Leak gate (every task):** `public_surface_leak` green; T2 updates allowlist (+`ChangedFiles`,+`ChangeStatus`), count `97→99`, two probe witnesses, promotion recorded in `docs/API-VISIBILITY-PLAN.md`.
- **Full suite (after T2 and after T5):** `cargo test -p polint` — catches any provider-order / inspect-JSON / capability-list snapshot drift (see Risks).
- **Format/lint (every task):** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`.

## Risks

- **Leak-gate snapshot (T2) — highest-touch.** `ALLOWED_PRELUDE` is FROZEN for v1.3 with a hard `len() == 97` assertion and a per-identifier probe witness. Adding `ChangedFiles` + `ChangeStatus` to the prelude REQUIRES, in the same commit: (1) extend `ALLOWED_PRELUDE`, (2) bump the count to `99`, (3) add two `_assert_*` witnesses to the probe, (4) record the promotion in `docs/API-VISIBILITY-PLAN.md`. Missing any one trips the gate. This is sanctioned (new fact-view authoring surface), not a leak. `RuleKind` is NOT prelude-exported (rides `__private` only, like `RuleMeta`) so it does not touch the gate.
- **`RuleMeta` widening churn (T1).** `RuleMeta` has ~9 struct-literal sites across `rule_manifest.rs`, `analysis_plan.rs`, `core/mod.rs`, `symbol_graph/go.rs`, `module_graph/go.rs`, `ts/tests.rs`, plus the macro. `RuleKind: Default + #[serde(default)]` keeps deserialization back-compatible, but every literal must add `kind` (or `..Default::default()`). Build will catch omissions; enumerate from the grep list in T1 step 7.
- **Do NOT widen `pub fn run_rules` (T1).** It is intentionally `pub` for `polint::_bench` (`core/mod.rs:7263-7264`); changing its signature is a public-surface change. The kind filter is done as a **pre-filter on the rules slice in the host's `analyze_and_run`**, leaving the core fn frozen — leak-gate-safe.
- **Manual `AnalysisDb::Default` (T2).** `AnalysisDb` hand-writes `impl Default` (`core/mod.rs:820`), it does not derive it. The new `changeset` field must be initialized there too, not only in the struct decl — otherwise a compile error.
- **Provider-order snapshots (memory: ~7–11 sites).** Adding a kernel *provider* historically touches many provider-order/determinism snapshot assertions. `changeset` is **host-injected, not a kernel provider** (no entry in `analysis/*/provider.rs`, no `provider_manifests` slot), so provider-order snapshots should be UNCHANGED. Still: after T2 and T5, run the full `cargo test -p polint` and update only snapshots that genuinely moved. If any provider-order snapshot changes, that is a signal something was wired as a provider by mistake — revisit.
- **Capability-list / facts-list surface.** `support_for`/`requested_names`/`Capabilities` gain a `changeset` row, which could surface in capability-enumerating tests (`explain`, capability planning). Keeping `changeset` OUT of `public_fact_view` (T2 step 10) avoids churning the `facts list`/`facts sample` public JSON contract. Re-run `inspect_rule_manifest_json_is_stable_for_local_rules` (`cli.rs:1239`, T1) — the `kind` field additively changes inspect JSON; update that assertion.
- **Path normalization (the correctness keystone).** The diff gate matches `ChangedFile.path` against `Diagnostic.file`. `Diagnostic.file` is `SourceFile.relative_path`, normalized `\`→`/` at file ingest (`fs/mod.rs:190`), mirrored by `check_path_pattern` (`cli/mod.rs:2507`). T3 MUST emit identically normalized repo-relative paths (strip `./`, `\`→`/`, repo-root-relative — `git diff` already gives repo-relative). A mismatch silently drops all findings. Cover with a T3 test asserting the exact string form and a T4 integration test that proves a real diagnostic survives the gate.
- **Injected-fact novelty (`AnalysisDb` contract).** `ChangedFiles` is the first *externally injected* fact family — it widens `AnalysisDb` from "kernel-derived only" to "host-set". The injection happens in the host's `analyze_and_run` AFTER `AnalysisKernel::run`, BEFORE rules (T4 step 8), so the kernel/cache identity is unaffected. The changeset MUST NOT participate in any cache-key computation (or every diff would bust the cache) — `set_changeset` is called post-kernel and is excluded from all digests by construction. Confirm no cache-key path reads `changeset`.
- **`git` availability in CI/tests.** T3/T4 tests need `git` on PATH. Guard them to skip-with-message when absent (as the Go sidecar tests guard their toolchain) so non-git environments do not hard-fail.
- **`ref` keyword.** `ReviewArgs` target field must not be literally named `ref` (reserved) — use `reff` with `#[arg(value_name = "REF")]`.
