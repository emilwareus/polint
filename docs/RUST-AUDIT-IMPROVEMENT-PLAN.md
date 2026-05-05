# Rust audit — improvement plan

Source: streams [A](rust-audit-incoming/stream-a-polint-internal.md), [B](rust-audit-incoming/stream-b-adapters.md), [C](rust-audit-incoming/stream-c-tests-bench.md), [D](rust-audit-incoming/stream-d-examples.md), [E](rust-audit-incoming/stream-e-workspace-ci.md), [F](rust-audit-incoming/stream-f-deep-followup.md). Standard: [`.agents/skills/rust-best-practices/SKILL.md`](../.agents/skills/rust-best-practices/SKILL.md).

## State of the codebase

Wave 1, Wave 2, and the bulk of Wave 3 (SDK scope helpers, supply-chain CI, graph clone reduction, doc-warnings job, lint policy escalation) are now **implemented** (see [Done](#done-implemented-2026-05-05) below).

Remaining open Wave 3 items are limited to **typed rule errors** (W3-4, breaking change) and **splitting `go`/`ts` modules** (W3-6, large refactor). Both deserve their own design discussion before opening a PR.

## Verification gate

Every PR in this plan must keep the following green:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

## Effort scale

- **S** — under 30 minutes, single file.
- **M** — 30 min to 2 hours, a few files plus tests.
- **L** — half a day or more, design or breaking-change discussion required.

## Severity

- **P0** — breaks a documented standard (panic surface, hidden duplication that risks divergence, `--locked` missing on a publish path). Ship within one focused PR.
- **P1** — measurable hygiene that the handbook expects (CI matrix, lints table, helper extraction).
- **P2** — quality-of-life and architectural cleanup. Optional or deferred.

---

## Done (implemented 2026-05-05)

Verified locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

### Wave 1 ✅

| ID | Delivered |
|----|-----------|
| **W1-1** | **`crates/polint/src/cache/keys.rs`** with `config_hash`, `rule_hash`, `deterministic_rule_options`; **`cli`** / **`runner`** call `crate::cache::keys::*`; **`polint-bench`** uses **`polint::_bench::analysis_keys`** wrappers. |
| **W1-2** | **`deterministic_polint_config`** (infallible) replaces JSON + `expect`; regression tests in **`cache::keys`**. **Note:** `.polint/cache` entries may miss once until repopulated (digest format intentionally changed). |
| **W1-3** | Go **`parse_go_file`** uses **`match`** + **`slot.insert`** — no **`unwrap`** on the parser slot. |
| **W1-4** | **`options.allow`** path clause aligned across path-scoping examples; **`examples/config-denied-literal`** unchanged + documented (**`allow`** is literal-values only there). |

### Wave 2 ✅

| ID | Status |
|----|--------|
| **W2-1** | ✅ CI **`cargo test`** + **`cargo clippy`** include **`--all-features`**. |
| **W2-2** | ✅ **`Makefile`** **`test`** → **`--workspace --all-features --locked`**; ✅ **`cargo publish`** (non dry-run) → **`--locked`**. |
| **W2-3** | ✅ **[`Cargo.toml`](../Cargo.toml)** **`[workspace.lints.rust]`** sets **`unsafe_code = forbid`** + **`unreachable_pub = warn`**; **`[workspace.lints.clippy]`** sets **`dbg_macro = deny`**, **`todo`** / **`unimplemented`** / **`redundant_clone`** / **`needless_collect`** / **`large_enum_variant`** = **`warn`**. Member crates inherit via **`[lints] workspace = true`**. |
| **W2-4** | ✅ **`pretty_assertions`** dropped from **`[workspace.dependencies]`**. |
| **W2-5** | ✅ **`crates/polint/tests/common/mod.rs`** extracted; **`tests/cli.rs`** **`mod common`**. |
| **W2-6** | ✅ Integration test renamed to **`rules_json_stdout_should_be_parseable_json`**. |

### Wave 3 ✅ (delivered 2026-05-05)

| ID | Status |
|----|--------|
| **W3-1** | ✅ Graph stubs use **`#[expect(dead_code, …)]`**. **`LoadSourcesTimings::total`** is now **`#[cfg(any(test, feature = "bench"))]`** (no more **`#[allow(dead_code)]`** annotation). |
| **W3-2** | ✅ **`ImportGraph::from_db`** / **`FunctionGraph::from_db`** allocate one **`String`** per unique node label (was up to 3×); helpers use a single **`ensure_node`** with `BTreeMap<&str, NodeIndex>` borrowing from **`AnalysisDb`**. Edge labels switched from **`String`** to **`()`** (always hidden by **`Config::EdgeNoLabel`**). |
| **W3-3** | ✅ **`polint::sdk::scope::{file_in_scope, file_matches_globs, glob_matches}`** is the canonical helper; re-exported via **`sdk::prelude`**. All 9 example rule files dropped their copy/pasted helpers and **`globset`** dependency. **`config-denied-literal`** uses **`file_matches_globs`** (which intentionally omits the **`allow`** clause). |
| **W3-5** | ✅ **`#![deny(missing_docs)]`** on **`polint::sdk`**; module-level docs on every SDK item. Whole-crate **`deny(missing_docs)`** still deferred for `core`/`diagnostics`. |
| **W3-7** | ✅ CI now runs **`EmbarkStudios/cargo-deny-action@v2 check --all-features --locked`** with [`deny.toml`](../deny.toml) (advisories + licenses + bans + sources). New **`doc`** job runs **`cargo doc --no-deps`** with **`RUSTDOCFLAGS=-D warnings`**. |

### Wave 3 deferred

| ID | Notes |
|----|--------|
| **W3-4** | Library error type at the rule boundary (typed `RuleError` via **`thiserror`**). Breaking change for downstream rule packs; needs minor-version bump and migration note. |
| **W3-6** | Splitting `go/mod.rs` (~1.4k LoC + tests) and `ts/mod.rs` (~3.2k LoC + tests). Pure structural refactor; defer until adapter changes are otherwise needed to avoid pointless merge-conflict churn. |

---

## Wave 1 — Panic-free production paths and dedupe (P0)

Historical spec (now implemented — see **Done** above). Three small fixes plus extraction shipped together so cache-key edits stay single-sourced.

### W1-1 · Extract cache-key helpers into one module

`config_hash`, `rule_hash`, and `deterministic_rule_options` are byte-identical in [`crates/polint/src/cli/mod.rs`](../crates/polint/src/cli/mod.rs) (lines 528–582) and [`crates/polint/src/runner/mod.rs`](../crates/polint/src/runner/mod.rs) (lines 202–256). Any future change to cache-key semantics must be edited twice today.

- **Action:** Move the three functions into `crates/polint/src/cache/mod.rs` (or a new `cache::keys` submodule). Re-export `pub(crate)`. Delete both copies; update call sites to `crate::cache::keys::config_hash(..)`.
- **Acceptance:** `rg "fn config_hash"` returns one definition. `cargo test --workspace --all-features --locked` is green. Cache-key bytes for a fixed config are unchanged (add a snapshot test if not present).
- **Effort:** S–M.
- **Handbook:** Ch. 1, "Borrowing & duplication".

### W1-2 · Eliminate `expect` in cache-key serialization

`config_hash` panics if `serde_json::to_string(&config.config)` ever fails. `PolintConfig` is fully `derive(Serialize)` over plain types, so it cannot fail in practice — but the handbook (Ch. 4 §4.2) prefers making the invariant unrepresentable rather than asserting it.

- **Recommended fix:** Replace JSON serialization with a deterministic, infallible string format mirroring `deterministic_rule_options`. This also removes a hidden dependency on `serde_json` formatting stability for cache keys. Sketch:

```rust
fn config_hash(config: &LoadedConfig) -> String {
    let missing = if config.missing { "missing" } else { "loaded" };
    let serialized = deterministic_polint_config(&config.config);
    crate::cache::stable_hash(&[missing, &serialized])
}
```

  Implement `deterministic_polint_config` next to `deterministic_rule_options`.
- **Alternative (smaller diff):** Keep JSON, but propagate `Result<String>` and fall back to a "cache miss" path if serialization ever fails (no panic).
- **Acceptance:** `rg 'expect.*polint config should serialize'` returns nothing. Cache hits/misses still behave identically for the existing tests; add one regression test that two structurally identical configs produce identical hashes.
- **Effort:** S (alternative) or M (recommended).
- **Files (after W1-1):** `crates/polint/src/cache/mod.rs`.
- **Handbook:** Ch. 4 §4.2.

### W1-3 · Eliminate `unwrap` in Go parser thread-local

[`crates/polint/src/go/mod.rs:164`](../crates/polint/src/go/mod.rs) does `slot.as_mut().unwrap()` immediately after a manual init branch. Use the `Option::insert` form so the unwrap path disappears:

```rust
let parser: &mut Parser = match slot.as_mut() {
    Some(parser) => parser,
    None => {
        let mut p = Parser::new();
        p.set_language(&tree_sitter_go::LANGUAGE.into())?;
        slot.insert(p)
    }
};
```

- **Acceptance:** `rg "slot.as_mut\(\).unwrap\(\)" crates/polint/src/go` returns nothing. `cargo test -p polint` green.
- **Effort:** S.
- **Handbook:** Ch. 4 §4.2.

### W1-4 · Align example `file_in_scope` helpers on `RuleOptions::allow`

Three example crates (`basic`, `ts-design-tokens`, `multiple-rules`) honor all three `RuleOptions` selectors — `files`, `allow_files`, **and** path equality on `allow`. The other example rules use the same helper name but **omit** the `allow` clause, so identical `.polint.toml` snippets behave differently per crate. Stream D and Stream F both flagged this.

- **Action:** In every example helper, mirror the three-clause version from [`examples/basic/.polint/rules/src/no_raw_colors.rs:95–106`](../examples/basic/.polint/rules/src/no_raw_colors.rs):

```rust
fn file_in_scope(options: &RuleOptions, file: &str) -> bool {
    (options.files.is_empty()
        || options.files.iter().any(|pattern| glob_matches(pattern, file)))
        && !options.allow_files.iter().any(|pattern| glob_matches(pattern, file))
        && !options.allow.iter().any(|allowed| allowed == file)
}
```

  Affected files: at least [`examples/go-import-boundaries/.polint/rules/src/go_import_boundaries.rs`](../examples/go-import-boundaries/.polint/rules/src/go_import_boundaries.rs) and [`examples/custom-rule-ts/.polint/rules/src/no_product_hex_colors.rs`](../examples/custom-rule-ts/.polint/rules/src/no_product_hex_colors.rs); audit the rest from Stream D's per-example table.

- **Note:** `examples/config-denied-literal` deliberately uses `allow` for **literal allowlisting**, not file scoping — leave it alone but add a `//` comment in its rule explaining the divergence so readers don't paste the wrong helper.
- **Follow-up to consider:** Move the three-clause helper into the SDK as `polint::sdk::scope::file_in_scope` so future examples inherit it. Tracked as W3-3.
- **Acceptance:** `rg -n "fn file_in_scope" examples/` shows the same three-clause body in every file-scoping helper. Manual smoke: `cargo run -p ... -- check` on the example with an `allow = ["src/skip.go"]` entry actually skips the file.
- **Effort:** M.

---

## Wave 2 — Tooling and CI hygiene (P1)

Independent, low-risk. Can ship in one PR ("ci: enforce --locked, --all-features, and workspace lints").

### W2-1 · Add `--all-features` to `cargo test` in CI

[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) already runs Clippy with `-D warnings` on `--all-targets --locked`, but `cargo test` omits `--all-features` so the `bench` feature path is never test-built in CI.

- **Change:** `cargo test --workspace --all-features --locked` in the `clippy-test-install` job (or a sibling job for the feature matrix).
- **Acceptance:** CI run shows `--all-features` in the test step.
- **Effort:** S.

### W2-2 · `--locked` in `Makefile` and `scripts/publish-crates.sh`

`make test` runs `cargo test --workspace` without `--locked`, defeating the lockfile. The publish script runs the real `cargo publish` without `--locked` either, which is the worst place to allow drift.

- **Change:** Add `--locked` (and optionally `--all-features` for `make test`) in both files.
- **Acceptance:** `rg -n "cargo (test|publish)" Makefile scripts/` shows `--locked` on every non-dry-run invocation.
- **Effort:** S.

### W2-3 · `[workspace.lints]` table

The handbook (Ch. 2) recommends a workspace-level lint table so members inherit the same Clippy posture without duplicating attributes per crate.

- **Change:** Add to root `Cargo.toml`:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unreachable_pub = "warn"
missing_debug_implementations = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
dbg_macro = "warn"
todo = "warn"
```

  Then `[lints]\nworkspace = true` in each member `Cargo.toml`. Start at `warn`; promote to `deny` once existing call sites are clean.
- **Acceptance:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` still green; new lints appear as warnings in `cargo clippy --workspace`.
- **Effort:** M (mostly chasing whatever new warnings appear).

### W2-4 · Drop unused `pretty_assertions` from workspace deps

It's pinned in `[workspace.dependencies]` but never used. Either remove it or wire it into a crate that benefits.

- **Acceptance:** `cargo tree -p pretty_assertions` shows zero in-tree consumers, so the line is removed (or one new consumer appears).
- **Effort:** S.

### W2-5 · Extract `tests/common` from `tests/cli.rs`

`crates/polint/tests/cli.rs` is the single biggest test file. The repeated `write_file`, `stdout_json`, `diagnostics(...)` helpers belong in `tests/common/mod.rs`.

- **Action:** Create `crates/polint/tests/common/mod.rs`, move helpers there, `mod common;` from each integration test that needs them.
- **Acceptance:** `tests/cli.rs` shrinks; `cargo test -p polint --tests` still green.
- **Effort:** M.

### W2-6 · Rename `test_rules_json_output_is_parseable`

Drop the `test_` prefix; matches Stream C's note and the handbook's `should_..._when_...` style.

- **Effort:** S.

---

## Wave 3 — Architecture and quality-of-life (P2)

Schedule independently. None of these block production use.

### W3-1 · `#[expect(dead_code)]` over `#[allow(dead_code)]`

Three sites: `crates/polint/src/graph/mod.rs:82` and `:88`, plus `crates/polint/src/fs/mod.rs:71`. Replace with `#[expect(dead_code, reason = "…")]` so the lint flips back if the placeholder is ever wired up.

- **Effort:** S.

### W3-2 · Reduce graph clones

`ImportGraph::from_db` and `FunctionGraph::from_db` clone `String`s repeatedly in their first pass. For very large repos this is measurable.

- **Approach:** First pass collects `&str` keys with lifetimes tied to the `AnalysisDb`; only allocate when adding to `petgraph`. Or interner-style `HashMap<&str, NodeIndex>`.
- **Acceptance:** Bench with `polint-bench` shows reduced allocations on a representative repo. Behavior unchanged.
- **Effort:** M.

### W3-3 · Promote `file_in_scope` to the SDK

Once W1-4 has aligned every example, fold the helper into `polint::sdk` (e.g. `polint::sdk::scope::file_in_scope`). Examples then become one line and future rule packs cannot drift again.

- **Effort:** M.

### W3-4 · Library error type at the rule boundary

`Rule::run` returns `anyhow::Result<()>`. The handbook recommends `thiserror` for libraries. Decide explicitly: keep `anyhow` for ergonomics, or introduce `polint::sdk::RuleError` (with `#[from] anyhow::Error` for back-compat) and migrate examples.

- **Caveat:** Breaking change for downstream rule packs. Schedule with a minor-version bump and a one-page migration note.
- **Effort:** L.

### W3-5 · `#![deny(missing_docs)]` on the public surface

Phase in: start with `crates/polint/src/sdk/`, then `runner`, then the crate root. Existing `pub fn`s mostly already have docs, so the cost is bounded.

- **Effort:** L (mostly review effort).

### W3-6 · Split `go/mod.rs` and `ts/mod.rs`

Both files exceed 2k lines. Submodules along the existing extraction phases (`parse`, `imports`, `functions`, `cache`, `tests`) would dramatically improve reviewability without changing behavior.

- **Effort:** L.

### W3-7 · Supply-chain CI job

Add `cargo deny check` (or `cargo audit`) and Dependabot config. Skill-aligned hygiene; nothing concrete is broken today.

- **Effort:** S.

---

## Out of scope

- Salsa-based incremental query infrastructure — explicitly deferred per `STACK.md` ("Keep a cache abstraction and ship the hash-based cache first.").
- Full Go semantic analysis (a sidecar) — same reason.
- Replacing `tree-sitter-go` or Oxc — current crates fit the constraints.

## Suggested PR sequence

1. **PR 1 — Wave 1** (W1-1 → W1-2 → W1-3 → W1-4). Single coherent diff: dedupe cache helpers, then patch the panic surfaces in the now-single location, then close the example-helper drift. Ships behavior-preserving fixes.
2. **PR 2 — Wave 2** (W2-1 … W2-6). All CI/Makefile/lints; touches no library code paths.
3. **PR 3 — Wave 3, item by item.** W3-1 and W3-7 are safe to land first; W3-4 and W3-5 deserve their own design discussion before opening a PR.
