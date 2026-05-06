# Rust best-practices audit (living document)

Audit of the polint Rust workspace against [`.agents/skills/rust-best-practices/SKILL.md`](../.agents/skills/rust-best-practices/SKILL.md) (Apollo handbook).

## TL;DR

**Update (2026-05-06):** Wave **1**, Wave **2**, and Wave **3** (including **W3-4** typed `RuleResult` / `RuleError` and **W3-6** `go`/`ts` adapter vs test split) from [`RUST-AUDIT-IMPROVEMENT-PLAN.md`](RUST-AUDIT-IMPROVEMENT-PLAN.md) are implemented; re-run the verification gate before each release.

Further adapter breakdown (`parse` / `imports` / … submodules under `go`/`ts`) remains optional polish, not tracked as an open audit item.

## Stream reports

| Stream | Scope | File |
|--------|-------|------|
| A | `crates/polint/src/{core,sdk,cache,config,diagnostics,fs,graph,cli,runner,main,lib}` | [stream-a-polint-internal.md](rust-audit-incoming/stream-a-polint-internal.md) |
| B | `crates/polint/src/{go,ts}` | [stream-b-adapters.md](rust-audit-incoming/stream-b-adapters.md) |
| C | `crates/polint/tests/`, `crates/polint-bench/` | [stream-c-tests-bench.md](rust-audit-incoming/stream-c-tests-bench.md) |
| D | `examples/**/.polint/rules/**/*.rs` | [stream-d-examples.md](rust-audit-incoming/stream-d-examples.md) |
| E | Root `Cargo.toml`, member manifests, `.github/workflows/`, `.cargo/` | [stream-e-workspace-ci.md](rust-audit-incoming/stream-e-workspace-ci.md) |
| F | Cross-cutting deep pass + tooling re-run vs A–E | [stream-f-deep-followup.md](rust-audit-incoming/stream-f-deep-followup.md) |

Notable cross-stream correction: Stream A's `register_box` `dead_code` failure does not reproduce — Stream F confirmed `RuleRegistry` is fully `#[cfg(test)]`. Workspace clippy is green.

## What shipped (✅ delivered — details in plan)

| Bucket | What shipped |
|--------|----------------|
| Wave 1 | **`crates/polint/src/cache/keys.rs`**, infallible **`config_hash`**, Go parser slot without **`unwrap`**, **`file_in_scope`** alignment + denied-literal docs |
| Wave 2 | CI **`--all-features`**, **`Makefile`/`publish` `--locked`**, **`[workspace.lints]`** (`unsafe` forbid, `unreachable_pub` warn, clippy `dbg_macro` deny + `redundant_clone`/`todo`/`unimplemented`/`needless_collect`/`large_enum_variant` warn), removed **`pretty_assertions`**, **`tests/common`**, renamed integration test |
| Wave 3 | **`polint::sdk::scope`** (`file_in_scope`/`file_matches_globs`/`glob_matches`) — examples deduplicated, `globset` removed from example deps; **`#[cfg(any(test, feature = "bench"))]`** on `LoadSourcesTimings::total`; graph clones reduced (one `String` per unique node label); `import_alias` Go redundancy fixed; **`pub`** → **`pub(crate)`** on truly-internal items (graph/cli/diagnostics); **`#![deny(missing_docs)]`** on SDK; CI runs **`cargo deny`** + **`cargo doc -D warnings`**; **`Rule::run` → `RuleResult`** with **`RuleError`** (`thiserror`); **`go`/`ts`** split into **`adapter.rs`** + **`tests.rs`** + thin **`mod.rs`** |

[`RUST-AUDIT-IMPROVEMENT-PLAN.md` § Done](RUST-AUDIT-IMPROVEMENT-PLAN.md#done-implemented-2026-05-05) is the checklist of record.
