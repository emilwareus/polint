# Stream A — polint internals (`core`, `sdk`, `cache`, `config`, `diagnostics`, `fs`, `graph`, `cli`, `runner`, `lib`, `main`)

Audit basis: [.agents/skills/rust-best-practices/SKILL.md](/Users/emilwareus/Development/exlint/.agents/skills/rust-best-practices/SKILL.md) (Apollo Rust Best Practices Handbook).

## 1) Executive summary

- **`cargo clippy -p polint --all-targets --locked -- -D warnings` currently fails**: `RuleRegistry::register_box` in `crates/polint/src/core/mod.rs` is flagged as **`dead_code` under `--all-targets` (lib tests)**. This blocks a strict Apollo-style lint gate until the method is used, removed, or explicitly suppressed with **`#[expect(dead_code)]`** and rationale (prefer over bare `allow` per skill).
- **Public rule API leans on `anyhow::Result`** (`crate` root `use anyhow::Result` in `core`, re-export in `sdk::prelude`). That matches ergonomics goals but **diverges from the skill’s library guidance** (“`thiserror` for library errors, `anyhow` for binaries only”); downstream rules inherit an `anyhow` boundary.
- **Non-test `expect` in cache hashing**: `config_hash` uses `serde_json::to_string(&config.config).expect(...)` in both **`cli/mod.rs:531`** and **`runner/mod.rs:205`**. Failure is unlikely for in-memory serde of config, but it is still an explicit panic path in production binary code; Apollo prefers **`Result`** propagation or infallible encoding where practical.
- **Threading story is sound**: **`pub trait Rule: Send + Sync`** (`core/mod.rs:661`). Rule execution uses **`catch_unwind`** around metadata and `run` (`run_rules`), aligning with resilience goals and concurrency safety expectations for rayon.
- **`unwrap` / `panic!` elsewhere in scope**: Observed usages are confined to **`#[cfg(test)]` blocks**, intentional panic regression tests (`TestRuleBehavior::Panic` in `core`), or **`unwrap_or`**/`Context`-style helpers — acceptable per skill for tests and scripted failure modes.
- **Lint suppressions**: `#[allow(dead_code)]` appears in **`fs/mod.rs:71`** (documented bench-only accessor) and **`graph/mod.rs:82`–`87`** (placeholder exports). Skill prefers **`#[expect(...)]`** with justification when suppression is unavoidable.
- **Performance smell (non-hot-path but illustrative)**: `ImportGraph::from_db`/`FunctionGraph::from_db` in **`graph/mod.rs`** repeat **`String` clones** when resolving nodes and edges; Apollo’s perf chapter flags redundant clones — worth iterating with **borrowed labels** or a single **`String` → `NodeIndex`** phase if graphs grow.
- **Documentation**: Crate does **not** enable **`#![deny(missing_docs)]`**. **`Rule`/`RuleCtx`/key workflow types** are documented; many **dense public fact types and ID newtypes** in `core` and early `diagnostics` items lack `///`, so rustdoc completeness is uneven.
- **Scoped search**: **`unreachable!`**, stray **`TODO`/`FIXME`**, and **`expect(clippy::…)`** were **not** found in the listed paths (`allow(` only where noted above).

## 2) Findings

| Severity | Topic | Location |
|----------|-------|----------|
| **Must-fix** | Clippy `-D warnings` fails on **`dead_code`** for test-only **`register_box`** | `crates/polint/src/core/mod.rs:~977` (impl `RuleRegistry`, `#[cfg(test)]`) |
| **Should-fix** | Public **`anyhow::Result`** on **`Rule::run`** / **`sdk::prelude`** vs library-oriented error modelling | `crates/polint/src/core/mod.rs:~4`, `:~661`–`664`; `crates/polint/src/sdk/mod.rs:~21` |
| **Should-fix** | Production **`expect`** on serde serialize for **`config_hash`** | `crates/polint/src/cli/mod.rs:~531`; `crates/polint/src/runner/mod.rs:~205` |
| **Should-fix** | **Redundant clones** building petgraph **`String`** nodes/edges | `crates/polint/src/graph/mod.rs:~12`–`31`, `:~50`–`67` |
| **Nice** | **`#[allow(dead_code)]`** instead of **`#[expect(dead_code)]`** + rationale | `crates/polint/src/fs/mod.rs:~71`; `crates/polint/src/graph/mod.rs:~82`, `:~87` |
| **Nice** | Placeholder **`cfg_to_dot`** / **`file_node_label`** kept **`pub`** only to silence churn — clarify via feature flag or tighten visibility | `crates/polint/src/graph/mod.rs:~82`–`90` |
| **Nice** | No **`#![deny(missing_docs)]`**; sparse **`///`** on many **`pub`** fact/ID types | `crates/polint/src/lib.rs`; `crates/polint/src/core/mod.rs` (e.g. `FileId`, `Language`, facts); `crates/polint/src/diagnostics/mod.rs` (e.g. `Severity`, `TextRange`) |
| **Nice** | Duplicated **`config_hash`**/`rule_hash` logic between **`cli`** and **`runner`** (drift hazard) | `crates/polint/src/cli/mod.rs` (~`528`–`559` region); `crates/polint/src/runner/mod.rs` (~`202`–`230`) |

### Command result (requested)

```text
cargo clippy -p polint --all-targets --locked -- -D warnings
→ FAILED: dead_code on RuleRegistry::register_box (crates/polint/src/core/mod.rs:977)
```

(Re-run with `--all-features` in CI/Makefile if you rely on optional code paths — not executed in this audit pass.)

## 3) Gaps vs skill checklist (SKILL.md quick reference)

| Skill expectation | Gap / note |
|-------------------|------------|
| Prefer **`&T`** / avoid **`.clone()`** unless ownership required | **`graph`** builds **`DiGraph<String, …>`** with repeated **`clone()`** on paths/names. |
| **`unwrap`/`expect`** not in production (tests OK) | **`config_hash`** **`expect`** in **`cli`** and **`runner`**; rest of scoped unwraps traced to tests. |
| **`thiserror` library errors**, **`anyhow` binaries only** | **`Rule::run`** and **`sdk`** surface **`anyhow::Result`**; internals mix **`anyhow`** with localized **`thiserror`** (e.g. **`fs`**/**`config`**). Consider a **`polint_sdk::SdkError`** (or similar) if strict alignment matters. |
| **`cargo clippy --all-targets --all-features --locked -- -D warnings`** | **`-D warnings` gate breaks today** (**`dead_code`**); **`--all-features`** not exercised in reported command. |
| Use **`#[expect(clippy::…)]`** over **`#[allow]`** where suppression needed | **`fs`**/**`graph`** use **`#[allow(dead_code)]`**; **`graph`** placeholders could use **`expect`** or **`cfg`**. |
| **`TODO(#issue)`** with linked issue | **None found** in scope — compliant. |
| **`#![deny(missing_docs)]`** for libraries | **Not enabled**; **selective** rustdoc on hot types only. |
| **`Send`/`Sync` on shared rule types** | **`Rule: Send + Sync`** — **met**. |
| Panic discipline | **Intentional panics** in tests + **catch_unwind** at host boundary — **reasonable**; hashing **`expect`** is the main production footgun. |
