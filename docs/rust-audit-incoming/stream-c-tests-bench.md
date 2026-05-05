# Stream C — tests & bench audit

**Reference:** `.agents/skills/rust-best-practices/` — Chapter 5 (Automated Testing), Chapter 4 §4.2 (`unwrap`/`expect` in tests).

**Scope reviewed:** `crates/polint/tests/**/*.rs`, `crates/polint-bench/**/*.rs`, and a light cross-check of `#[cfg(test)]` in `crates/polint/src/` for insta/proptest usage (not exhaustive).

---

## Summary

Integration tests are strong on behavior coverage and failure messages on many `assert!` call sites, but **`cli.rs` is very large**, often packs **several assertions per test**, and uses **parameterized loops** where the handbook would prefer **one focused test (or rstest case) per behavior**. `unwrap`/`expect` in tests are **appropriate** per the handbook; a few **helper-level `unwrap`s on JSON** are the main brittleness risk. The **bench crate** is clear and uses `anyhow` appropriately for a binary; its **embedded unit test** bundles all examples into one loop. **`insta` / `proptest`** appear in **library unit tests**, not under `tests/`.

---

## `crates/polint/tests/`

### Layout and roles

- **`cli.rs`** (~2.1k lines): almost all integration coverage in a single file with shared helpers (`write_file`, `stdout_json`, `diagnostics`, etc.). The handbook’s integration-test layout (`tests/common/`, `tests/mod.rs`) is not used; at this size, extracting **`tests/common/mod.rs`** (or a small `tests/support.rs`) would match the documented pattern and shrink the main test file.
- **`cargo_install_smoke.rs`**: focused smoke test, **`#[ignore]`** with a clear reason and doc comment pointing to CI — aligns well with §5.1 (ignored slow tests).

### Test naming

- **Strengths:** Names are mostly **behavior-oriented** (`check_reports_go_parser_diagnostic_for_invalid_source`, `fatal_config_parse_error_exits_two`) rather than generic `test_foo`.
- **Gaps vs handbook:** Apollo’s style favors **sentence-like names** (`<unit>_should_<behavior>_when_<state>`). Many names omit the conditional clause (e.g. `init_creates_config` reads well but is not fully “sentence” form).
- **Nit:** `test_rules_json_output_is_parseable` repeats the **`test_` prefix**, which §5.1 calls out as a weak pattern (`test_add_happy_path` example). Prefer e.g. `test_rules_emits_parseable_json` or `test_rules_should_emit_parseable_json`.

### Assertion density (§5.1 — one behavior / few assertions)

The handbook recommends **one primary assertion per test** (or **rstest `#[case::…]`** with descriptive names when parameterizing).

**Representative patterns in `cli.rs`:**

| Pattern | Examples | Handbook alignment |
|--------|----------|---------------------|
| Single focused check | Many parser / exit-code tests that locate one diagnostic then assert one property | Good |
| Several unrelated checks in one test | `init_creates_config` (success + two path `assert!`), `new_rule_creates_skeleton` (three path assertions), `add_skill_installs_claude_skill_non_interactively` (stdout predicate + four string `assert!`) | Weaker — failures require more scanning |
| Many assertions in a parameterized loop | `new_rule_rejects_unsafe_rule_names_without_writing_outside_rules_dir`, `checked_in_examples_are_runnable_cli_fixtures` | Acceptable as **integration** breadth, but each iteration bundles **multiple behaviors** (filesystem + stderr + diagnostics). Splitting into **one test per rule name** or **one test per example** (with shared setup) would match “one failure, one story” |

**Parameterized table test:** `checked_in_examples_are_runnable_cli_fixtures` is valuable but dense: per example it asserts README, config, rule file, JSON shape, rule set equality, and per-file diagnostics. Consider **named sub-tests** (e.g. `mod examples { #[test] fn basic_… }`) or **rstest** with `#[case::basic(…)]` so `cargo test` output names the failing example immediately.

### `unwrap` / `expect` and error visibility (§4.2)

- **Test bodies and fixtures:** Widespread `.unwrap()` on `tempfile`, `Command::cargo_bin`, and `fs::write` is **normal and allowed** in tests.
- **Good practice already present:** `stdout_json` uses **`unwrap_or_else` + `panic!` with stdout context** when JSON is invalid — better than a bare `serde_json` unwrap for debugging.
- **Brittle helpers:** `diagnostic_files` does `diagnostic["file"].as_str().unwrap()` — if the schema drops or renames `file`, failures are **panic without a structured message**. Prefer **`expect("diagnostic missing file")`** or `and_then` + `panic!("… {diagnostic:#?}")`.
- **Inconsistent strictness:** `profile_and_severity_override_affect_json_and_exit_code` uses `.find(...).unwrap()` on the iterator; other tests use `.expect("…")`. Prefer **`expect` with message** everywhere a find must succeed.

