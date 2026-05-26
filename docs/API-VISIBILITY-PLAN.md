# API visibility hardening plan

**Goal:** Eliminate accidental `pub` inside `crates/polint`, align with [`AGENTS.md`](../AGENTS.md) (Conventions → Public API and visibility), and clear **`unreachable_pub`** warnings on normal `cargo build` / `cargo test` so CI logs and local builds stay quiet without hiding real issues.

**Non-goals:** Redesigning the rule-author SDK or changing semver policy; this is visibility and module-boundary hygiene.

## Current shape (baseline)

- [`crates/polint/src/lib.rs`](../crates/polint/src/lib.rs): **`pub`** only for `runner`, `sdk`, `run_main`, and **`#[cfg(feature = "bench")] pub mod _bench`** (required so `polint-bench` can import `polint::_bench::*`). Everything else is already **`pub(crate) mod`**.
- **`unreachable_pub`** fires because many items *inside* those `pub(crate)` modules are still declared **`pub`**, even though no downstream crate can name them through `lib.rs`. Rust correctly suggests **`pub(crate)`** (or tighter).

## Phase 40 eval boundary

Phase 40's evaluation harness, external benchmark adapters, adaptation records,
baseline comparison records, and promotion gates remain crate-private/internal.
They are allowed to support tests, research summaries, and hidden implementation
work, but they are not a supported SDK, runner API, stable CLI command, public
JSON schema, or docs/facts surface.

Public query-view promotion for call graph, data flow, evidence, benchmark
execution, or eval reports is deferred to Phase 41 and must go through the same
explicit visibility review as other SDK additions.

## Principles (execution checklist)

1. **Default private** in new code; widen only on demand.
2. **`pub(crate)`** for anything shared across `crates/polint` but not part of `sdk` / `runner` / `_bench` contracts.
3. **Do not** widen `sdk` or `runner` exports without an explicit design note and docs update.
4. **`_bench`:** keep stable enough for `polint-bench`; prefer `pub(crate)` on the underlying modules and thin `pub` re-exports only where the bench crate must see symbols.

## Phased rollout

Work in small PRs or commits per area so rebases stay tractable.

### Phase 1 — Leaf internal modules

**Scope:** `cache/` (including `keys.rs`), `config/`, `fs/`, `graph/`, `diagnostics/` (non-SDK parts), `rule_error.rs` if any stray `pub`.

**Actions:**

- Replace top-level **`pub`** on types, fns, and consts with **`pub(crate)`** where every use site is inside `crates/polint`.
- Keep **`pub`** only if something is re-exported through a path that must stay `pub` (unlikely here).

**Verify:** `cargo build -p polint --locked`, `cargo test -p polint --lib --locked`, `make lint`.

### Phase 2 — `core`

**Scope:** Large module; facts, `AnalysisDb`, `Rule`, `run_rules`, etc.

**Actions:**

- Same rule: **`pub`** only for items that `sdk` / `runner` / `_bench` genuinely need as **`pub`** through their module boundaries. Most internals should become **`pub(crate)`** or private with `pub(crate)` accessors.
- Pay attention to **`pub use`** or re-exports from `sdk::prelude` — prelude items come from `core`; those paths stay **`pub`** via `sdk`, not by leaving every `core` helper `pub`.

**Verify:** Full `cargo test --workspace --all-features --locked` (examples embed `polint`).

### Phase 3 — Adapters `go/` and `ts/`

**Scope:** `go/adapter.rs`, `go/mod.rs`, `ts/adapter.rs`, `ts/mod.rs`, tests under `go/tests.rs`, `ts/tests.rs`.

**Actions:**

- In **`adapter.rs`**, most `pub fn analyze_*` and similar are only called from runner/fs paths inside the crate → prefer **`pub(crate) fn`** unless a test-only need forces a different split (then **`pub(super)`** + `#[cfg(test)]` helpers in the parent module).
- **`pub use adapter::*`** in `mod.rs`: after adapter items are `pub(crate)`, the glob re-export still exposes them to sibling modules inside the crate; confirm no accidental **`pub`** leakage at the `go` / `ts` module boundary.

**Verify:** Adapter unit tests + integration tests touching Go/TS fixtures.

### Phase 4 — `runner`, `cli`, `main`

**Scope:** Only symbols that must stay public for binary vs lib split.

**Actions:**

- Tighten **`pub`** on CLI helpers and runner internals to **`pub(crate)`** where the binary only needs `run_main` + crate internals.
- Ensure **`run_cli`** and any stable rule-pack entry points in `runner` stay documented and correctly `pub`.

**Verify:** `cargo test -p polint --test cli --locked`, `polint` binary smoke.

### Phase 5 — `sdk` audit

**Scope:** `sdk/mod.rs`, `sdk/scope.rs`, prelude re-exports.

**Actions:**

- Confirm every **`pub`** item in `sdk` is intentional for rule authors and covered by `#![deny(missing_docs)]` where required.
- Avoid adding new **`pub use`** of internal types; prefer explicit re-exports in `prelude` only.

**Verify:** Doc build `RUSTDOCFLAGS=-D warnings cargo doc -p polint --all-features --no-deps --locked`.

### Phase 6 — Lint hardening ✅

- **`[workspace.lints.rust] unreachable_pub = "deny"`** in root [`Cargo.toml`](../Cargo.toml).
- Bench-only and `_bench` facade items use **`#[allow(unreachable_pub)]`** with a one-line rationale where rustc cannot see reachability through `polint::_bench::*`.

## Acceptance criteria (done = plan complete)

1. **`cargo build -p polint --locked`** emits **zero** `unreachable_pub` warnings (and no new `dead_code` / privacy errors).
2. **`cargo test --workspace --all-features --locked`** and **`make lint`** pass.
3. **Rule-pack compile check:** at least one example under `examples/**/.polint/rules` still builds against the published-style path dependency.
4. **`polint-bench`** still builds with **`--features bench`** on `polint` as today (or with documented import path updates only).

## Tracking

- Execute phases in order; merge Phase 1–2 before large adapter edits to reduce conflict surface.
- Link PRs to this file in the PR description until all phases are checked off.
