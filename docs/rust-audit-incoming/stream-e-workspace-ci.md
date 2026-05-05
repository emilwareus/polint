# Stream E — workspace & CI (Rust hygiene audit)

**Scope:** Root `Cargo.toml`, all workspace members, `.github/workflows/*.yml`, `.cargo/config.toml`, `rust-toolchain.toml`. Cross-checked with `.agents/skills/rust-best-practices` (Clippy / workspace lint guidance in `references/chapter_02.md`).

**Date:** 2026-05-05

---

## Summary

The workspace is **coherent**: edition 2024, resolver 3, single `[workspace.dependencies]` table, members consistently use `*.workspace = true`, `rust-toolchain.toml` pins the channel and adds `rustfmt` + `clippy`, and CI uses **`--locked`** for clippy, test, release builds, and dry-run publish. Gaps are mostly **lint policy in Cargo**, **Clippy/test flags vs handbook**, **Makefile vs CI parity**, a **dead workspace dependency**, **publish script `--locked`**, and **no supply-chain / MSRV / docs jobs**.

---

## Strengths

| Area | Observation |
|------|-------------|
| Workspace layout | One root manifest; `polint` is the published API; `polint-bench` is `publish = false`; example rule crates are `publish = false` and depend on `polint` via workspace — clear doc/example vs shipped library boundary. |
| Version alignment | Oxc crates share `0.129.0`; tree-sitter pair aligned; `polint` path dep uses matching `version = "0.1.2"`. |
| Toolchain | `rust-toolchain.toml` channel `1.95` matches `[workspace.package] rust-version`; components include `clippy` and `rustfmt`. |
| CI lockfile | `cargo clippy` and `cargo test` use `--workspace --locked`; install smoke uses `--locked`. |
| Release | CLI matrix uses `cargo build --locked --release -p polint`; dry-run publish uses `--locked`. |
| Cross-platform | Clippy/test/install smoke run on Ubuntu, Windows, macOS. |

---

## Findings

### 1. No `[workspace.lints]` (Clippy / rustc policy in Cargo)

**Current state:** No `[lints]` or `[workspace.lints]` in any workspace `Cargo.toml` (only appears in the skill reference text).

**Why it matters:** Chapter 2 recommends declaring **workspace-level** `[workspace.lints.rust]` / `[workspace.lints.clippy]` so defaults are uniform and `cargo clippy` without extra CLI flags still reflects project policy.

**Suggestion:** Add `[workspace.lints]` with `workspace = true` in member packages (or inherit), starting with e.g. `clippy::all` at deny/warn and explicit priorities for `redundant_clone`, etc., as in the handbook example — tuned to avoid immediate breakage.

---

### 2. CI Clippy omits `--all-features` (vs skill + bench feature)

**Current:** `cargo clippy --workspace --all-targets --locked -- -D warnings`

**Handbook:** `cargo clippy --all-targets --all-features --locked -- -D warnings`

**Why it matters:** `polint` exposes `feature = "bench"` and `polint-bench` depends on it. Without `--all-features`, Clippy may skip cfg-gated `bench` paths in some resolution paths; `--all-features` matches the documented “check everything” workflow.

**Suggestion:** Add `--all-features` to the CI Clippy step (and optionally the same for `cargo test` if you want tests to exercise feature combinations consistently).

---

### 3. `cargo test` in CI omits `--all-features`

**Current:** `cargo test --workspace --locked`

**Suggestion:** Align with Clippy: `cargo test --workspace --all-features --locked` if bench and other features should be continuously validated.

---

### 4. Makefile `test` target not `--locked`

**Current:** `$(CARGO) test --workspace`

**Gap:** Local default can drift from CI (lockfile stale locally but CI passes). `install` correctly uses `--locked`.

**Suggestion:** `$(CARGO) test --workspace --locked` (and optionally `--all-features`).

---

### 5. Toolchain drift: `dtolnay/rust-toolchain` and implicit `rust-toolchain.toml`

**Current:** Workflows use `dtolnay/rust-toolchain@stable` with only `components:` set.

**Note:** The action typically **reads `rust-toolchain.toml`** when no `toolchain:` input is set, so CI likely follows 1.95. This is implicit; contributors may not realize.

**Suggestion:** Optionally set `toolchain: 1.95` explicitly in workflow `with:` (or document that the file is the source of truth) so upgrades are deliberate and obvious in diffs.

---

### 6. `fmt` job toolchain vs `clippy-test-install` job

**Current:** `fmt` installs only `rustfmt`; matrix job installs only `clippy`.