### Integration vs unit concerns

- Tests correctly treat the **binary as a black box** (`assert_cmd`, predicates) and exercise **public CLI** — matches §5.3 integration-test purpose.
- **Doc tests / living API docs:** not assessed here (outside scope); handbook still recommends `///` examples for public API.

### `insta` / `proptest` under `tests/`

- **Not used** in `crates/polint/tests/`. Dev-dependencies include both; integration tests rely on **JSON pointer / string** checks instead. For **stable CLI JSON / SARIF** shapes, **targeted insta snapshots** (with **named snapshots** and **small scoped values**, per §5.6) could reduce repetitive pointer plumbing — optional, not required.

---

## Optional cross-check: `#[cfg(test)]` in `crates/polint/src/`

- **`proptest`:** Used in e.g. `cache`, `diagnostics`, `core`, `fs` — properties and ordering invariants; aligns with handbook’s emphasis on **clear failure stories** if each proptest focuses on **one property** (e.g. cache stable id vs path).
- **`insta`:** Used heavily in `diagnostics` tests — fits §5.5–5.6 (structural/rendered output). When adding snapshots, prefer **named snapshots** and **avoid huge blobs** (handbook §5.6).

---

## `crates/polint-bench/`

### `src/main.rs` (binary)

- **`fn main() -> anyhow::Result<()>`** — appropriate for a **binary** (§4.4).
- **Documentation** tells users to run **`--release`** — aligns with performance guidance in the skill quick reference.
- Logic is straightforward: discover examples, aggregate timings, print TSV — no test smells.

### `src/lib.rs` (library + tests)

- **`cold_analyze_breakdown`:** Uses `expect` when serializing config to JSON for hashing — acceptable in **internal bench/helper** code paths; failure means an invariant broke.
- **`#[cfg(test)] mod tests`:**
  - **`all_examples_cold_pipeline_succeeds_and_phases_sum`:** Single test **iterates every example** with `unwrap_or_else(|e| panic!(...))` and two assertions (`n > 0`, `!sum.is_zero()`). If one example regresses, the **panic message includes the path** (good), but **cargo test filtering** cannot run “just one example” without code change. Consider **`#[rstest]`** or a **thin wrapper** + one test per example directory, or keep one test but document that **`POLINT_BENCH_LOG`** is the debug switch (already used).
  - **`breakdown_track_wall_clock_on_fixture`:** Single fixture, one inequality — **good** alignment with one-behavior testing; the **2 ms slack** may be flaky on heavily loaded CI — worth monitoring.

### Bench code quality

- **Clear separation** of phases (`PipelineBreakdown`, `LoadSourcesTimings`).
- **Honest aggregate** comment (“sum of per-repo times — use for A/B, not abs latency”) — avoids misreading benchmarks.

---

## Recommendations (prioritized)

1. **Split `tests/cli.rs` helpers** into `tests/common.rs` (or `tests/support/mod.rs`) once you next touch integration tests heavily — improves navigation and matches §5.3 layout sketch.
2. **Reduce assertions per test** where cheap: split `init_creates_config` into “stdout mentions initialized” vs “paths exist”, or keep one test but accept **multiple asserts on one logical outcome** as a documented tradeoff for CLI integration tests.
3. **Replace bare `unwrap` in JSON helpers** (`diagnostic_files`, optional chaining sites) with **`expect`/`panic!` that dump the offending value**.
4. **Rename** `test_rules_json_output_is_parseable` to drop redundant `test_` prefix.
5. **Parameterized example table:** Prefer **rstest cases** or **`mod checked_in_examples { … }`** with **one `#[test]` per example** so failures name the example in the test path.
6. **Bench tests:** Optionally **split per-example** cold-pipeline tests or add **ignored** slow variants if CI noise appears on `breakdown_track_wall_clock_on_fixture`.
7. **Optional:** Introduce **scoped insta snapshots** for a **single golden JSON report** or SARIF fragment to complement pointer-based asserts — only if maintenance cost is acceptable.

---

## Non-goals (this stream)

- Full review of every `#[cfg(test)]` block in `src/`.
- Clippy / `cargo nextest` vs `cargo test --doc` policy (handbook §5.2) — belongs in workspace/CI stream if needed.
