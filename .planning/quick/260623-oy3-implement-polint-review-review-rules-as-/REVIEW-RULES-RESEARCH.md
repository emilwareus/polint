# Review Rules — Research (narrow): "polint check, but diff-gated"

> The concept is locked, not up for redesign. `polint review` is **`polint check` with the
> identical rule-as-code setup** — review rules are normal `#[polint::rule]` Rust functions
> using the full SDK and analysis engine (symbols, imports, references, callgraph, metrics,
> everything check rules use). **Nothing in TOML.** The *only* difference from `check`: a
> review rule fires only against a **diff to a target branch/commit**. Even "fire when this
> exact file changes" is Rust code.
>
> This doc researches *exactly* how to build that by reusing check's machinery. All
> citations are `file:line` under `crates/polint`.

## 1. How `check` runs rules today (the machinery we reuse verbatim)

A consumer's rules live in a Rust crate at `.polint/rules` whose `main.rs` calls
`polint::runner::run_cli(vec![rule_a(), rule_b(), …])`. Two binaries, joined by a subprocess:

- **Outer `polint`** — `check` (`cli/mod.rs:2079`) → `discover_local_rule_hosts` (`:2528`, loops
  `config.rules.paths`, finds each pack `Cargo.toml`) → `check_local_rule_hosts` (`:2605`) →
  `run_local_rule_host` (`:2703`), which spawns:
  ```
  cargo run --quiet --manifest-path <pack>/Cargo.toml -- check --format json --fail-on none …
  ```
  and parses the child's JSON back via `diagnostics_from_public_json_report` (`:2776`).
- **Inner host** (the pack) — `run_cli` (`runner/mod.rs:123`) → `check` (`:210`) →
  `analyze_and_run` (`:354`): builds the `AnalysisPlan`, runs `AnalysisKernel::run` to produce
  `output.db` (`:376`), then `run_rules_with_capability_support(&output.db, rules, …)` (`:385`)
  executes each rule. Each rule's fact-view params are built from `&AnalysisDb` via
  `FactView::build(db)` (macro: `polint-macros/src/lib.rs:61-68`).

**Load-bearing fact:** every fact view is built from `&AnalysisDb` only; `RuleCtx` holds just
`db: &AnalysisDb` (`core/mod.rs:7146`). So anything a rule reads — including the diff — must be
reachable from `AnalysisDb`.

`polint review` reuses all of this. It is a near-clone of `check_local_rule_hosts` plus two
additions: a **rule designation** (which rules are review rules) and a **diff** that reaches
the rules.

## 2. Designation — `#[polint::rule(kind = "review")]`

Review rules are authored identically to check rules; they're marked at the rule site:

```rust
#[polint::rule(id = "review/…", description = "…", severity = "warn", kind = "review")]
```

Mechanism (small, recommended over a second pack dir or a TOML profile):
- `parse_rule_args` (`polint-macros/src/lib.rs:139`) already parses `name = "value"` args — add a
  `kind` arm (default `"check"`).
- `RuleMeta` (`core/mod.rs:6788`) gains `kind: RuleKind { Check, Review }`, emitted in the
  macro's generated `RuleMeta { … }`. `RuleMeta` already derives `Serialize/Deserialize`, so the
  kind rides the existing `inspect rule` JSON — the **outer** process learns each rule's kind via
  `run_local_rule_host_inspect` (`cli/mod.rs:2784`) without running analysis.
- Filter at execution: `run_rules_with_capability_support` already filters rules
  (`core/mod.rs:7296`); add `if meta.kind != wanted { skip }`. `check` runs `kind=Check`,
  `review` runs `kind=Review`. One inner `--kind` flag carries the selection into the host.

One pack, one `run_cli` vec, review-ness expressed in Rust at the rule. ~1 macro arg + 1 enum +
1 filter line.

## 3. The diff gate — a `ChangedFiles<'_>` fact-view (primary) + a default finding-level gate

Two complementary mechanisms; both serve "only triggers with a diff," both keep rules as code.

### 3a. `ChangedFiles<'_>` — the diff as a fact a rule reads (the core of "as code")
A review rule takes the changeset as a parameter, exactly like `Imports<'_>`/`Symbols<'_>`:

```rust
fn rule(ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>, symbols: Symbols<'_>) -> RuleResult
```

`ChangedFiles` exposes the changed paths + new-side line ranges + status (Added/Modified/Deleted/
Renamed) vs the target ref, with helpers (`iter`, `contains_path`, `matches_glob`, `lines_for`,
`is_added`…). This is how "fire when `db/migrations/**` changes" is **Rust code**, and how a
complex rule restricts real analysis to changed code.

### 3b. Default finding-level gate (so any rule is "check but diff-only" for free)
The outer `review` command, by default, surfaces only diagnostics whose `file` (and optionally
`TextRange` lines, `diagnostics/mod.rs:767,95`) intersect the changeset — a pure post-filter where
`apply_report_filters` runs (`cli/mod.rs:2642`). This makes *any* existing analysis rule "trigger
only on the diff" without the author writing gate code, with an opt-out for rules that
intentionally report off-diff context. No SDK change.

**Together:** `ChangedFiles` gives precise in-code change targeting; the default gate gives
"same as check, but only on the diff" for free. A rule never needs TOML.

## 4. Getting the diff, and the changeset → subprocess handoff

There is **no git code in the crate today** (confirmed). Add a thin `std::process::Command`
shell-out in the `go/lifecycle.rs` style (`go/lifecycle.rs:10`) — **no `git2`/`gix`**.

