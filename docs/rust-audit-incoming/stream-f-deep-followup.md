# Stream F — Deep follow-up (cross-cutting search + tooling)

Cross-check of shards A–E with repo search, targeted reads, and `cargo clippy`. Basis: same skill as streams A–E ([`.agents/skills/rust-best-practices/SKILL.md`](../../.agents/skills/rust-best-practices/SKILL.md)).

---

## Findings (deep pass only)

1. **[cross-cutting — Medium]** **`config_hash` / `rule_hash` / `deterministic_rule_options` duplication** — Identical blocks in `crates/polint/src/cli/mod.rs` (~528–581) and `crates/polint/src/runner/mod.rs` (~202–255). Any change to cache-key semantics must be edited twice.

2. **[cross-cutting — Low]** **Clone-heavy graph construction** — `ImportGraph::from_db` and `FunctionGraph::from_db` (`crates/polint/src/graph/mod.rs`) clone path/name strings multiple times per node/edge (`:18–30`, `:60–66`), including duplicate `import.path` for node map vs edge label. Matches Stream A’s perf note; worth revisiting if graphs matter on large repos.

3. **[cross-cutting — Low]** **`#[allow(dead_code)]` on graph placeholders** — `cfg_to_dot` / `file_node_label` (`graph/mod.rs:82–88`) still use `allow` rather than `expect` + rationale (Stream A / skill Ch. 2).

4. **[tooling — Info]** **`cargo clippy --workspace --all-targets --locked -- -D warnings`** — **Passes** (same for `--all-features`). **Diff vs Stream A:** `RuleRegistry` and `register_box` are entirely under `#[cfg(test)]` (`core/mod.rs:~956–984`), so the earlier **`dead_code` on `register_box`** report does **not** reproduce.

5. **[production robustness — Medium]** **`expect` on JSON serialization for cache hashing** — `serde_json::to_string(&config.config).expect("polint config should serialize to JSON")` at **`crates/polint/src/cli/mod.rs:531`** and **`crates/polint/src/runner/mod.rs:205`**. Confirms Stream A; handbook prefers non-panicking or propagated failure.

6. **[production robustness — Medium]** **`unwrap` on parser slot (Go)** — **`crates/polint/src/go/mod.rs:164`** (`slot.as_mut().unwrap()` after `slot.is_none()` branch). Confirms Stream B; invariant is local but still a panic surface in production adapter code.

7. **[production robustness — Info]** **No other `unwrap` / `expect` / `panic!` in non-test `polint` sources** beyond the above — `ts/mod.rs` expects/`unwrap` sit in `#[cfg(test)] mod tests` (from `:3226`); `core` panics are test rules inside `#[cfg(test)] mod tests` (`:~1104`); `sdk/mod.rs:68` is prelude smoke test only.

8. **[examples vs Stream D — Medium]** **Spot-check `RuleOptions::allow` (files)** — `examples/go-import-boundaries/.polint/rules/src/go_import_boundaries.rs:60–70` and `examples/custom-rule-ts/.polint/rules/src/no_product_hex_colors.rs:94–104` implement `file_in_scope` **without** `&& !options.allow.iter().any(|a| a == file)`, while `basic`, `ts-design-tokens`, and `multiple-rules/glob.rs` include it. Matches Stream D’s highest-severity inconsistency; improvement plan should track alignment or explicit “illustrative only” documentation.

---

## Short diff vs streams A–E

| Topic | Streams said | Deep pass |
|-------|----------------|-----------|
| Clippy `-D warnings` / `register_box` | A: fails on `dead_code` | **Passes**; registry fully `cfg(test)` |
| `config_hash` `expect` | A: cli + runner | **Same lines**, confirmed |
| Go `unwrap` | B: ~`parse_go_file` | **`go/mod.rs:164`**, confirmed |
| Graph clones / `allow(dead_code)` | A | **Unchanged** |
| Examples `allow` semantics | D: table | **Spot-checked two crates**, matches D |
| CI Clippy | E: omit `--all-features` | Policy gap remains; **both clippy invocations pass** in this environment |

---

_End of Stream F._