**Observation:** Both should resolve to the same channel via `rust-toolchain.toml` — **low risk** if the action honors the file.

**Suggestion:** No change required unless you observe version skew; optional merge into one job to reduce total toolchain installs.

---

### 7. Release: `cargo build --workspace` without `--locked` during lock refresh

**Current (bump-and-tag):** After version bump, `cargo build --workspace` refreshes `Cargo.lock`.

**Observation:** Intentional for updating the lock after manifest changes. The following commit includes `Cargo.lock`. Acceptable pattern.

---

### 8. `scripts/publish-crates.sh` — real publish missing `--locked`

**Current:** `DRY_RUN` path uses `cargo publish -p polint --dry-run --locked`; production loop uses `cargo publish -p "$p" --token ...` **without** `--locked`.

**Why it matters:** Publishes can proceed from a tree where `Cargo.lock` is out of date relative to `Cargo.toml`, weakening reproducibility vs what CI tests.

**Suggestion:** Add `--locked` to the non-dry-run `cargo publish` invocation.

---

### 9. Dead `workspace.dependencies` entry: `pretty_assertions`

**Current:** Declared in root `[workspace.dependencies]` but **no member** `Cargo.toml` references `pretty_assertions.workspace = true`.

**Suggestion:** Remove from `[workspace.dependencies]` or add to `polint` dev-dependencies if you plan to use it — avoid unused pinned versions.

---

### 10. Library docs / `#![deny(missing_docs)]`

**Current:** `crates/polint/src/lib.rs` has crate-level `//!` docs but no `#![deny(missing_docs)]`. The skill’s quick reference recommends `deny(missing_docs)` for libraries.

**Observation:** Many internal `pub(crate)` modules may require a focused rollout before turning on crate-wide deny.

**Suggestion:** Consider `#![deny(missing_docs)]` for `sdk` (and public re-exports) first, or workspace lint `unsafe_code = "forbid"` / `missing_docs` as warn in `[workspace.lints]` before denying.

---

### 11. Example member manifests: minimal `workspace.package` inheritance

**Current:** Example rule crates set `version`, `edition`, `publish`; they do **not** inherit `license.workspace`, `repository.workspace`, or `description.workspace`.

**Observation:** For `publish = false` examples this is often intentional. If you ever publish an example template, align metadata.

---

### 12. `.cargo/config.toml`

**Current:** Only `[target.aarch64-unknown-linux-gnu]` linker for `aarch64-linux-gnu-gcc` — supports cross-compile story (matches release workflow installing `gcc-aarch64-linux-gnu`).

**Observation:** No alias for `clippy`/`test`; no `[build]` rustflags — neutral.

---

### 13. Security / minimal versions / supply chain — gaps

**Current:** No `cargo audit`, `cargo deny`, Dependabot config, or `-Zminimal-versions` (nightly) in CI.

**Suggestion:** Add at least one of: scheduled `cargo audit` job, `deny.toml` + `cargo deny check`, or Dependabot for Rust — per team appetite for noise vs coverage.

---

### 14. MSRV / doc matrix gaps

**Current:** No dedicated job that runs `cargo check --workspace --locked` on an **explicit** MSRV image (beyond what stable resolves to), and no `cargo doc --workspace --no-deps` (or `-D warnings`) job for rustdoc hygiene.

**Suggestion:** Optional `msrv` job pin channel `1.95` and optional `doc` job with `RUSTDOCFLAGS='-D warnings'` for public API crates.

---

## Priority recommendations (concise)

1. Add **`--all-features`** to CI Clippy (and likely test); mirror in Makefile if you add `--locked` there.  
2. Introduce **`[workspace.lints]`** + `lints.workspace = true` on members when ready.  
3. **`cargo publish --locked`** in `publish-crates.sh` non-dry-run path.  
4. Remove or **use** `pretty_assertions` in workspace deps.  
5. Optionally **explicit toolchain version** in workflows and **supply-chain** / **doc** jobs.

---

## Files reviewed

| Path | Role |
|------|------|
| `Cargo.toml` | Workspace root, `workspace.dependencies`, members |
| `crates/polint/Cargo.toml`, `crates/polint-bench/Cargo.toml` | Core crates |
| `examples/*/.polint/rules/Cargo.toml` | 11 example rule packages |
| `.github/workflows/ci.yml`, `release.yml` | CI and release |
| `.cargo/config.toml` | Target linker |
| `rust-toolchain.toml` | Channel + components |

---

## Not in scope (per instructions)

Other shards under `docs/rust-audit-incoming/`; non-member `_scaffold_smoke_*` `Cargo.toml` trees.