The diff is produced in the **outer `review` command** (a new `Command::Review`):
- Resolve target: `git merge-base <target-ref> HEAD` (PR semantics), or a raw commit.
- `git diff --name-status <base>` (paths+status) + `git diff --unified=0 <base>` (new-side hunks).
- Normalize to repo-relative `/`-paths to match `Diagnostic.file` / `check_path_pattern`
  (`cli/mod.rs:2507`).

The rules run in the **host subprocess**, which builds its own `AnalysisDb` (`runner/mod.rs:376`) —
the outer process never holds that db. So the changeset must travel into the host:
1. Outer `review` serializes the changeset to a JSON file under the cache dir (reuse the
   `POLINT_CACHE_DIR`/`CARGO_TARGET_DIR` env contract at `cli/mod.rs:2735`).
2. Pass a hidden inner flag `--changed-files <FILE>` (+ `--kind review`) appended in
   `run_local_rule_host` (`cli/mod.rs:2716`).
3. Inner `analyze_and_run` reads it and calls `output.db.set_changeset(parsed)` right after
   `AnalysisKernel::run` (`runner/mod.rs:383`), before rules run. Now `ChangedFiles::build(db)`
   sees it.

A file (not an env var) avoids env-size limits on large diffs; the cache-dir+path pattern is the
established contract.

## 5. Adding the `ChangedFiles` view — the exact lockstep change-list

Same 7-touch pattern every fact family costs; the one novelty is the data is **injected** (set on
the db by the host) rather than **derived by a provider**:

1. **`AnalysisDb`** (`core/mod.rs:656`): store `changeset: Option<ChangeSetFacts>` + `set_changeset(…)`
   (next to `add_file`, `:965`). Name it distinctly from the incremental-cache `ChangeSet`
   (`analysis_kernel/incremental/change_set.rs`).
2. **View type** (`sdk/facts.rs`): `pub struct ChangedFiles<'a> { db: &'a AnalysisDb }` + query
   methods + `impl_fact_view!(ChangedFiles)` (`sdk/facts.rs:899`).
3. **Prelude** (`sdk/mod.rs:47`): export `ChangedFiles`.
4. **Macro capability map** (`polint-macros/src/lib.rs:338`, `capability_for_type`): add
   `"ChangedFiles" => "changeset"`.
5. **`Capabilities`** (`core/mod.rs:6802`): add `changeset: bool` + `.changeset()` builder + a
   `requested_names` row (`:6951`).
6. **Plan support** (`analysis_plan.rs:670`, `support_for`): `"changeset"` → `Supported`.
7. (optional) register in `public_fact_view` (`cli/mod.rs:1745`) for `facts list`/docs.

## 6. Authoring — simple and complex review rules (both ordinary Rust)

**Simple — fire when a path changes:**
```rust
use polint::sdk::prelude::*;

#[polint::rule(id = "review/migrations", description = "Migrations changed — DB owner must review.",
               severity = "warn", kind = "review")]
pub(crate) fn migrations(ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>) -> RuleResult {
    for c in changes.iter() {
        if c.matches_glob("db/migrations/**") {
            ctx.report(Diagnostic::warning(ctx.rule_id(), c.path(),
                DiagnosticRange::point(1, 1), "Migration changed: a DB owner must review."));
        }
    }
    Ok(())
}
```

**Complex — a changed exported symbol that other modules import (full engine + diff):**
```rust
#[polint::rule(id = "review/public-api-change",
    description = "Changed exported symbol that other modules import.", severity = "warn", kind = "review")]
pub(crate) fn public_api_change(
    ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>,
    files: SourceFiles<'_>, symbols: Symbols<'_>, references: References<'_>,
) -> RuleResult {
    for src in files.iter() {
        if !changes.contains_path(&src.relative_path) { continue; }
        for sym in symbols.for_file(src.id).filter(|s| s.is_exported) {
            let used_elsewhere = references.to(sym.id).any(|r| r.file.is_some_and(|f| f != src.id));
            if used_elsewhere {
                if let Some(span) = &sym.primary_span {
                    ctx.warn(span, format!("Exported `{}` changed and is imported by other modules \
                        — review the public-API impact.", sym.name));
                }
            }
        }
    }
    Ok(())
}
```
Both are normal `#[polint::rule]` functions; the only new surface is the `ChangedFiles<'_>`
parameter and `kind = "review"`.

## 7. Complexity read (blunt)

- **~80% is reuse of `check` verbatim:** `Command::Review` ≈ a copy of `check_local_rule_hosts`
  (`cli/mod.rs:2605`); designation 2B is ~1 macro arg + 1 `RuleMeta` field (already `Serialize`) +
  1 filter line, carried to the outer process for free via the existing `inspect rule` JSON; the
  finding-level gate is a `Vec<Diagnostic>` post-filter.
- **~20% is the one new thing:** the `ChangedFiles` fact-view (the first **externally injected**
  fact family — widens `AnalysisDb`'s contract from "kernel-derived" to "host-set") plus the
  outer-`git` → cache-file → host `set_changeset` handoff (because rules run in the subprocess).
  The established env/cache-dir seam already exists; the edge cases are path normalization to match
  `Diagnostic.file`, large diffs, deleted files (no new-side lines), and merge-base resolution.
- **No git library, no TOML rules, no multi-PR sprawl.** It is `check` + a kind flag + a `git`
  shell-out + one injected fact-view.
