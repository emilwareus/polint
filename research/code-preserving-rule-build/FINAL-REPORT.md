# polint: code-preserving build, distribution, and execution architecture

**Repository:** `emilwareus/polint` at `/workspace/polint`
**Commit:** `b272b378` (`main` = `origin/main`), workspace version `0.2.1`
**Date:** 2026-08-25
**Status:** Research only. No repository files were modified; no branches, commits, or PRs were created.

**Reading note.** This report is a dated snapshot: every repository path and line
range is as of `b272b378`. Where the branch that carries this report has since
moved a line, the citation is annotated. One substantive figure has been
superseded — the build cost quoted in §3.3 was a documentation figure, and
`research/evaluation-harness/baselines/build-cost.json` now carries a first-party
measurement of the same path. §3.3 records both; §9 states which of its numbers
are measured and which remain budgets.

---

## 1. Executive recommendation

**Adopt "prebuilt engine host + thin-SDK rule binary over a fact-snapshot protocol." Do not adopt a DSL, do not adopt native plugins, and do not make WASM the primary answer.**

The single change that matters is an **inversion**: today the customer compiles the entire polint engine into a repo-local binary and that binary does the analysis; the prebuilt `polint` CLI is only an orchestrator. Invert it. Let the **prebuilt, downloaded `polint` binary do the analysis** (it already owns the cache, the config, and the capability planner), and let the repo-local rule pack compile against a **thin SDK crate that contains no parsers, no solvers, no SQLite, and no kernel** — just fact types, fact views, diagnostics, `RuleCtx`, and the runner protocol client.

Concretely, in priority order:

1. **Split the crate along a boundary that already exists in the source.** Create `polint-sdk` (thin) and `polint-engine` (the current 238,359-line engine), and keep **`polint` as a thin facade crate** so that *rule source files do not change by a single byte* and `cargo install polint` still works. Rule packs depend on `polint` with `default-features = false`.
2. **Move fact production out of the rule host.** The prebuilt host computes facts and writes a **fact snapshot**; the rule binary deserializes it once into an owned `FactSnapshot` and the existing views borrow from it, so `SourceFiles::all(self) -> &'a [SourceFile]` and every other typed accessor keeps its exact signature.
3. **Stop spawning `cargo` when nothing changed.** Fingerprint the rule sources + manifest + SDK version + `rustc -vV`; if the cached rule binary matches, run it directly. Today `polint check` spawns `cargo run` unconditionally.
4. **Vendor the SDK source inside the polint binary** and build the rule pack with `--offline --locked` against a materialized path dependency. This makes first-run offline operation possible and eliminates version skew between the CLI and the library the rule pack links.
5. **Then, and only then, consider prebuilt SDK rlib artifacts** (keyed on exact `rustc -vV`) as a *fast path with source fallback*, and a **WASM backend** as a *second execution target* for shared rule packs and untrusted repositories.

**Why this and not the alternatives.** Native Rust rule code must be compiled for the target by something, somewhere — that is not negotiable, and any claim of "no compilation at all while rules stay Rust" is either false or a claim that the *author* compiles instead of the customer. So the design question is not "can we remove compilation" but **"what is the smallest thing the customer must compile, and can they compile it offline, fast, on two cores?"** Today the measured answer on `examples/basic` is "225 compilation units and 187 s the first time, one Cargo start on every scan afterwards whether or not anything changed, 582.7 MB retained" (§3.3.2, `research/evaluation-harness/baselines/build-cost.json`). After this change the answer should be "one crate, against a prebuilt SDK, in seconds" — and with prebuilt SDK artifacts, "one `rustc` invocation."

**Scenario defaults** (detail in §5):

| Scenario | Default |
|---|---|
| **A** — team authors and reviews its own Rust rules, scans locally/CI | Native thin-SDK rule binary + snapshot protocol. Compile once per rule edit. CI publishes a prebuilt rule artifact so non-authors never compile. |
| **B** — customer receives a prebuilt/shared rule pack from another party | Signed portable artifact produced by the author's CI. Customer compiles nothing. Interim: per-target native binaries (polint already ships its own CLI this way). Target state: one WASM module. |
| **C** — agent scans an arbitrary fresh repo that may contain untrusted rules | **Do not build and do not execute repo rules.** Explicit, persisted per-repo trust is required. Even with trust, apply manifest lockdown and `--offline`. When the WASM backend exists, offer `--trust-rules=sandbox`. |

---

## 2. User correction and product invariant: rules remain code

The previous recommendation — make repo policies primarily a DSL or declarative format — **was rejected and is treated here as out of scope as a primary model.**

The product invariant this report holds fixed:

> **Rules are real Rust code.** A rule is a plain synchronous function annotated with `#[polint::rule]`, taking `&mut RuleCtx<'_>` first and typed fact views as further parameters, returning `RuleResult`. Capabilities are *derived from the function signature*. The author writes ordinary Rust with ordinary control flow, ordinary helper functions, ordinary `cargo`-shaped testing, and ordinary review. Engineers and AI agents read and modify it as code.

This invariant is not incidental — it is enforced by the repository's own contracts:

- The macro derives capabilities from fact-view parameter types and refuses lookalike types (`AGENTS.md:138-152`, `crates/polint-macros/src/lib.rs:313-324`).
- `AGENTS.md:138-141` explicitly forbids "manual `impl Rule` examples, compatibility shims, or public rule constructors as a beta escape hatch" — the typed macro path is *the* path.
- `ARCHITECTURE.md:485-495` lists "shipping a built-in policy catalog instead of repository-owned rules" as an enduring non-goal.

**Everything in this report is constrained by that invariant.** Where an option would erode it — for example, native `cdylib` plugins requiring `#[repr(C)]` fact structs, or a WASM guest interface that forces every view to return owned `Vec`s instead of borrowed slices — the option is rejected on that ground and the ground is stated.

**Where a declarative subset may legitimately appear** (optional convenience only, never the core):
- A `polint new-rule --from-config` generator that *emits Rust source* for very common shapes (forbidden imports, denied literals, thresholds). The output is code the author owns and edits. This is a scaffolding feature, not a rule model.
- `[[rules.config]]` already carries arbitrary rule-owned TOML into `RuleOptions::settings` (`docs/CONSUMER-SETUP.md:276-297`). That is the correct amount of "declarative": *data* is declarative, *policy* is code.

Neither of those is on the critical path of this recommendation.

---

## 3. Current-state evidence

### 3.1 What runs today, exactly

`polint check` discovers rule packs from `[rules] paths` in `.polint.toml` and, **if any exist, delegates the entire run to them**:

```rust
// crates/polint/src/cli/mod.rs:3418-3425
fn check(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    if args.new_only && !args.baseline { anyhow::bail!("--new-only requires --baseline"); }
    let local_rule_hosts = discover_local_rule_hosts(&root)?;
    if !local_rule_hosts.is_empty() {
        return check_local_rule_hosts(&root, args, &local_rule_hosts);
    }
    ...
}
```

Discovery is manifest-file existence under configured paths (`crates/polint/src/cli/mod.rs:3872-3882`). Each discovered manifest gets its own subprocess (`crates/polint/src/cli/mod.rs:3949-3962`).

The subprocess is **`cargo run`**:

```rust
// crates/polint/src/cli/mod.rs:4260-4297 (run_local_rule_host_kind)
let cargo = std::env::var("POLINT_CARGO")
    .or_else(|_| std::env::var("CARGO"))
    .unwrap_or_else(|_| "cargo".to_string());
let cache_layout = CacheLayout::for_repo(root);
let mut command = ProcessCommand::new(&cargo);
command.current_dir(root).args(["run", "--quiet"]);
apply_local_rule_host_profile(&mut command);          // adds --release by default
command.args([
    "--manifest-path", manifest…, "--",
    "check", "--format", "json", "--fail-on", "none",
    "--ignore-comments", …, "--kind", kind,
]);
command
    .env(POLINT_CACHE_DIR_ENV, cache_layout.root())
    .env("CARGO_TARGET_DIR", cache_layout.rules_target_dir());
```

The default Cargo profile for the rule host is **release** (`crates/polint/src/cli/mod.rs:4425-4452`), because "rule execution can dominate large-repo scans" (`docs/CONSUMER-SETUP.md:175-178`).

There is a **second** `cargo run` for rule metadata (`inspect rule`), at `crates/polint/src/cli/mod.rs:4350-4407`, and a **third** in the fixture-test path (`polint test`), at `crates/polint/src/rule_test.rs:323-373`. The extension host adds a **fourth** `cargo run` for `.polint/extensions/*` (`crates/polint/src/analysis/extensions/host.rs:128-162`).

The child process is where the engine actually runs:

```rust
// crates/polint/src/runner/mod.rs:415-439 (analyze_and_run)
let mut output = AnalysisKernel::run(KernelInput {
    loaded: &loaded, cache: &cache,
    config_digest: &config_digest, rule_digest: &rule_digest,
    plan: &plan, parallel: true,
})?;
…
diagnostics.extend(run_rules_with_runtime_provider_blockers(
    &output.db, rules, &options, Some(&exact_enabled), true,
    &output.capability_support, &output.runtime_blocked_rules,
));
```

`run_cli` also initializes `tracing_subscriber` (`crates/polint/src/runner/mod.rs:144-149`) and parses a full `clap` CLI (`crates/polint/src/runner/mod.rs:22-142`).

**Consequence: the prebuilt `polint` binary the customer downloads is, in the presence of a rule pack, a JSON-parsing orchestrator.** All parsing, all providers, all solvers, all caching, all fact production happen in a binary the customer compiled. The outer process additionally loads a *scoped* subset of sources a second time when it must apply comment-ignores or render `--stat` (`crates/polint/src/cli/mod.rs:3967-3985`).

### 3.2 What the customer's rule pack actually compiles

`polint init` / `polint new-rule` scaffold this manifest:

```rust
// crates/polint/src/cli/mod.rs:1130-1158 (pack_cargo_toml)
polint = { version = "{version}", default-features = false, features = [{features}] }
…
[package]
name = "polint-local-rules"
version = "{version}"
edition = "2024"
publish = false
[dependencies]
{polint_dep_line}
[workspace]
```

`{features}` mirrors the CLI build's language selection (`crates/polint/src/cli/mod.rs:1119-1128`). The trailing empty `[workspace]` makes the rule pack its own workspace — which is good news for feature unification (§6.2).

The `polint` library it depends on is one crate of **238,359 lines** of Rust:

| Module (`crates/polint/src/…`) | Lines | Role |
|---|---:|---|
| `analysis_neutral/**` | 88,470 | CFG, calls, data flow, IFDS/slicing, domains, summaries, solvers, identity, module/symbol models |
| `analysis_kernel/**` | 36,137 | composition root, provider scheduling, incremental keys, SQLite-backed store |
| `ts/**` | 31,045 | Oxc TS/JS frontend, lowering, symbol/module graphs |
| `go/**` | 20,248 | tree-sitter Go frontend, sidecar lifecycle, lowering |
| `core/**` | 11,640 | `AnalysisDb`, `Rule`, `RuleCtx`, fact stores, metadata |
| `cli/**` | 5,572 | the CLI, the scaffolder, the generated skill |
| `analysis/**` | 8,420 | facade providers, extensions, unknown taxonomy |
| `module_graph/`, `symbol_graph/` | 8,675 | facade graph entry points |
| `cache/**`, `config/**` | 2,831 | cache layout, digests, config loading |
| `diagnostics/mod.rs` | 2,833 | `Diagnostic`, renderers (human/GitHub/JSON/SARIF/ai-friendly) |
| `policy_queries.rs` | 3,580 | bounded query evaluation behind `Events`/`Calls`/`DataFlow` |
| `sdk/**` | 3,385 | `facts.rs` 1,985 · `policy.rs` 958 · `scope.rs` 183 · `mod.rs` 259 |
| `internal_core/`, `ir/`, `analysis_api/`, `frontend_api/` | 5,242 | foundations and contracts |
| **Total `crates/polint/src`** | **238,359** | |

*(Measured with `find … -name '*.rs' | xargs wc -l` per directory on the working tree at `b272b378`.)*

Its dependency families, from `crates/polint/Cargo.toml:26-68` and `Cargo.toml:38-73`:

- **Oxc** (17 crates: `oxc_parser`, `oxc_semantic`, `oxc_ast`, `oxc_ast_visit`, `oxc_ecmascript`, `oxc_regular_expression`, `oxc_syntax`, `oxc_span`, `oxc_allocator`, `oxc_data_structures`, `oxc_diagnostics`, `oxc_estree`, `oxc_index`, `oxc_str`, `oxc-miette`, `oxc-miette-derive`, `oxc_ast_macros`) plus `oxc_resolver`, which drags in `simd-json`, `value-trait`, `halfbrown`, `url`, `idna`, and the **twelve `icu_*` crates**.
- **tree-sitter** + **tree-sitter-go** — C compilation via `cc`.
- **`rusqlite` with `features = ["bundled"]`** (`Cargo.toml:61`) → `libsqlite3-sys` with a `cc`/`pkg-config`/`vcpkg` build script. **Every rule-pack build compiles the SQLite amalgamation from C**, on every platform, including Windows. This dependency is *not* optional and *not* feature-gated; it exists for `analysis_kernel/store/**`.
- `rayon`, `petgraph`, `clap` + `clap_derive`, `serde` + derive, `serde_json`, `serde_norway` (+ `unsafe-libyaml-norway`), `toml` (+ `winnow`, `serde_spanned`, `toml_parser`, `toml_writer`), `tracing` + `tracing-subscriber` (+ `regex`, `regex-automata`, `regex-syntax`, `matchers`, `sharded-slab`, `thread_local`, `nu-ansi-term`), `ignore` + `globset` (+ `aho-corasick`, `bstr`, `walkdir`, `same-file`), `json-strip-comments`, `tempfile`, `thiserror`, `anyhow`, `libc`/`rustix` (unix), `windows-sys` (windows).
- `polint-macros` — a proc-macro crate, so `syn` + `quote` + `proc-macro2` compile for the host as well.

The workspace `Cargo.lock` contains **274 packages** (that count includes dev-dependencies and the 17 checked-in example rule packs; it is an upper bound on any single build's closure, not the rule-pack closure itself).

Two Cargo facts shape what that number means in practice, both verified against primary docs:

- *"Platform-specific dependencies with the `[target]` table are resolved as-if all platforms are enabled. In other words, the resolver ignores the platform or cfg expression."* — [Cargo Book, Dependency Resolution](https://doc.rust-lang.org/cargo/reference/resolver.html). This is why `sqlite-wasm-rs`, the `wit-*`/`wasm-*` family, and the whole `windows-*` family appear in the lock on a Linux machine.
- *"If `--target` is not specified, then all target dependencies are fetched."* — [`cargo fetch`](https://doc.rust-lang.org/cargo/commands/cargo-fetch.html). Building filters by target; the resolve graph does not.

### 3.3 The measured cost

Two measurements describe the same path. The first is the figure this report was
written against; the second was taken afterwards, with a harness, and is the one
later work should quote.

#### 3.3.1 The documentation figure this report was written against

The repository records a measurement of the rule-host build:

> "Measured locally on a rule pack with a path dependency on the `polint` library (x86_64 Linux, release profile, **223 compiled units**): a cold build takes **185.4 s**; after pruning, the rebuild recompiles exactly one unit — the rule package — in **0.7 s**. The target directory is **562 MB** before pruning and **537 MB** after, the difference being the rule package's own output plus incremental state."
> — `docs/GITHUB-ACTION.md:162-166`

It decomposes the problem precisely:

- **222 of the 223 units are not the customer's rule.** They are polint and its dependencies.
- **The customer's own rule crate compiles and links in 0.7 s.** Linking a binary against the already-built polint rlib set is *not* the bottleneck.
- **537 MB must be retained** for that 0.7 s to stay 0.7 s.

So the cost is entirely "compile a static-analysis engine per repository," and the rule itself is already nearly free. That is exactly the shape that a thin-SDK split fixes.

It is, however, a prose figure in a document rather than a reproducible
measurement, and it was recorded for the GitHub Action's cache design rather than
for this question. §3.3.2 replaces it.

#### 3.3.2 The first-party measurement

`polint-bench build-cost` drives the real CLI against a scratch copy of a
repository, counts Cargo starts through the `POLINT_CARGO` indirection, counts
compiled units through a `RUSTC_WRAPPER` shim, and walks the rule-host target
directory and the polint cache either side of each run. The committed artifact is
`research/evaluation-harness/baselines/build-cost.json`
(`schema_version = polint-build-cost-1`); `research/evaluation-harness/README.md`
documents each metric and each limit.

Measured on `examples/basic`, release profile, one machine (`environment.label =
linux-container-6cpu`, `cargo`/`rustc` 1.95.0), median of three runs per cell:

| Scenario | Cargo starts | Compiled units | Wall clock | `rules-target` after |
|---|---:|---:|---:|---:|
| `cold` | 1 | **225** | 187.3 s | **582.7 MB** (1,708 files) |
| `warm-noop` | **1** | 0 | 157 ms | 582.7 MB |
| `warm-rule-edit` | 1 | **1** | 735 ms | 582.7 MB |
| `warm-source-edit` | **1** | 0 | 163 ms | 582.7 MB |
| `test-suite` (2 generated cases) | **2** | 225 | 209.5 s | 582.7 MB |

What this confirms, and what it corrects:

- **Confirmed — the shape.** 224 of the 225 compiled units are polint and its
  dependencies (241 `rustc` starts, 16 of them Cargo probes); a one-line rule
  edit recompiles exactly one unit and writes 25.9 MB. The engine, not the rule,
  is the cost.
- **Confirmed — Cargo runs on every scan.** `warm-noop` and `warm-source-edit`
  compile nothing and still start Cargo once. That is the §4.1 claim, measured.
- **Confirmed — the counts are deterministic.** Cargo starts, `rustc` starts,
  compiled units, retained bytes, and retained file counts were identical across
  all three runs of every cell. Only wall-clock and rule-host RSS moved.
- **Corrected — the retained figure.** 582.7 MB, not 537 MB. The two are not the
  same quantity: 537 MB is the Action's figure *after* it prunes the rule
  package's own output, and it was taken on different hardware with a different
  pack. The harness prunes nothing, so 582.7 MB is closer to what a developer's
  disk actually holds.
- **Corrected — warm scans are cheaper than the documentation implies, and still
  not free.** A no-op re-scan costs 157 ms end to end, essentially all of it a
  Cargo freshness pass that compiles nothing; a rule edit costs 735 ms. The
  target for those cells is therefore "zero Cargo starts", not "a faster build".
- **Not comparable — wall clock across machines.** 187.3 s here against the
  documentation's 185.4 s is coincidence, not agreement: the same cell on this
  same host measured 417.6 s while the host was contended. Wall-clock is only
  meaningful against a baseline from the same machine in the same state.
- **Not yet measured.** One repository, one machine. Compiler peak RSS is never
  observed and is recorded as `null`, not estimated. The 2 vCPU / 4 GB acceptance
  rig (R1, §9.1) has not been run.

### 3.4 Everything else the cold path requires

- **A Rust toolchain**, and specifically one at or above MSRV 1.95 (`Cargo.toml:36`, `rust-toolchain.toml`). `polint init` writes a **repository-root `rust-toolchain.toml`** pinning `channel = "1.95"` when one is absent (`crates/polint/src/cli/mod.rs:723-742`). On a rustup installation that channel string triggers a toolchain download if it is not present.
- **Network access to crates.io** on first build. The failure taxonomy is explicit: MSRV, network/registry, manifest, missing `rustc` (`crates/polint/src/cli/rules_host_error.rs:20-34, 74-95`). There is no offline path today: the rule pack's `polint` dependency is a registry dependency.
- **A C toolchain**, for `libsqlite3-sys` (bundled) and `tree-sitter`.
- **`cargo` on `PATH` on every single scan.** `run_local_rule_host_kind` spawns `cargo run` unconditionally; even a fully-warm no-op scan pays a Cargo fingerprint pass and a process spawn.
- **Incremental compilation is disabled repository-wide** (`.cargo/config.toml:1-4`: *"Incremental artifacts are local to each worktree and can dwarf the actual build outputs. A shared sccache provides cross-worktree reuse without them."*) — but that file governs the polint repo, not a customer repo; a customer repo gets Cargo's defaults (dev: `incremental = true`; release: `incremental = false`, per the [Cargo profiles reference](https://doc.rust-lang.org/cargo/reference/profiles.html)).

### 3.5 The three assets that already exist and make the fix cheap

This is the most important finding in the current-state review. **The pieces required for a host protocol are already built and already versioned.**

**(a) The typed SDK is already a pure read projection.** Every fact view is a one-field newtype over `&AnalysisDb`, and every method is a slice, an iterator, or a filter:

```rust
// crates/polint/src/sdk/facts.rs:1098-1128
pub trait FactView<'a>: Sized { fn build(db: &'a AnalysisDb) -> Self; }
macro_rules! impl_fact_view { ($ty:ident) => {
    impl<'a> FactView<'a> for $ty<'a> { fn build(db: &'a AnalysisDb) -> Self { Self { db } } }
}; … }

// crates/polint/src/sdk/facts.rs:23-52
pub struct SourceFiles<'a> { db: &'a AnalysisDb }
impl<'a> SourceFiles<'a> {
    pub fn all(self) -> &'a [SourceFile] { self.db.files() }
    pub fn iter(self) -> std::slice::Iter<'a, SourceFile> { self.db.files().iter() }
    …
}
```

`RuleCtx` is likewise read-only over the same reference (`crates/polint/src/core/rule.rs:156-162`), and the `Rule` closure type is `Fn(&AnalysisDb, &mut RuleCtx<'_>) -> RuleResult` (`crates/polint/src/core/rule.rs:61-81`). **Nothing on the rule-facing path mutates the database or drives analysis.** The only reason the rule pack links the engine is that the *type* it borrows from is the engine's `AnalysisDb`.

**(b) The public fact structs are plain, `#[non_exhaustive]` data.** For example:

```rust
// crates/polint/src/analysis_api/symbol_facts.rs:134-151
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SymbolFact {
    pub id: SymbolId, pub language: Language, pub name: String,
    pub qualified_name: String, pub kind: SymbolKind, pub namespace: SymbolNamespace,
    pub file: Option<FileId>, pub package: Option<PackageId>, pub module: Option<ModuleNodeId>,
    pub owner: Option<SymbolId>, pub primary_span: Option<Span>, pub is_exported: bool,
    pub stable_key: StableKeyId, pub precision: SymbolPrecision,
}
```

No trait objects, no closures, no borrowed lifetimes, no `Arc<dyn …>`. Adding `Serialize`/`Deserialize` is mechanical; `#[non_exhaustive]` gives wire-format headroom. The neighbouring enums already derive `Serialize, Deserialize` (`crates/polint/src/analysis_api/symbol_facts.rs:124-132`).

**(c) polint already ships an external process protocol, with handshake, schema versions, timeouts, and output bounds.** The extension host speaks JSON over stdio to a `cargo run` child:

```rust
// crates/polint/src/analysis/extensions/host.rs:17-19, 56-126
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_STDOUT_LIMIT: usize = 1_048_576;
const DEFAULT_STDERR_LIMIT: usize = 16_384;
…
pub(crate) fn handshake(&self, extension: &DiscoveredExtension)
    -> ExtensionHostResult<ExtensionHandshakeResponse> { … }
pub(crate) fn run_provider(&self, extension: &DiscoveredExtension, provider_id: &str, …)
    -> ExtensionHostResult<ExtensionProviderRunResponse> { … }
```

And the two halves of the rule wire are **already public, versioned JSON schemas**: `polint-rule-inspect-v1.json` for rule manifests (`docs/CONSUMER-SETUP.md:44-57`) and `POLINT_REPORT_JSON_SCHEMA_V1_URL` for the report (`crates/polint/src/sdk/mod.rs:41-46`). The host already parses the report from the child's stdout (`crates/polint/src/cli/mod.rs:4331-4342`).

**The only wire object that does not exist yet is the fact snapshot.**

**(d) The macro is already engine-agnostic.** The generated code names only SDK paths:

```rust
// crates/polint-macros/src/lib.rs:64-120 (abridged)
let #ident: #ty = <::polint::sdk::facts::#view_type<'_>
    as ::polint::sdk::__private::FactView<'_>>::build(db);
…
::polint::sdk::__private::make_rule_with_manifest(
    || ::polint::sdk::__private::RuleMeta { … },
    || ::polint::sdk::__private::Capabilities::new()#(.#capability_methods())*,
    vec![#(#fact_view_requirements),*],
    |db: &::polint::sdk::__private::AnalysisDb,
     #ctx_ident: &mut ::polint::sdk::prelude::RuleCtx<'_>| -> …
)
```

Every path is `::polint::sdk::*` or `::polint::sdk::prelude::*`. **If `polint::sdk::__private::AnalysisDb` becomes a re-export of an SDK-owned `FactSnapshot`, the macro output does not change at all.**

### 3.6 Architectural history that constrains the answer

The repository already tried a multi-crate split and it did **not** land. `crates/polint/tests/internal_architecture.rs:4-33` asserts that seven named packages must not reappear in the workspace manifest, the lockfile, or on disk:

```rust
const REMOVED_PACKAGES: &[&str] = &[
    "polint-core", "polint-ir", "polint-analysis-api", "polint-frontend-api",
    "polint-analysis", "polint-go", "polint-ts",
];
#[test] fn workspace_has_only_two_publishable_product_packages() { … }
```

`.swarm/T-SPLIT-LAND.md:1-27` records the correction candidly: the eight-crate split was claimed and did not happen; what landed was a *module* reorganization plus a behavioural gate (`crates/polint/tests/module_layering.rs`) that asserts the forbidden edges.

**This is a hard constraint on the recommendation, and a useful one.** The right answer here is *not* "resurrect seven crates." It is "add exactly **one** new compiler-enforced boundary — the one that is actually load-bearing for the customer's build — and keep everything else as internal modules." §7 keeps the count at three publishable packages.

`ARCHITECTURE.md:485-508` also lists relevant enduring non-goals, two of which bear directly on this work:

- *"promising a public ABI for dynamically loaded providers"* — this forecloses native `cdylib` plugins as a supported design, independently of the technical arguments in §6.4.
- *"treating full persistent fact storage or demand-driven/editor-latency execution as an author-facing guarantee"* — this bounds the daemon option in §6.8.

---

## 4. Answers to the seven questions

### 4.1 What exactly must be compiled today, and why

Per repository, per developer machine, per CI runner, on the first `polint check`:

| Compiled | Why | Removable? |
|---|---|---|
| The whole `polint` library — 238,359 lines | The rule pack's `main.rs` calls `polint::runner::run_cli`, and `run_cli` runs `AnalysisKernel::run` in-process (`runner/mod.rs:415-422`). The rule host *is* the engine. | **Yes** — move the kernel to the prebuilt host. |
| Oxc (17 crates + `oxc_resolver` + `simd-json` + `url`/`idna`/12× `icu_*`) | `feature = "lang-typescript"`, mirrored into the pack manifest by `cli/mod.rs:1119-1128`. | **Yes** — parsers belong to the engine. |
| tree-sitter + tree-sitter-go (C) | `feature = "lang-go"`. | **Yes** — same. |
| `rusqlite` + `libsqlite3-sys` **bundled** (SQLite amalgamation, C) | Unconditional dependency of `polint` (`Cargo.toml:61`) for `analysis_kernel/store/**`. Not feature-gated. | **Yes** — the store is host-side; and it should be feature-gated in the engine regardless. |
| `rayon`, `petgraph`, `ignore` | Parallel providers, graphs, file walking. | **Yes** — host-side. |
| `clap` + `clap_derive` | `runner::run_cli` parses a full CLI (`runner/mod.rs:22-142`). | **Yes** — the host↔rule wire is polint-internal; a ~100-line parser suffices. |
| `tracing-subscriber` (+ `regex`, `matchers`, `sharded-slab`, `thread_local`, `nu-ansi-term`) | `run_cli` initializes a subscriber (`runner/mod.rs:145-149`). | **Yes** — drop from the SDK. |
| `toml`, `serde_norway`, `json-strip-comments` | Config loading, baselines, tsconfig. | **Yes** — host-side. |
| `serde`, `serde_json` | Diagnostics, report JSON. | **No** — SDK needs them. |
| `globset` (+ `aho-corasick`, `bstr`, `regex-automata`, `regex-syntax`, `memchr`) | `sdk::scope::glob_matches` (`sdk/scope.rs:36-78`). | **Optional** — keep, or hand-roll. Measure. |
| `polint-macros` + `syn` + `quote` + `proc-macro2` | `#[polint::rule]` is the product. | **No** — irreducible. |
| The customer's rule crate | The policy. | **No** — irreducible, and already only 0.7 s. |

**Why it is like this:** the design chose *in-process fact access with borrowed slices*, which is excellent for rule ergonomics and zero-copy performance, and paid for it by making the rule pack link the producer of those slices. That trade was correct when the engine was small. At 238 KLOC and 223 units it is no longer correct, and the trade can be preserved (borrowed slices!) while changing *who produces* them.

Also worth stating: **`cargo` runs on every scan, not only on the first.** `cli/mod.rs:4264` spawns `cargo run` unconditionally. Cargo's fingerprint pass is fast but not free, and it hard-requires `cargo` + `rustc` on `PATH` even when the rule binary is already current.

### 4.2 Which parts force the large closure, and which can be split out

**Force the closure (must stay in the engine):**

| Module | Lines | Pulls |
|---|---:|---|
| `ts/**` | 31,045 | the entire Oxc family, `oxc_resolver`, `simd-json`, `url`/`idna`/`icu_*` |
| `go/**` | 20,248 | `tree-sitter`, `tree-sitter-go` (C) |
| `analysis_kernel/**` | 36,137 | `rusqlite` **bundled** (C), incremental keys, provider scheduling |
| `analysis_neutral/**` | 88,470 | `petgraph`, `rayon`; solvers, IFDS, domains, summaries |
| `analysis/**`, `module_graph/`, `symbol_graph/`, `frontend/`, `fs/`, `git/` | ~20,000 | `ignore`, process plumbing |
| `config/`, `cache/`, `baseline.rs`, `ignores.rs` | ~5,000 | `toml`, `serde_norway`, `json-strip-comments` |
| `cli/**` | 5,572 | `clap` |

**Can be split out (the SDK):**

| Module | Lines | Notes |
|---|---:|---|
| `sdk/facts.rs` | 1,985 | pure `&db` projections |
| `policy_queries.rs` | 3,580 | evaluation behind `Events`/`Calls`/`DataFlow`; reads facts only (`sdk/facts.rs:886-899, 950-962`) |
| `sdk/policy.rs` | 958 | pattern builders; imports only `cache::stable_hash` and `diagnostics` |
| `sdk/scope.rs`, `sdk/mod.rs` | 442 | glob scoping, prelude, `__private` |
| `rule_error.rs`, `rule_manifest.rs`, `core/capability.rs`, `core/labels.rs`, `core/metadata.rs` (partial) | ~2,500 | rule contracts |
| `core/rule.rs` | 451 | `Rule`, `RuleCtx`, `RuleOptions`, `Capabilities` |
| `core/db.rs` **read side** | ~2,000–3,000 of 5,801 | accessors + stable-key resolution; the store/interner *mutation* machinery stays in the engine |
| `internal_core/diagnostic.rs` + JSON report | ~1,500 of 2,833 | `Diagnostic`, `Severity`, `Evidence`, `Fix`, `PolintReport`; human/GitHub/SARIF/ai-friendly **renderers stay in the host** |
| `internal_core` ids/spans/lang/stable-key **types** | ~600 | `FileId`, `Span`, `Language`, `StableKeyId` |

**Estimated SDK size: ~14,000–18,000 lines** (6–8 % of the crate), with a dependency closure of roughly `serde`, `serde_json`, `globset` (+5 transitives), and the proc-macro trio. That is on the order of **25–35 units instead of 223** — a target to be *measured* in experiment E1, not a claim.

Two useful bonus facts:

- `crates/polint/src/cache/mod.rs:872-885` — `stable_hash` is a hand-written FNV-1a with **no dependency**. It can move to the SDK for free.
- `crates/polint/src/sdk/policy.rs:9-10` — `sdk::policy` imports exactly two internal things (`cache::stable_hash`, `diagnostics`). It is already almost free-standing.

**The one hard piece is `core/db.rs` (5,801 lines).** It holds both the read model the SDK needs and the fact-store/interner machinery the engine needs:

```rust
// crates/polint/src/core/db.rs:142-156
pub struct AnalysisDb {
    pub(crate) files: Vec<SourceFile>,
    pub(crate) stable_keys: StableKeyInterner,
    pub(crate) fact_meta: FactMetaStore,
    pub(crate) fact_stores: BTreeMap<FactFamily, FactStoreEntry>,
    pub(crate) path_contexts: Option<crate::path_context::PathContextIndex>,
    pub(crate) changeset: Option<ReviewChangeset>,
}
```

The clean cut: the SDK owns a `FactSnapshot { files, stable_key_table, path_contexts, changeset, families: … }` with all the read accessors; the engine's `AnalysisDb` **contains** a `FactSnapshot` plus the mutable stores and the live interner, and produces the snapshot at the end of the kernel run. This is the single riskiest refactor in the plan and is gated by experiment **E2**.

### 4.3 Is rules-as-Rust compatible with *no scan-time Cargo at all*?

**Answer, precisely: yes for the customer, no for the world.** Native Rust cannot execute without being compiled for some target by some compiler. The question is only *where* that compilation happens and *what portability and trust costs* the location imposes.

There are exactly four places compilation can live, and they are not equivalent:

| # | Where compilation happens | Customer needs a Rust toolchain? | Customer needs `cargo` at scan time? | Portability cost | Trust cost |
|---|---|---|---|---|---|
| **P1** | Customer machine, whole engine (**today**) | Yes | Yes, every scan | None (source is portable) | Executes repo `build.rs` + proc macros + binary, full privileges |
| **P2** | Customer machine, **rule crate only, against a thin SDK** (recommended) | Yes | Yes on rule change; **no** when the fingerprint matches | None | Same class as P1 but a far smaller, gateable manifest |
| **P3** | Customer machine, **rule crate only, against prebuilt SDK rlibs, driven by `rustc` directly** | Yes, and **the exact same `rustc` build** | No — no Cargo, no registry, no lockfile | Breaks on any `rustc` mismatch; needs artifacts per (version × target × rustc) | Best of the compile-on-customer options: no `build.rs`, no registry, one crate |
| **P4** | **Author/CI machine**; customer receives a compiled artifact | **No** | No | Native artifact = one per target (5 today); **WASM = one artifact for all targets** | Customer executes someone else's compiled code → needs signing, and ideally a sandbox |

Two things follow, and they should be stated to users in exactly these words:

1. **P4 is the only path with zero customer compilation, and it necessarily moves compilation to the author.** That is not a loophole; it is the honest statement. It is also completely compatible with "rules are code": the *source of truth* is still Rust in the repo; the artifact is a build output, exactly like the `polint` binary itself is a build output of this repository (`.github/workflows/release.yml:124-196`).
2. **Compiling is itself the security boundary, not just running.** `cargo build` executes `build.rs` and procedural macros at compile time with the invoking user's privileges. So "compile untrusted Rust safely, then sandbox the run" is *not a coherent plan* unless the compile is also constrained. This is treated in §10.

**Portability costs, concretely:**

- **P3** binds polint to an exact `rustc` build. The Rust reference is unambiguous that there is nothing to rely on here: *"Type layout can be changed with each compilation. Instead of trying to document exactly what is done, we only document what is guaranteed today."* — [Type layout](https://doc.rust-lang.org/reference/type-layout.html). `rustc_metadata` carries a `METADATA_HEADER` including a `METADATA_VERSION` ([rustc_metadata](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_metadata/index.html)); that page makes **no** stability claim, which is itself the point. `-C metadata` exists precisely "to differentiate symbols between two different versions of the same crate being linked" ([codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html)). Practical consequence: prebuilt rlibs must be keyed on `rustc -vV` verbatim and must have a source-compile fallback.
- **P4 native** requires cross-building 5 targets (the release matrix at `.github/workflows/release.yml:129-155`: `x86_64`/`aarch64-unknown-linux-gnu`, `x86_64`/`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`). Cross-compiling macOS from Linux is the awkward one; polint solves it today by using macOS runners.
- **P4 WASM** collapses that to one artifact. `wasm32-wasip2` is **Tier 2** with `std`, emits components, needs a component-model runtime (Wasmtime 17+), defaults to `-Cpanic=abort`, and is *"not tested in CI at this time"* per [platform support](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html). `wasm32-wasip1` is the more conservative choice today. WASM is 32-bit: a fact snapshot must fit in a 4 GiB linear memory, and a copy into guest memory is required.

**Bottom line for the product:** target **P2 as the default**, make **P3 an opportunistic accelerator**, and make **P4 the default for scenarios B and C**. Do not promise "no Rust toolchain" for scenario A unless you are willing to make P4 mandatory there too — which would cost the edit-and-run loop that makes rules-as-code pleasant.

### 4.4 Which design is actually optimal

**Optimal: prebuilt host + small code artifact — where "small code artifact" is a native rule binary built against a thin SDK, driven by a fact-snapshot protocol.** A WASM backend is a valuable *second* target, not the primary one. Native rule packs and host-managed rule *processes with query round-trips* are both rejected. Reasoning is in §6; the comparison matrix is §6.10.

The decisive arguments:

1. It is the only option that **removes 222 of 223 compilation units without changing one character of rule source**, because the macro already emits only `::polint::sdk::*` paths and the views are already `&db` projections.
2. It **preserves borrowed-slice fact access** (`-> &'a [SymbolFact]`), because the rule process deserializes the snapshot once into owned memory and the views borrow *that*. A query-per-call RPC design cannot do this and would force every accessor to return owned data — an API break for every existing rule.
3. **Both ends of the wire already exist** as versioned public JSON schemas (rule-inspect v1, report v1) and there is a working precedent for the stdio protocol in `analysis/extensions/host.rs`.
4. It **inverts the process split in the direction that already has the cache**: `.polint/cache/analysis|layers|derived|semantic-store` is the host's, and today the host hands it to a child via `POLINT_CACHE_DIR` (`cli/mod.rs:4295-4297`). Moving analysis into the host removes that hand-off entirely.
5. It **makes the WASM backend cheap later**, because the SDK/host boundary is the same one WASM needs. Doing WASM first would require the same split anyway *plus* a runtime, a new target, and a sandbox model.

The honest cost, stated plainly: **one serialize + one deserialize per scan** that does not exist today. Whether that is acceptable is an empirical question with a defined kill criterion (E3, §9).

### 4.5 Preserving typed facts, capabilities, diagnostics, tests, determinism, and AI-friendliness

| Property | Mechanism today | Under the recommendation |
|---|---|---|
| **Typed facts** | `FactView::build(&AnalysisDb)`; views return `&'a [T]` | Identical. `AnalysisDb` → SDK-owned `FactSnapshot`; `sdk::__private::AnalysisDb` re-exports it so macro output is unchanged. Views still return `&'a [T]`, borrowing the deserialized snapshot. |
| **Capability derivation** | Macro maps parameter type → capability method + `FactViewRequirement` (`polint-macros/src/lib.rs:313-324`) | Identical. Capabilities are compile-time metadata in the rule binary; the host reads them via the existing `inspect rule` JSON (schema `polint-rule-inspect-v1.json`) and plans from them — which is *more* honest than today, because the host now provably materializes only the planned closure. |
| **Capability diagnostics** | `Supported`/`Unsupported`/`SetupMissing` + `polint/capability` (`ARCHITECTURE.md:228-233`) | Identical; `CapabilitySupportView` travels in the snapshot header. Blocked rules still do not run on fabricated facts. |
| **Diagnostics** | `Diagnostic` built in the rule; renderers in the host | Improved separation: `Diagnostic` + report JSON in the SDK; human/GitHub/SARIF/ai-friendly renderers stay host-side (shrinking the SDK). |
| **Rule tests** | `polint test` copies fixtures then `cargo run`s the pack per case (`rule_test.rs:323-373`) | Faster: build the rule binary **once**, then per fixture snapshot + run. This is the biggest single ergonomics win of the change. |
| **Determinism** | Stable ordering, resolved stable-key text, digest inputs (`ARCHITECTURE.md:370-390`) | Strengthened. The snapshot is a *materialized, digestible artifact* of the run. Include its digest in the report and gate byte-equality across the two execution modes (E4). |
| **AI-friendly authoring** | `use polint::sdk::prelude::*;`, `#[polint::rule]`, `polint new-rule`, generated skill, `--format ai-friendly` | Unchanged — rule sources are byte-identical. Agents get *better* behaviour: no multi-minute silent `cargo` build on first `polint check`, and offline operation. |
| **No DSL as primary model** | — | Held. §2. |

**One thing does change for authors**, and it should be documented rather than hidden: the rule binary no longer has ambient access to the repository during rule execution. Today a rule *could* read arbitrary files or spawn processes because it is an ordinary binary with the user's privileges. That was never a supported capability (`RuleCtx` "is not a back door to the entire fact database" — `ARCHITECTURE.md:133-135`), but it was reachable. Under the snapshot protocol it stays reachable in native mode and becomes *unreachable* in WASM mode. Say so.

### 4.6 Smallest viable architecture and migration path

See §7 (architecture) and §8 (phases). Summary of the minimum:

- **+2 workspace packages** (`polint-sdk`, `polint-engine`); `polint` becomes a thin facade. Publishable goes from 2 to **3** — `polint-engine` starts `publish = false` and is promoted only if a third party ever needs to build a host (`IMPLEMENTATION-PLAN.md` §3.3, §13.3 D-5).
- **1 new wire object** (the fact snapshot). The other two wire objects already exist as versioned schemas.
- **1 manifest line changes** in generated rule packs: the `polint` dependency gains Cargo's `package = "polint-sdk"` rename, keeping the extern crate name `polint` (`IMPLEMENTATION-PLAN.md` §11.4). The scaffolder already emits the rest of that line (`cli/mod.rs:1137-1144`).
- **0 changes** to rule source files, `.polint.toml`, profiles, baselines, ignores, or report schemas.

### 4.7 Experiments and budgets

See §9. Every number there is a **budget or a kill criterion**, not a result. The measured numbers are the two in §3.3: the repository's documentation figure (223 units / 185.4 s / 562 MB → 537 MB / 0.7 s rebuild, `docs/GITHUB-ACTION.md:162-166`) and the committed `build-cost` baseline (225 units, 187.3 s, one Cargo start, 582.7 MB retained on `examples/basic`).

---

## 5. Product scenarios and constraints

### Scenario A — a team authors and reviews its own Rust rules, then scans locally and in CI

Trust: **implicit**. The rules are in the repo the team owns and reviews; a malicious rule is a malicious commit, and the team's existing code-review process is the control.

What matters: **edit-and-run latency**, first-clone latency for a teammate who never edits rules, laptop disk, and CI minutes.

Today, measured (§3.3.2): the first clone pays 187.3 s and 582.7 MB; every scan spawns `cargo` — 157 ms of freshness checking even when nothing changed; offline is impossible.

**Recommended default: P2** — native rule binary against the thin SDK, snapshot protocol, fingerprint gate so unchanged rules never invoke `cargo`. **Plus**: CI publishes the built rule artifact (§P4) so teammates who never touch rules never compile anything.

### Scenario B — a customer receives a prebuilt or shared rule pack from another party

Trust: **explicit and bounded**. The customer chose the supplier (a platform team, a vendor, an open-source policy pack) but did not review the code line by line.

What matters: **zero build**, integrity, provenance, and running on whatever OS/arch the customer has.

**Recommended default: P4** — a signed artifact plus the Rust source for review. Interim implementation: the author's CI cross-builds the 5 release targets and publishes `rules-<os>-<arch>.tar.gz` + `.sha256`, exactly the mechanism polint already uses for its own CLI (`scripts/install.sh:29-66`, `action.yml:70-160`). Target state: **one WASM module** — one artifact for all platforms, and sandboxable.

Verification is non-negotiable here: signature (or at minimum a pinned digest in `.polint.toml`), plus a recorded `polint` SDK-ABI version the artifact was built against.

### Scenario C — an agent scans an arbitrary fresh repository that may contain untrusted rules

Trust: **none**. The repository is attacker-controlled input.

This is the scenario the current architecture handles worst, and it deserves to be stated bluntly:

> **Today, running `polint check` on an untrusted checkout that contains `.polint/rules` is arbitrary code execution.** `cargo run` compiles and runs repo-controlled code with the user's full privileges (`cli/mod.rs:4264`). `build.rs` and procedural macros execute at **compile** time. `.polint/extensions/*` does the same (`analysis/extensions/host.rs:128-162`). The generated agent skill even grants `Bash(cargo:*)` (`cli/skill.rs:185`).

This is the same trust posture as `npm install` or `cargo test`, and it is defensible *for repos you chose to clone*. It is not defensible for an agent that clones arbitrary repositories.

**Recommended default: do not build and do not execute repo rules.** Run only the non-rule surfaces (`polint facts list`, `polint inspect`, metrics, discovery) and report clearly that rule execution was skipped because the repository is untrusted. Building requires an explicit, persisted decision — `polint trust <repo>` keyed on remote + commit, in the spirit of editor workspace trust. Even then, apply manifest lockdown (§10) and `--offline`. Once the WASM backend exists, offer `--trust-rules=sandbox` for compile-with-lockdown + run-in-sandbox.

### Cross-cutting constraints

- **Low-powered machines are the acceptance environment.** 2 vCPU / 4 GB / cold page cache. A build that takes 185 s on a fast x86_64 workstation is materially worse there — Cargo parallelism is the thing that hides the cost, and it is exactly what a small machine lacks.
- **Minimal customer disk.** 582.7 MB per repository is the measured retained cost (§3.3.2); a developer with five polint-enabled repos pays it five times unless they share a target dir.
- **Offline / air-gapped.** Must work. Today it cannot.
- **Determinism is a product property** (`ARCHITECTURE.md:370-390`) and cannot regress.
- **Two publishable packages was a deliberate outcome**, guarded by a test (`internal_architecture.rs:14-33`). Any proposal must justify each new package individually.

---

## 6. Code-preserving alternatives comparison

Each option below is assessed against: *does rule source change?*, *what does the customer compile?*, *cold latency*, *retained bytes*, *offline*, *portability*, *security*.

### 6.1 Option A — Status quo

Rule source: unchanged. Customer compiles: everything.

Cold: 187.3 s / 225 units / 582.7 MB retained. Warm no-op: 157 ms, one Cargo start, zero units. Warm rule edit: 735 ms, one unit (§3.3.2). Offline: no. Portability: excellent (source is portable; any `rustc ≥ 1.95` works). Security: RCE-by-checkout.

**Rejected as a destination.** It is, however, the correct *fallback mode* to retain behind `POLINT_RULES_MODE=in-process` for the polint repository's own tests and for users who hit a snapshot-protocol limitation.

### 6.2 Option B — Thin SDK + prebuilt engine host + fact-snapshot protocol ★ recommended

**Rule source: byte-identical.** Customer compiles: one crate + ~25–35 SDK units.

The package shape (§7.1) matters and has one non-obvious constraint: **Cargo forbids cyclic package dependencies**, so `polint` cannot both be the thin SDK *and* optionally depend on an engine that depends back on `polint`. The resolution is a three-package graph with no cycle:

```
polint-sdk   ← polint-engine
     ↑              ↑
     └──── polint ──┘   (polint: pub use polint_sdk as sdk; optional dep on polint-engine
                          behind default feature "engine", which also enables [[bin]] polint)
```

- Rule pack: `polint = { version = "0.3", default-features = false }` → closure is `polint-sdk` + `polint-macros` only.
- `cargo install polint` → default features → `polint-engine` → the `polint` binary. Unchanged for users.
- Feature unification cannot leak the engine into a rule pack, because the scaffolded pack is **its own workspace** (`[workspace]` at `cli/mod.rs:1156`) and *"Features for target-specific dependencies are not enabled if the target is not currently being built"* / resolver v2 semantics apply ([Cargo resolver](https://doc.rust-lang.org/cargo/reference/resolver.html)); the workspace already uses `resolver = "3"` (`Cargo.toml:29`). It does **not** hold for this repository's own 17 example packs, which are workspace members and would have `engine` unified back on — see the note at the head of §7.1. This must still be **asserted by a test** (§7.4) rather than assumed.

**Alternative single-package shape**, worth recording: keep two packages and `#[cfg(feature = "engine")]` the heavy modules inside `polint`, with every heavy dependency `optional = true`. `crates/polint/src/lib.rs:19-58` already declares every module in one place, so the cfg edit is ~25 lines. This preserves the "two publishable packages" invariant exactly. Costs: a 238 KLOC crate with a combinatorial feature matrix; slower `cargo check --all-features`; and the rule pack still *downloads* the full crate source even though it compiles little of it. **Recommend the three-package split; record this as the fallback if crates.io naming or publishing is a problem.**

Cold: target ≤ 20 s on 2 cores (E1/E7). Retained: target ≤ 120 MB. Offline: **yes**, with the vendored-SDK step. Portability: unchanged from today (source-portable, any `rustc ≥ MSRV`). Security: same class as today, but the manifest is now small enough to lock down meaningfully (§10).

**Costs and risks, stated:** one serialize + deserialize per scan (E3); the `core/db.rs` read/write split (E2); the preview policy views (`Events`, `Calls`, `DataFlow`) need their derived facts in the snapshot or a host-side evaluation escape hatch.

### 6.3 Option C — Precompile/cache the engine as a reusable Cargo artifact

Three genuinely different things get conflated under "cache the build." Separating them is the point of this subsection.

**C-i. Compiler cache reuse (`sccache` via `RUSTC_WRAPPER`).** Caches individual `rustc` invocations keyed on inputs and flags; shares across workspaces and worktrees. The Cargo book recommends exactly this for cross-project sharing ([build cache](https://doc.rust-lang.org/cargo/reference/build-cache.html)), and polint's own `.cargo/config.toml:1-8` adopts it for contributors.
*What it fixes:* the second and later cold builds on one machine, and cross-repo reuse for a developer with several polint repos.
*What it does not fix:* the very first build on a fresh machine or container; a cloud sccache would, but that is infrastructure a customer must operate.
*Disk:* it **adds** a second copy — polint's config caps it at 10 GiB.
**Verdict: complementary, never the answer.**

**C-ii. Final target retention (`.polint/cache/rules-target`, 582.7 MB measured, §3.3.2).** This is what makes the 0.7 s rebuild possible; it is also the whole disk problem. The GitHub Action already keys it correctly on compiler inputs and prunes the rule package's own output before saving (`docs/GITHUB-ACTION.md:95-171`, commit `3c0f4dfa`). That work is well-designed and should be preserved — but it is *managing* a cost, not removing it. Under Option B most of it becomes unnecessary.

**C-iii. Shipping prebuilt engine/SDK artifacts.** Two sub-variants:

- **C-iii-a: ship a populated target directory.** Cargo re-fingerprints every unit and `.d` dep-info files record absolute paths; relocation across machines is fragile. **Reject.**
- **C-iii-b: ship an rlib bundle and drive `rustc` directly.** A rule pack is *one crate with one external dependency*. So:
  ```
  rustc --edition 2024 --crate-type bin -O \
        --extern polint=<sdk>/libpolint_sdk-<hash>.rlib \
        --extern polint_macros=<sdk>/libpolint_macros-<hash>.so \
        -L dependency=<sdk>/deps -o <out> .polint/rules/src/main.rs
  ```
  No Cargo, no lockfile, no registry, no `build.rs`. This is the **fastest possible P2/P3 path** and is a genuinely attractive accelerator.
  *Blocker:* rlib/rmeta compatibility is tied to the exact compiler. Primary evidence: *"Type layout can be changed with each compilation"* ([type layout](https://doc.rust-lang.org/reference/type-layout.html)); `-C metadata` exists to disambiguate symbols between crate versions ([codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html)); `rustc_metadata` carries a `METADATA_VERSION` and documents no stability ([rustc_metadata](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_metadata/index.html)). Proc-macro `.so`s are host shared objects loaded by `rustc` — same constraint.
  *Combinatorics:* 5 targets × N `rustc` versions. `polint init` already pins `rust-toolchain.toml` (`cli/mod.rs:723-742`), which helps — but it pins `channel = "1.95"`, and rustup resolves that to the newest 1.95.x, so artifact reuse would additionally require pinning `1.95.0` exactly.
  *A linker (`cc`/MSVC) is still required.*
  **Verdict: Phase-3 fast path with mandatory source fallback. Not a foundation.** Kill criterion in §9.

**A cheap, high-value cousin worth doing regardless: vendor the SDK source into the polint binary.** Materialize it to `.polint/cache/sdk/<version>/`, point the rule pack at it via a path dependency or `[patch]`, and build `--offline --locked`. This does not reduce compile time at all, but it (a) makes first-run offline operation possible, (b) guarantees the SDK version matches the CLI exactly, and (c) removes the registry as a supply-chain surface for the rule build. It is Phase 2 and it is small.

### 6.4 Option D — Native plugins: cdylib rule packs loaded by a generic prebuilt host

Build the rule pack as a `cdylib` exporting an `extern "C"` registration entry point; the prebuilt host `dlopen`s it.

**Rejected. Four independent reasons, any one sufficient:**

1. **It destroys the typed API.** Rust has no stable ABI. Only `#[repr(C)]` types may cross a `dlopen` boundary safely; `String`, `Vec<T>`, `Option<NonNull>` niches, `&[T]`, and trait objects may not. Making the fact model FFI-safe means rewriting `SymbolFact` and every sibling as `repr(C)` with raw pointers and lengths, or wrapping everything in an `abi_stable`-style shim. Either way, rules stop being ordinary typed Rust — a direct violation of the product invariant in §2. ([Type layout](https://doc.rust-lang.org/reference/type-layout.html); [linkage](https://doc.rust-lang.org/reference/linkage.html) describes `cdylib` as *"used when compiling a dynamic library to be loaded from another language"* — which is the correct framing: it is a C boundary, not a Rust one.)
2. **It does not remove compilation.** The customer still compiles the rule crate. Only the artifact kind changes.
3. **It is worse for security.** `dlopen` of a repo-supplied binary has no compile-time gate at all, and forecloses any future sandbox.
4. **It contradicts the repository's own contracts.** `ARCHITECTURE.md:499-500` lists *"promising a public ABI for dynamically loaded providers"* as an enduring non-goal, and `unsafe_code` is `forbid` at the workspace level / `deny` in `polint` (`Cargo.toml:82`, `crates/polint/Cargo.toml:92-94`) — `libloading` requires host-side `unsafe`.

*Ecosystem note (unverified in this environment — see §13.3):* Go's `plugin` package has similar constraints, and golangci-lint's current supported extension mechanism is understood to be "module plugins," which **rebuild the golangci-lint binary with your linters linked in** rather than loading `.so` files. If accurate, that is a mature ecosystem arriving at the same conclusion: for a statically-linked analysis tool, *relink* beats *dlopen*. Verify at <https://golangci-lint.run/plugins/module-plugins/>.

*Ecosystem note (unverified):* Trail of Bits' **Dylint** does load Rust lints as dynamic libraries, but against `rustc_private` on a **pinned nightly per library**, with the toolchain baked into the library filename. That is the price of the approach and it is not one polint should pay. Verify at <https://github.com/trailofbits/dylint>.

### 6.5 Option E — Compile rules to WASM, preserving Rust as the authoring model

Build `.polint/rules` for `wasm32-wasip1` (or `wasm32-wasip2`) and run it inside a runtime embedded in the prebuilt host.

**Assessment: excellent second backend, wrong primary.**

*Preserves the model:* yes, and better than people expect. The guest links `polint-sdk`, receives the snapshot bytes in guest memory, deserializes into owned guest memory, and hands out `&'a [SymbolFact]` exactly as today. **The typed API survives intact** because the deserialize-once-then-borrow shape is the same as Option B. This is only true because we do *not* try to expose facts as host-callback RPC.

*Wins:*
- **One artifact for every platform.** This is the decisive advantage for scenario B and it is not available any other way.
- **A real sandbox.** WASI is capability-based; a rule module gets no filesystem, no network, and no process spawning unless the host grants it. This is the only option that materially improves scenario C's *execution* posture.
- Deterministic execution semantics.

*Losses:*
- **It does not remove customer-side compilation for scenario A.** You still need `rustc` plus `rustup target add wasm32-wasip1` (an extra download) and you still compile the SDK for the wasm target.
- **Compiling to WASM does not sandbox the compile.** `build.rs` and proc macros still run natively at full privilege. Manifest lockdown is required regardless (§10).
- **Runtime weight in the polint binary.** Embedding a WASM runtime is a large dependency for a tool that currently ships a single static binary.
- **32-bit guest.** A fact snapshot must fit in a 4 GiB linear memory, and it must be *copied* into that memory — a second copy on top of the snapshot itself. On the largest monorepos this is the binding constraint.
- **`wasm32-wasip2` is Tier 2, emits components, needs Wasmtime 17+, defaults to `-Cpanic=abort`, and is documented as not CI-tested** ([platform support](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html)). `wasm32-wasip1` is the safer near-term target.
- **Module instantiation cost.** JIT-compiling a multi-MB module on every scan would be self-defeating; an AOT-compiled module must be cached under `.polint/cache`. *Wasmtime's `Module::serialize`/`deserialize` and `wasmtime compile` are the standard mechanism, and precompiled artifacts are runtime-version and target specific — **unverified in this environment**; check <https://docs.wasmtime.dev/> and <https://docs.rs/wasmtime>.*

**Verdict: Phase 4, behind `[rules] target = "wasm"`, as the default artifact format for scenario B and the sandbox mode for scenario C. Not the answer to scenario A's cold-start problem.**

### 6.6 Option F — Hybrid: rules stay Rust, built in CI/author environments, shipped as signed portable code artifacts

This is the **only** design with literally zero customer compilation, and it composes with B and E rather than competing.

Division of labour:

| | Author / CI | Customer |
|---|---|---|
| Writes Rust rule source | ✔ | reads and reviews it |
| Runs `polint rules build` | ✔ | — |
| Needs a Rust toolchain | ✔ | **✘** |
| Signs the artifact | ✔ | verifies signature/digest |
| Publishes | ✔ (release asset / OCI / internal artifact store) | resolves and caches under `.polint/cache/rules-artifact/` |
| Runs `polint check` | ✔ | ✔ |

Mechanically this is a small feature on top of Option B: add `polint rules build --target <triple>|wasm --out <path>`, add `[rules] artifact = { url|path, sha256, sdk_abi }` to `.polint.toml`, and make the host prefer a verified artifact over building. polint already operates exactly this pattern for its own CLI — 5-target matrix, `.tar.gz` + `.sha256`, checksum verification in both `scripts/install.sh:52-66` and `action.yml:130-150`.

**Recommended defaults:** mandatory for scenario B; recommended for scenario A's non-authoring teammates and CI; the only *safe* rule execution for scenario C (paired with a sandbox).

**Do not make it mandatory for scenario A authors** — it would destroy the edit-and-run loop that makes rules-as-code good.

### 6.7 Option G — Compiler-level mitigations only

| Mitigation | Effect | Verdict |
|---|---|---|
| `POLINT_RULES_PROFILE=dev` (already exists, `cli/mod.rs:4425-4452`) | dev is `opt-level=0`, `incremental=true`, `codegen-units=256` ([profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)) — faster builds, slower rule execution | Already shipped; a knob, not a fix. |
| Incremental compilation | Helps rebuilds, not first build; *"inhibits certain optimizations and is not recommended for release builds"* ([codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html)); polint's own repo disables it because artifacts *"can dwarf the actual build outputs"* (`.cargo/config.toml:1-4`) | Worsens the disk goal. |
| `sccache` | §6.3 C-i | Complementary. |
| `cargo-binstall` / `cargo-dist` | Distributing the **polint binary**, which polint already does well (release matrix + install script + action) | Marginal. |
| PGO | Real speedups for the *engine at runtime*, via `-Cprofile-generate` / `llvm-profdata merge` / `-Cprofile-use`; *"All `rustc` flags must match between instrumentation and optimization phases"* ([PGO](https://doc.rust-lang.org/rustc/profile-guided-optimization.html)) | Worth doing for the **prebuilt host binary** in the release pipeline. Irrelevant to build cost. |
| LTO / `codegen-units=1` | Slower builds for faster runtime | Wrong direction for the rule pack; possible for the released host. |
| Cranelift codegen backend | Faster debug builds | Requires a nightly component; incompatible with a stable-toolchain product promise. Reject. |
| `-Z bindeps`, `-Z build-std` | Would make "rule crate produces a cdylib/wasm artifact consumed by the host" ergonomic | **Nightly only** ([unstable features](https://doc.rust-lang.org/cargo/reference/unstable.html)). Reject for the product path. |

**Verdict: no combination of these changes the order of magnitude.** The problem is 222 units of engine, and no compiler flag deletes them.

### 6.8 Option H — Daemon / server

Keep a long-lived polint process with a warm `AnalysisDb`.

Helps: repeated scans, editor latency, watch mode. Does not help: the *cold* scan, which is the stated requirement. Also collides with `ARCHITECTURE.md:503-505`, which lists demand-driven/editor-latency execution as an explicit non-goal today.

**Verdict: defer. Revisit after Option B, at which point a daemon becomes a small addition (the host is already the process that owns the facts).**

### 6.9 Option I — Host-managed rule process with query round-trips

Keep facts in the host; the rule process asks for them over stdio (LSP-shaped).

**Rejected on API grounds.** Fact views return borrowed slices (`fn all(self) -> &'a [SourceFile]`, `sdk/facts.rs:29-31`). A lazy RPC view cannot return a borrow into data it has not received. Every accessor would have to return owned `Vec<T>`, breaking every existing rule and every example. It also multiplies syscalls in the hot loop — rules call `file_in_scope` once per fact row, which is why `sdk/scope.rs:41-64` memoizes glob compilation in the first place.

Bulk snapshot + deserialize-once (Option B) is strictly better for this API shape.

### 6.10 Decision matrix

Scoring: ● strong · ◐ partial · ○ weak/absent.

| | A. Status quo | **B. Thin SDK + snapshot ★** | C-iii-b. Prebuilt rlibs | D. cdylib plugin | E. WASM | F. Author-built artifact | G. Compiler flags | H. Daemon | I. RPC views |
|---|---|---|---|---|---|---|---|---|---|
| Rule source unchanged | ● | ● | ● | ○ (repr(C)) | ● | ● | ● | ● | ○ |
| Typed borrowed-slice API preserved | ● | ● | ● | ○ | ● | ● | ● | ● | ○ |
| Capability derivation preserved | ● | ● | ● | ◐ | ● | ● | ● | ● | ● |
| Removes engine compile on customer | ○ | ● | ● | ○ | ● | ● | ○ | ○ | ● |
| Removes *all* customer compile | ○ | ○ | ○ | ○ | ○ | ● | ○ | ○ | ○ |
| Cold-scan latency | ○ | ● | ● | ◐ | ◐ | ● | ◐ | ○ | ● |
| Retained disk | ○ | ● | ● | ◐ | ◐ | ● | ○ | ○ | ● |
| Offline first run | ○ | ● (vendored SDK) | ● | ◐ | ◐ | ● | ○ | ○ | ● |
| Cross-platform robustness | ● | ● | ○ (rustc-pinned) | ○ | ● | ◐ native / ● wasm | ● | ● | ● |
| Sandbox for untrusted rules | ○ | ○ | ○ | ○ | ● | ◐ (needs sandbox too) | ○ | ○ | ○ |
| Determinism preserved | ● | ● | ● | ◐ | ● | ● | ● | ◐ | ● |
| Implementation risk | — | ◐ (db split, snapshot volume) | ◐ | ● high | ◐ | ○ low | ○ low | ◐ | ● high |
| Workspace-package delta (published) | 0 | +2 (+1) | +0 (with B) | +1 (+1) | +0 (with B) | 0 | 0 | 0 | +2 (+1) |

**Explicit rejections, one line each:**

- **DSL / declarative-first** — rejected by the product invariant (§2). Optional generator only.
- **cdylib / native plugin** — no stable Rust ABI, forces `repr(C)` facts, does not remove compilation, worse security, contradicts `ARCHITECTURE.md:499-500` and the `unsafe_code` lint policy.
- **WASM-first** — does not remove scenario-A compilation, adds a runtime and a target, 32-bit snapshot limit, and needs the same SDK split anyway.
- **Ship a populated target dir** — Cargo fingerprints + absolute dep-info paths make relocation fragile; 582.7 MB.
- **`-Z bindeps` / `-Z build-std`** — nightly only ([unstable features](https://doc.rust-lang.org/cargo/reference/unstable.html)).
- **Cranelift backend** — nightly component.
- **sccache / incremental / profile tuning alone** — do not change the order of magnitude; incremental worsens disk.
- **Daemon as the fix** — helps warm, not cold; explicit non-goal today.
- **RPC fact views** — breaks the borrowed-slice API and every existing rule.
- **Resurrect the 8-crate split** — already rejected in-repo and guarded by a test; one boundary is enough.

---

## 7. Recommended target architecture

### 7.1 Packages

**Superseded in one respect — read `IMPLEMENTATION-PLAN.md` §3.1 for the decided
graph.** The shape below reaches the thin closure by having rule packs depend on
the `polint` facade with `default-features = false`, so the engine arrives only
through an optional dependency behind a default `engine` feature. That works for
a scaffolded pack, which is its own workspace. It does **not** work for the 17
example packs in *this* repository, which are workspace members alongside the
`polint` binary: Cargo unifies features across the workspace members being built,
so `cargo build --workspace` would re-enable `engine` for every example pack and
the guard in §7.4 would fail for a reason no rule author could act on. The plan
therefore has rule packs depend on `polint-sdk` directly, renamed to `polint` with
Cargo's `package =` key — which is immune to feature unification because it is a
different package, and still leaves rule `.rs` sources byte-identical. Everything
else below stands.

```
polint-macros   (unchanged)  proc-macro; syn/quote/proc-macro2
polint-sdk      (NEW)        fact types · fact views · policy queries · Diagnostic + report JSON
                             RuleCtx · Rule · Capabilities · RuleOptions · scope helpers
                             FactSnapshot (owned read model) · runner protocol client
                             deps: serde, serde_json, globset(?)          ~14–18 KLOC
polint-engine   (NEW = today's crate, renamed)
                             frontends · kernel · providers · cache · config · CLI · renderers
                             deps: oxc*, tree-sitter*, rusqlite, rayon, petgraph, clap, ignore, …
                             depends on: polint-sdk                       ~220 KLOC
polint          (facade)     pub use polint_sdk as sdk;
                             pub use polint_sdk::runner;
                             pub use polint_macros::rule;
                             [features] default = ["engine"]; engine = ["dep:polint-engine"]
                             [[bin]] name = "polint", required-features = ["engine"]
```

- Rule packs: `polint = { version = "0.3", default-features = false }`.
- `cargo install polint` unchanged.
- `use polint::sdk::prelude::*;` and `polint::runner::run_cli(vec![...])` unchanged.
- Macro output unchanged (it emits `::polint::sdk::*` only).
- No dependency cycle.

### 7.2 The protocol

Three messages; **two of the three schemas already exist and are public.**

```
┌�� polint (prebuilt binary, downloaded) ──────────────────────────────────┐
│ 1. discover rule packs from [rules] paths                               │
│ 2. ensure rule binary is current:                                       │
│      fingerprint = H(rule sources, pack manifest, SDK version,          │
│                      rustc -vV, profile, target)                        │
│      if cached binary matches -> DO NOT SPAWN CARGO                     │
│      else -> build (cargo --offline --locked, or direct rustc fast path)│
│ 3. rule-binary `manifest`  ──────────────► stdout: rule-inspect-v1 JSON │   EXISTS
│ 4. plan capabilities; run providers; write snapshot                      │
│      .polint/cache/facts/<plan-digest>.polintfacts                       │   NEW
│ 5. rule-binary `run --facts <path> --options <path> --kind check|review` │
│                              ──────────► stdout: polint-report-v1 JSON  │   EXISTS
│ 6. ignores, baseline, filters, renderers, exit code                      │
└──────────────────────────────────────────────────────────────────────────┘
```

Steps 3 and 5 can be one process with a stdio handshake, exactly as `analysis/extensions/host.rs:56-126` already does (`handshake` then `run_provider`), avoiding a second spawn. Reuse the timeout/stdout-limit/stderr-limit discipline from `host.rs:17-19`.

**Snapshot v1 contents:**

| Section | Notes |
|---|---|
| Header | `schema_version`, `polint_version`, `sdk_abi_version`, `plan_digest`, `config_digest`, `rule_digest` |
| `capability_support` | the existing `CapabilitySupportView` — honesty model travels with the data |
| `files` | `Vec<SourceFile>`; `FileId` is an index |
| `stable_keys` | resolved key text table; `StableKeyId` indexes it (matches `ARCHITECTURE.md:237-264`) |
| `path_contexts` | for `RuleCtx::path_context_related` |
| `changeset` | `polint review` diff, `None` under `check` |
| one section **per requested fact family** | **only the planned closure is serialized** |

Format: v0 JSON (correctness and debuggability), v1 a compact length-prefixed binary. Digest the snapshot and include the digest in the report — this makes the analysis→rule boundary independently auditable, which is a determinism *improvement*.

**Why the borrowed-slice API survives:** the rule process deserializes once into an owned `FactSnapshot` that lives for the whole run; `FactView::build(&snapshot)` yields views that borrow *it*. `SourceFiles::all(self) -> &'a [SourceFile]` is unchanged.

### 7.3 Execution modes

| Mode | Selection | Used by |
|---|---|---|
| `snapshot` (default after Phase 3) | default | A, and B/C via artifacts |
| `in-process` | `POLINT_RULES_MODE=in-process`, rule pack with `features = ["engine"]` | polint's own tests; escape hatch if E3 fails on a specific repo |
| `artifact` | `[rules] artifact = { … }` | B; A's non-authors and CI |
| `wasm` (Phase 4) | `[rules] target = "wasm"` | B default artifact; C sandbox |

### 7.4 New invariants to encode as tests

The repository's strength is that architecture claims are gated by tests. Add three:

1. **Closure gate.** Assert that the dependency closure of `polint --no-default-features` for each released target stays at or under a fixed package count and contains **none** of: `oxc_*`, `tree-sitter*`, `rusqlite`, `libsqlite3-sys`, `clap`, `tracing-subscriber`, `rayon`, `petgraph`, `ignore`, `toml`, `serde_norway`. This is the single most valuable regression gate in the whole plan — it prevents the split from silently eroding.
2. **Feature-leak gate.** Build a temp-repo rule pack outside the workspace and assert its lockfile does not contain the engine.
3. **Mode-equivalence gate.** Extend the existing golden and determinism suites (`cargo test -p polint --test golden`, `--lib eval::determinism_gate`) to run every case in both `in-process` and `snapshot` mode and assert byte-identical diagnostics.

Update `crates/polint/tests/internal_architecture.rs:14-33` from "exactly two publishable product packages" to three, keeping `REMOVED_PACKAGES` intact so the old split cannot creep back.

---

## 8. What changes in the repository, what stays, and the phased plan

### 8.1 Stays exactly as-is

- **Every rule source file**, in `examples/**/.polint/rules/src/*.rs` and in customer repos. Byte-identical.
- `#[polint::rule]`, the prelude, `run_cli(vec![...])` registration, `RuleCtx`, `RuleOptions::settings`.
- Capability derivation from fact-view parameters; `polint/capability` diagnostics; precision/status honesty.
- `.polint.toml` — workspace/include/exclude, `[profiles.*]`, `[[rules.config]]`, `[ignores]`, `[sarif.rule_help_uri]`, `[path_contexts]`, `[languages.go]`.
- Baselines, comment ignores, `polint test` fixture format, `polint inspect rule` / `facts` / `unknowns` / `explain` JSON schemas, report JSON / SARIF / GitHub / ai-friendly formats.
- The GitHub Action's public inputs and outputs.
- Determinism rules and the golden corpus.
- `REMOVED_PACKAGES` — the 8-crate split stays dead.

### 8.2 Changes

| Area | Change |
|---|---|
| `Cargo.toml` | members += `crates/polint-sdk`, `crates/polint-engine`; `polint` becomes a facade package |
| `crates/polint/src/core/db.rs` (5,801) | split: read model → SDK `FactSnapshot`; stores/interner/mutation → engine |
| `crates/polint/src/core/rule.rs` (451) | → SDK (`Rule`, `RuleCtx`, `RuleOptions`, `Capabilities`) |
| `crates/polint/src/sdk/**` (3,385) | → SDK verbatim |
| `crates/polint/src/policy_queries.rs` (3,580) | → SDK |
| `crates/polint/src/diagnostics/mod.rs` (2,833) | split: types + report JSON → SDK; renderers stay in engine |
| `crates/polint/src/rule_error.rs`, `rule_manifest.rs`, `core/capability.rs`, `core/labels.rs`, `core/metadata.rs` | → SDK (partially) |
| `crates/polint/src/cache/mod.rs:872-885` | `stable_hash` → SDK (no deps) |
| `crates/polint/src/runner/mod.rs` (524) | → SDK; rewritten as the protocol client; **drop `clap` and `tracing-subscriber`** |
| `crates/polint/src/cli/mod.rs:4236-4407` | `run_local_rule_host*` becomes the snapshot host; add fingerprint gate so `cargo` is not spawned when the binary is current |
| `crates/polint/src/cli/mod.rs:1130-1158` | `pack_cargo_toml` emits `default-features = false` for the engine feature too |
| `crates/polint/src/cli/mod.rs:723-742` | reconsider writing a repo-root `rust-toolchain.toml`; prefer `.polint/rust-toolchain.toml` or nothing once the SDK is vendored |
| `crates/polint/src/rule_test.rs:323-373` | build once, then snapshot-per-fixture |
| `crates/polint/src/analysis/extensions/host.rs` | same treatment: extensions should link the SDK, not the engine |
| `crates/polint/tests/internal_architecture.rs:14-33` | 2 → 3 publishable packages; add the closure gate |
| `docs/CONSUMER-SETUP.md`, `docs/GITHUB-ACTION.md`, `README.md` | cache tables, env vars, versions section; the Action's build-cache half largely retires |
| `Cargo.toml:61` | feature-gate `rusqlite` in the engine (independent win: it is the largest native build cost) |

### 8.3 Phases

**Phase 0 — Measure (no user-visible change).**
Baseline every metric in §9 on the golden corpus and on a 2 vCPU / 4 GB container. Add `polint check --rules-build-report <path>` emitting units, wall time, target bytes, downloaded bytes. Run E1.
*Exit:* a committed baseline table. *Risk:* none.

**Phase 1 — Extract `polint-sdk`; rule host still links the engine.**
Mechanical move. `polint` becomes a facade with `default = ["engine"]`. Rule packs get `default-features = false`. **Behaviour identical** — the rule host still enables `engine` internally, so `run_cli` still runs the kernel in-process. Ship it.
*Exit:* closure gate + feature-leak gate green; all golden/determinism/public-surface-leak tests unchanged. *Risk:* medium (E2). *Rollback:* revert the crate move; no wire format exists yet.

**Phase 2 �� Snapshot protocol, opt-in.**
Implement `FactSnapshot`, host-side production, SDK-side consumption. Gate behind `POLINT_RULES_MODE=snapshot`. Add the mode-equivalence gate. Add the vendored SDK (`.polint/cache/sdk/<version>` + `--offline --locked`). Add the fingerprint gate so `cargo` is not spawned when the rule binary is current.
*Exit:* E3 within budget; E4 byte-identical across modes; E8 offline passes. *Risk:* high (snapshot volume). *Rollback:* the flag defaults off.

**Phase 3 — Flip the default; trim.**
`snapshot` becomes default; `in-process` stays as an escape hatch. Rule packs no longer enable `engine`. Retire the Action's rule-build cache entry (or shrink it drastically). Stop writing a repo-root `rust-toolchain.toml`. Optional: prebuilt SDK rlib artifacts with source fallback (E5).
*Exit:* cold/disk/offline budgets met on the low-powered rig (E7). *Risk:* medium. *Rollback:* flip the default back.

**Phase 4 — Portable artifacts and sandboxing.**
`polint rules build --out`; `[rules] artifact = { url|path, sha256, sdk_abi }`; signature verification. Trust gate + manifest lockdown default-on for repos with no recorded trust. WASM backend behind `[rules] target = "wasm"` (E6).
*Exit:* scenario B works with zero customer compilation; scenario C default-denies. *Risk:* medium. *Rollback:* artifacts and wasm are additive.

---

## 9. Experiment plan, budgets, and kill criteria

**No benchmark results are invented here.** The measured figures in this report are the two in §3.3: the repository's own documentation figure, and the first-party `build-cost` baseline. Everything below is a method, a budget, or a kill criterion — and every ratio budget should be taken against the §3.3.2 baseline cell for the same repository, scenario, and machine, never against the §3.3.1 prose figure.

### 9.1 Standard rigs

- **R1 — low-powered acceptance rig (primary):** 2 vCPU, 4 GB RAM, cold page cache, empty `CARGO_HOME`, HDD-class or throttled I/O. **All acceptance budgets are stated against R1.** The §3.3.2 baseline is from a 6-CPU container, not R1; a cold build will be materially worse on two cores, and that must be measured rather than extrapolated.
- **R2 — developer laptop:** 8–10 cores, 16–32 GB, warm caches.
- **R3 — CI runner:** GitHub-hosted `ubuntu-latest`, `macos-*`, `windows-*`.
- **R4 — air-gapped:** network namespace with no egress, empty `CARGO_HOME`.

Corpora: `examples/**` (small), `tests/golden-corpus/inputs.toml`, and the scale corpus via `scripts/fetch-scale-repos.py` / `scripts/run-scale-corpus.py`.

Existing instrumentation to reuse rather than rebuild: `POLINT_GOLDEN_COST_PATH` writes wall-clock ms + peak RSS + RSS delta per check (`crates/polint/src/golden_cost.rs:18-46`), built on `crate::measure::TimedRun` (`getrusage`/`K32GetProcessMemoryInfo`, `crates/polint/Cargo.toml:53-68`); and `polint-bench build-cost` (`make build-cost`), which already implements E1's Cargo-invocation, compiled-unit, and bytes-written/retained counting for the scenarios in §3.3.2 and reads that same sidecar for rule-host RSS.

### 9.2 Experiments

| ID | Question | Method | Records |
|---|---|---|---|
| **E1** | How big is the closure, before and after? | `cargo tree -e normal --target <triple>` and `cargo build --timings` for (a) today's pack, (b) the proposed SDK closure prototype, on all 5 released targets | unit count, wall time, target bytes, `CARGO_HOME` delta bytes, per-unit slowest 20 |
| **E2** | Can `core/db.rs` be split cleanly? | Spike branch: extract the read model; compile every `examples/**` rule pack against SDK-only | lines moved to SDK, lines of engine code that had to follow, public-API deltas |
| **E3** | What does the snapshot cost? | For each corpus repo × each example rule's planned closure: serialize, measure bytes and ms, deserialize, measure ms and peak RSS | snapshot bytes, ser ms, deser ms, RSS delta, ratio to total analysis time |
| **E4** | Is determinism preserved? | Run golden + `eval::determinism_gate` in both modes, 20× each, reversed file order | byte-equality of diagnostics; snapshot digest stability |
| **E5** | Do prebuilt SDK rlibs pay off? | Build the bundle for pinned `rustc`; drive `rustc` directly; also measure the mismatch-fallback path | build ms, bundle bytes, projected hit rate from `rustc -vV` distribution |
| **E6** | Is WASM viable as a second backend? | Build an example pack for `wasm32-wasip1`; instantiate AOT-cached; run | module bytes, AOT bytes, instantiate ms, run ms vs native, guest memory peak, polint binary size delta |
| **E7** | Does it hold on a small machine? | Repeat E1 + E3 + full cold scan on **R1** | all of the above; this is the acceptance run |
| **E8** | Does offline work? | **R4**: `polint init` → `polint new-rule` → `polint check`, no network, empty `CARGO_HOME` | pass/fail; every network attempt logged |
| **E9** | Does the trust gate hold? | Repo with `.polint/rules/build.rs` writing a sentinel file; also a pack with an extra dependency, a path dep, and a `[patch]` | sentinel must not exist; polint must refuse with a clear message |
| **E10** | How often do we compile? | Instrument: count `cargo`/`rustc` spawns across a realistic session (10 scans, 2 rule edits) | spawn count; must be 2, not 10 |

### 9.3 Budgets (targets on R1 unless noted) and kill criteria

| Metric | Today | **Target** | **Kill criterion** |
|---|---|---|---|
| Cold `polint check`, 1 rule, small repo, empty caches | the §3.3.2 `cold` cell on the same machine (187.3 s on the recorded one) | **≤ 20 s total**; stretch ≤ 5 s with E5 | > 60 s on R1 → the split did not buy enough; reopen §6 |
| Warm re-scan, no rule edit | 0.7 s rebuild + analysis + cargo spawn | **analysis + ≤ 150 ms**, **zero `cargo` spawns** | > 1 s of non-analysis overhead |
| Rule edit → re-scan | 0.7 s + analysis | **≤ 3 s** on R1 | > 8 s |
| Bytes downloaded, first run | polint binary + all crates.io tarballs for 223 units | **polint binary + 0** (vendored SDK) | any network requirement for a pack whose only dep is `polint` |
| Bytes retained per repo | **582.7 MB** | **≤ 120 MB**; stretch ≤ 40 MB | > 250 MB |
| Peak RSS, rule process | n/a (engine in-process) | **≤ 1.3 × snapshot bytes + 64 MB** | > 2 × snapshot + 128 MB |
| Peak RSS, host | current baseline | **≤ 1.15 ×** today | > 1.4 × |
| Snapshot round-trip | 0 (does not exist) | **≤ 15 % of analysis wall time**, p95 over the corpus | **> 25 % on the largest corpus repo → the snapshot design fails; fall back to `in-process` as default and reopen §6.3-C-iii-b (prebuilt rlibs) as the primary** |
| Rule startup (spawn → first fact read) | n/a | **≤ 100 ms + snapshot load**; load ≤ 200 ms per 100 MB | > 500 ms fixed overhead |
| Offline first run | fails | **passes** | fails → Phase 2 does not ship |
| Cross-platform | 5 targets | all 5, and **no C toolchain needed for the rule build** | any target requires a C compiler for the pack |
| Compile frequency | every scan | **only on rule/SDK/toolchain change** | any unconditional `cargo` spawn remains |
| Determinism | byte-identical goldens | **byte-identical across both modes** | any divergence → blocks Phase 3 |
| Trust gate | none | untrusted repo: **no `build.rs` execution, no rule binary execution** | sentinel file created in E9 |
| Prebuilt-rlib fast path (E5) | n/a | ≥ 70 % projected hit rate | < 70 %, or > 2 support incidents per release → drop E5 permanently |
| WASM backend (E6) | n/a | ≤ 2 × native run time; ≤ 25 MB binary growth | > 3 × native, or no scenario-B demand → defer indefinitely |

### 9.4 Global kill criteria for the whole direction

Abandon Option B and fall back to Option A + C-iii-b if **any** of these hold after Phase 2:

1. E3 exceeds 25 % of analysis time on the largest corpus repo and no format change (binary, columnar, mmap, interned strings) brings it under.
2. E2 shows the `core/db.rs` read/write split requires moving more than 20 % of `analysis_kernel` into the SDK — meaning the boundary is not where it looks.
3. E4 shows any non-determinism attributable to the snapshot round-trip that cannot be fixed by canonical ordering.
4. The SDK closure (E1) cannot be brought below **60 units** on any released target — at which point cold-start improvement is under ~4× and does not justify the churn.

---

## 10. Security and trust boundaries

### 10.1 The boundary that exists today

`polint check` on a repository containing `.polint/rules`:

1. spawns `cargo run --manifest-path .polint/rules/Cargo.toml` (`cli/mod.rs:4264-4286`);
2. Cargo resolves dependencies **from crates.io** using a lockfile the repo controls;
3. Cargo executes **`build.rs`** of the pack and of every dependency, and **procedural macros**, at compile time, with the invoking user's full privileges;
4. Cargo runs the produced binary with the same privileges;
5. the binary reads the repo, writes `.polint/output/` and `.polint/cache/`, and has unrestricted filesystem, network, and process-spawn access.

The same applies to `.polint/extensions/*` (`analysis/extensions/host.rs:128-162`) and to `polint test` (`rule_test.rs:323-373`). The generated agent skill grants `Bash(cargo:*)` (`cli/skill.rs:185`).

**This is arbitrary code execution by checkout.** For scenarios A and B it is the same trust posture as `npm install`, `cargo test`, or a `Makefile` — defensible, because the user chose the repository. For scenario C it is a vulnerability, because the agent did not.

Note also that no timeout bounds the rule host today, while the extension host has a 30 s default (`analysis/extensions/host.rs:17`).

### 10.2 Threat surfaces, ranked

| # | Surface | Executes when | Privilege | Mitigation |
|---|---|---|---|---|
| 1 | **`build.rs`** in the pack or a dependency | **compile** | full user | **Manifest lockdown**: refuse packs that declare `build.rs` or `[build-dependencies]` |
| 2 | **Procedural macros** from repo-controlled deps | **compile** | full user | Lockdown: deps ⊆ `{polint}`; `polint-macros` is polint's own and trusted |
| 3 | **Dependency substitution** — arbitrary crates, `path`/`git` deps, `[patch]`, `[replace]`, alternate registries | compile | full user | Lockdown + vendored SDK + `--offline --locked` |
| 4 | **Rule body at runtime** — fs, net, exec | run | full user | Timeout + output bounds (reuse `host.rs:17-19`); WASM sandbox in Phase 4 |
| 5 | **Toolchain pinning** — `rust-toolchain.toml` in the repo triggers a rustup download of a repo-chosen channel | compile | network + disk | Ignore repo `rust-toolchain.toml` for the pack build in untrusted mode; use polint's own pin |
| 6 | **Cache poisoning** — `.polint/cache/rules-target` restored from CI | compile/run | full user | Already well handled by the Action (compiler-scoped key, prune before save — `docs/GITHUB-ACTION.md:108-171`); mostly retires under Option B |
| 7 | **Untrusted prebuilt artifact** (scenario B) | run | full user | Signature or pinned `sha256` + recorded `sdk_abi`; sandbox if WASM |
| 8 | **Resource exhaustion** — pathological rule or fixture | compile/run | CPU/RAM/disk | Timeouts, memory caps, target-dir size ceiling (the Action already has `build-cache-max-size-mb`) |

### 10.3 Controls to add, in order

**T1 — Trust gate.** A repository is trusted for rule execution only if recorded in a user-level trust store (path + git remote + commit). `polint trust` / `polint trust --revoke`. Untrusted repos run non-rule surfaces and say so, loudly and specifically. This is the single highest-value control and it is cheap.

**T2 — Manifest lockdown** (enforced by polint *before* invoking any build). Reject a rule pack whose `Cargo.toml` or lockfile:
- declares `build.rs` or `[build-dependencies]`;
- has any dependency other than `polint` (proc-macro deps especially);
- uses `path`, `git`, or non-default-registry sources;
- contains `[patch]` or `[replace]`;
- (recommended) does not set `unsafe_code = "forbid"` in its lint table.

This is a small, cheap, static check that removes surfaces 1–3 outright and is worth shipping **even without the rest of the plan**.

**T3 — Hermetic build.** Vendored SDK + `--offline --locked`. No registry access during a rule build, ever.

**T4 — Bounded execution.** Timeout, stdout/stderr caps, and (platform-permitting) a memory cap on the rule process. Reuse `ExtensionHost`'s constants and error taxonomy.

**T5 — Sandbox (Phase 4).** WASM/WASI with no preopened directories beyond the snapshot, no network, no clock beyond what determinism allows. **State plainly that this sandboxes execution, not compilation** — T2 remains necessary.

**T6 — Artifact integrity (Phase 4).** Signature or pinned digest, recorded provenance, recorded `sdk_abi`. Reuse the sha256-verify pattern already in `scripts/install.sh:52-66` and `action.yml:130-150`.

### 10.4 How each option scores

| Option | `build.rs` at compile | Arbitrary deps | Runtime sandbox | Net effect |
|---|---|---|---|---|
| A. Status quo | ✔ executes | ✔ allowed | none | Worst |
| **B. Thin SDK + snapshot** | executes unless T2 | blocked by T2 | none (native) | **Much better with T1–T4** |
| C-iii-b. Prebuilt rlibs + direct `rustc` | **never runs Cargo → no `build.rs`, no registry** | structurally impossible | none | Best compile-side posture of the compile-on-customer options |
| D. cdylib plugin | ✔ plus `dlopen` of repo binary | ✔ | none, and forecloses one | Worst |
| E. WASM | ✔ (compile is still native) | blocked by T2 | **✔ strong** | Best runtime posture |
| F. Author-built artifact | **✘ on customer** | ✘ on customer | depends on format | Best for scenario B, with T6 |

**Recommended composite:** T1 + T2 + T3 + T4 in Phase 2/3 (they are cheap and independent of the snapshot work), T5 + T6 in Phase 4. For scenario C, default-deny until T5 exists.

---

## 11. Risks and unresolved decisions

**Risks**

1. **Snapshot volume (highest).** A `StringLiterals` rule over a large TS monorepo could produce very large fact sets. Mitigations in order: serialize only the planned closure (already true — capability planning computes it); narrow by rule `files` scope (the host already computes a rule-scope globset, `cli/mod.rs:3892-3912`); compact binary format; intern strings via the existing stable-key table; memory-map with an archived layout. The last one would change `name: String` to a borrowed or `Arc<str>` form — **an API break** — so it is a last resort with its own decision point. Gated by E3.
2. **`core/db.rs` split depth.** 5,801 lines mixing read model and store machinery. Gated by E2.
3. **Preview policy views.** `Events`, `Calls`, `DataFlow` delegate to `policy_queries` over derived facts (`sdk/facts.rs:886-899, 950-962`). If those derived families are expensive to serialize, the fallback is host-side evaluation with a query round-trip *for those views only* — acceptable because they are documented preview, but it would make them behave differently from stable views. Decide explicitly.
4. **crates.io naming and publishing.** `polint-sdk` and `polint-engine` must be claimable. Three publishable packages is more release surface (`scripts/publish-crates.sh`, `scripts/bump-workspace-version.py`, `docs/RELEASING.md` all need updating). If this is unacceptable, use the single-package feature-gated variant (§6.2).
5. **Two-package invariant.** `internal_architecture.rs:14-33` encodes it. Changing it is a deliberate architectural decision, not a test edit — it should be recorded as such.
6. **Windows.** `libsqlite3-sys` bundled and `tree-sitter` need a C toolchain today. Removing them from the rule-pack closure is a real Windows win, but the SDK must be verified to need no C at all.
7. **Determinism across modes.** Two execution paths means two chances to diverge. Gated by E4 and by keeping `in-process` mode permanently in CI.

**Unresolved decisions — these need a product call, not more research**

1. **Is a no-Rust-toolchain consumer path required for scenario A?** If yes, Phase 4 artifacts become mandatory rather than optional, and the CI-publishes-artifact workflow becomes the documented default. If no, Phase 2/3 suffice. *This is the single decision that most changes the plan.*
2. **Does `polint init` keep writing a repo-root `rust-toolchain.toml`?** (`cli/mod.rs:723-742`) It is intrusive — it pins the *whole repository's* toolchain for polint's benefit — and once the SDK is vendored and the fast path is `rustc`-driven, it is arguably unnecessary. Moving it under `.polint/` is a compatibility change for existing users.
3. **Should `rusqlite`/the semantic store be feature-gated in the engine?** Independent of this plan, it is the largest native build cost in the tree and a Windows/musl liability.
4. **String representation in facts.** `String` (portable, simple, copies) vs. `Arc<str>` interned (cheaper snapshot, small API change) vs. archived `&str` (zero-copy mmap, larger API change). Defer until E3 says which is needed.
5. **Snapshot schema stability.** Is it a public contract (third parties may produce or consume it) or polint-internal (host and SDK are always released together)? **Recommendation: internal, with `sdk_abi_version` checked at handshake and a clear mismatch error.** Making it public multiplies the compatibility burden for no demonstrated user need.
6. **Where does the trust store live?** User-level (`~/.config/polint/trust`) vs. per-invocation flag. User-level is far better UX but is a new stateful surface.
7. **WASM target: `wasip1` or `wasip2`?** `wasip2` is Tier 2, component-model, Wasmtime 17+, `-Cpanic=abort`, and documented as not CI-tested ([platform support](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html)). `wasip1` is the conservative choice for a first backend.

---

## 12. Conclusion

polint's build cost is not caused by rules being code. It is caused by **rules linking the engine**. The typed fact API — the thing that makes rules-as-code good — is already a pure read projection over an in-memory database, the fact structs are already plain serializable data, the macro already emits only SDK paths, and the host↔rule wire already exists as two versioned public JSON schemas. The measured cold start on `examples/basic` is 225 compilation units in 187 seconds — 224 of those units parsers, solvers, kernel, and a bundled SQLite that the customer's rule never touches — and 582.7 MB retained; a one-line rule edit recompiles exactly one unit in under a second (§3.3.2).

So the answer is not a DSL, not a plugin ABI, and not WebAssembly. **The answer is to move the engine to the side of the process boundary that already ships as a prebuilt binary, and let the rule pack compile against a thin SDK.** Rule sources do not change. Typed facts, capability derivation, diagnostics, fixture tests, determinism, and AI-friendly authoring are preserved, and two of them — fixture tests and determinism auditability — get better.

Native Rust must be compiled by someone. This plan makes that "someone" compile as little as possible: one crate, offline, against a vendored SDK, only when the rules actually changed — with an optional prebuilt-rlib fast path that removes Cargo from the loop entirely, and an optional portable artifact path that removes compilation from the customer entirely for shared rule packs.

The recommended defaults differ by scenario, and should: **a team that owns its rules compiles them (A); a team that receives them does not (B); an agent facing an unknown repository refuses to, until someone says otherwise (C).** That last one is not a performance decision. Today, `polint check` on an untrusted checkout is arbitrary code execution at compile time, and the cheapest, highest-value item in this whole report — manifest lockdown plus a trust gate — is worth shipping on its own schedule, ahead of everything else.

---

## 13. Sources

### 13.1 Repository evidence (primary; all paths relative to `/workspace/polint`, commit `b272b378`)

- `ARCHITECTURE.md:26-55` — two publishable packages; private module ownership table
- `ARCHITECTURE.md:112-135` — supported rule-author surface; `#[polint::rule]` contract; `RuleCtx` is not a back door
- `ARCHITECTURE.md:203-233` — provider contract; capability planning; `Supported`/`Unsupported`/`SetupMissing`
- `ARCHITECTURE.md:237-264` — stable-key identity; interned IDs vs. resolved text
- `ARCHITECTURE.md:341-390` — cache boundaries, digest inputs, determinism rules
- `ARCHITECTURE.md:485-508` — enduring non-goals (no built-in catalog; no dynamic-provider ABI; no editor-latency guarantee)
- `AGENTS.md:122-171` — Rule Authoring Platform Contract (rules are external SDK consumers; capabilities derived from typed views; no `impl Rule` escape hatch)
- `Cargo.toml:2-28` — workspace members incl. 17 example rule packs; `:29` `resolver = "3"`; `:36` `rust-version = "1.95"`; `:49-54` oxc; `:61` `rusqlite … features = ["bundled"]`; `:72-73` tree-sitter; `:82` `unsafe_code = "forbid"`
- `crates/polint/Cargo.toml:11-24` — `default = ["lang-go","lang-typescript"]`, `all-languages`, `bench`
- `crates/polint/Cargo.toml:26-51` — dependencies (oxc optional, rusqlite unconditional, clap, rayon, tracing-subscriber, …)
- `crates/polint/Cargo.toml:53-68` — unix `libc`/`rustix` (getrusage), windows `windows-sys` (Job Objects)
- `crates/polint/Cargo.toml:92-94` — `unsafe_code = "deny"` (workspace `forbid` downgraded for one audited FFI)
- `rust-toolchain.toml:1-3` — `channel = "1.95"`
- `.cargo/config.toml:1-8` — `incremental = false`; sccache guidance; `SCCACHE_CACHE_SIZE = 10G`
- `Cargo.lock` — 274 `[[package]]` entries; `:1158-1193` `polint`'s dependency list; `:694-702` `libsqlite3-sys` → `cc`/`pkg-config`/`vcpkg`; `:1570-1581` `rusqlite`; `:981-999` `oxc_resolver` → `simd-json`
- `crates/polint/src/cli/mod.rs:4252-4343` — `run_local_rule_host_kind`: `cargo run --quiet [--release] --manifest-path … -- check --format json …`; `CARGO_TARGET_DIR` = `rules-target`; report parsed from stdout
- `crates/polint/src/cli/mod.rs:4350-4407` — `run_local_rule_host_inspect`: second `cargo run` for rule manifests
- `crates/polint/src/cli/mod.rs:4417-4452` — `LocalRuleHostProfile`; **release is the default**
- `crates/polint/src/cli/mod.rs:3949-3985` — one subprocess per manifest; scoped second source load for ignores/`--stat`
- `crates/polint/src/cli/mod.rs:3872-3882` — `discover_local_rule_hosts` from `[rules] paths`
- `crates/polint/src/cli/mod.rs:3884-3912` — `config_rule_scope_globset` (host already narrows by rule scope)
- `crates/polint/src/cli/mod.rs:3418-3425` — `check` delegates entirely to rule hosts when present
- `crates/polint/src/cli/mod.rs:4054-4106` — `review` requires rule hosts; `--kind review --changed-files`
- `crates/polint/src/cli/mod.rs:1130-1158` — `pack_cargo_toml` (`default-features = false`, language features, `[workspace]`)
- `crates/polint/src/cli/mod.rs:1160-1173` — `initial_pack_main` (`run_cli(vec![…])`)
- `crates/polint/src/cli/mod.rs:1119-1128` — `enabled_language_features`
- `crates/polint/src/cli/mod.rs:667-686, 723-742` — `polint init`; `ensure_repo_rust_toolchain_shim` writes repo-root `rust-toolchain.toml`
- `crates/polint/src/cli/rules_host_error.rs:1-37, 74-95` — cold-start failure taxonomy: MSRV, network/registry, manifest, missing `rustc`
- `crates/polint/src/cli/skill.rs:185` — generated agent skill grants `Bash(cargo:*)`
- `crates/polint/src/runner/mod.rs:22-142` — full `clap` CLI in the rule host
- `crates/polint/src/runner/mod.rs:144-167` — `run_cli` (+ `tracing_subscriber` init)
- `crates/polint/src/runner/mod.rs:381-448` — `analyze_and_run`: `AnalysisKernel::run` executes **in the rule host**
- `crates/polint/src/sdk/mod.rs:28-61` — prelude allowlist; `:41-46` report schema URL; `:63-136` `__private` (`AnalysisDb`, `Capabilities`, `FactView`, `make_rule_with_manifest`)
- `crates/polint/src/sdk/facts.rs:23-52` — `SourceFiles<'a>` is `{ db: &'a AnalysisDb }`; `all() -> &'a [SourceFile]`
- `crates/polint/src/sdk/facts.rs:497-560` — `Symbols<'a>`: slices, filters, stable-key resolution
- `crates/polint/src/sdk/facts.rs:872-962` — `Cfg`/`CallGraph` reserved; `Events`/`Calls`/`DataFlow` delegate to `policy_queries`
- `crates/polint/src/sdk/facts.rs:1098-1128` — `FactView<'a>::build(db: &'a AnalysisDb)`
- `crates/polint/src/sdk/scope.rs:36-111` — `globset`-backed scope helpers with a process-wide matcher memo
- `crates/polint/src/sdk/policy.rs:9-10` — imports only `cache::stable_hash` and `diagnostics`
- `crates/polint/src/core/rule.rs:61-81` — `Rule` = `Arc` closures; run closure is `Fn(&AnalysisDb, &mut RuleCtx<'_>) -> RuleResult`
- `crates/polint/src/core/rule.rs:156-184` — `RuleCtx<'a> { db: &'a AnalysisDb, … }`
- `crates/polint/src/core/db.rs:142-169` — `AnalysisDb` fields (files, interner, fact metadata, fact stores, path contexts, changeset)
- `crates/polint/src/analysis_api/symbol_facts.rs:124-189` — plain `#[non_exhaustive]` fact structs; neighbouring enums already derive `Serialize, Deserialize`
- `crates/polint-macros/src/lib.rs:8-17, 37-120` — expansion emits only `::polint::sdk::*` paths
- `crates/polint-macros/src/lib.rs:313-324` — `capability_for_type`: parameter type → capability + canonical path
- `crates/polint-macros/Cargo.toml:1-20` — proc-macro crate; `syn`/`quote`/`proc-macro2`
- `crates/polint/src/analysis/extensions/host.rs:17-19` — timeout 30 s, stdout 1 MiB, stderr 16 KiB
- `crates/polint/src/analysis/extensions/host.rs:56-126` — `handshake` / `run_provider` JSON-over-stdio with schema validation and identity checks
- `crates/polint/src/analysis/extensions/host.rs:128-162` — extension `command_spec`: `cargo run --manifest-path …`, `CARGO_TARGET_DIR` = `extensions-target`
- `crates/polint/src/rule_test.rs:323-373` — `polint test` spawns `cargo run` per fixture case
- `crates/polint/src/golden_cost.rs:18-68` — `POLINT_GOLDEN_COST_PATH`; wall-clock ms + peak RSS + RSS delta
- `crates/polint/src/cache/mod.rs:362-417` — cache layout (`analysis`, `layers`, `derived`, `semantic-store`, `rules-target`, `extensions-target`, `review`)
- `crates/polint/src/cache/mod.rs:872-885` — `stable_hash` (FNV-1a, dependency-free)
- `crates/polint/src/lib.rs:1-58` — public surface (`runner`, `sdk`, `rule`, `run_main`); all other modules `pub(crate)`
- `crates/polint/tests/internal_architecture.rs:4-33` — `REMOVED_PACKAGES`; `workspace_has_only_two_publishable_product_packages`
- `crates/polint/tests/internal_architecture.rs:35-81` — enforced internal dependency directions
- `crates/polint/tests/` — `public_surface_leak.rs` (873), `capability_matrix.rs` (913), `github_action_cache.rs` (1,119), `golden.rs` (834), `cli.rs` (12,464)
- `.swarm/T-SPLIT-LAND.md:1-27` — correction: the eight-crate split did not land; module layering + `module_layering.rs` did
- `docs/GITHUB-ACTION.md:60-94` — analysis cache key
- `docs/GITHUB-ACTION.md:95-144` — build cache key (compiler-scoped); *"Compiling those packages means compiling the `polint` library and its dependencies, which dominates the check phase on a cold runner."*
- `docs/GITHUB-ACTION.md:145-171` — prune-before-save; **the 223 units / 185.4 s / 562 MB → 537 MB / 0.7 s documentation figure (`:162-166`)**
- `research/evaluation-harness/baselines/build-cost.json` — the first-party `build-cost` baseline (§3.3.2); `research/evaluation-harness/README.md` — its metric definitions and limits; `crates/polint-bench/src/build_cost/**` — the harness that produced it
- `docs/CONSUMER-SETUP.md:1-11` — MSRV requirement for rule packs
- `docs/CONSUMER-SETUP.md:44-107` — `inspect rule` / `test` / `facts` / `unknowns` / `explain` JSON surfaces
- `docs/CONSUMER-SETUP.md:109-121` — environment variables (`POLINT_CARGO`, `POLINT_CACHE_DIR`, `POLINT_RULES_PROFILE`, `POLINT_RULES_TARGET_DIR`, `POLINT_RULES_TOOLCHAIN`)
- `docs/CONSUMER-SETUP.md:124-178` — cache roles; *"Repo-local rule hosts run optimized by default … because rule execution can dominate large-repo scans."*
- `docs/CONSUMER-SETUP.md:249-263` — rules-host failure modes
- `docs/RULE-AUTHORING-PLATFORM-REVIEW.md:40-56, 108-131` — the SDK-consumer contract and the temp-repo proof requirement
- `docs/API-VISIBILITY-PLAN.md:45` — provider extensions are "preview," not a broad public provider API
- `README.md:114-167` — rule pack layout and the canonical rule example
- `README.md:176-237` — cache layout; contributor sccache guidance
- `README.md:460-485` — Action cache behaviour; *"A fully cold first run can still pay install, build, and analysis costs."*; MSRV table
- `.github/workflows/release.yml:124-196` — 5-target release matrix (`x86_64`/`aarch64-unknown-linux-gnu`, `x86_64`/`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`); tar.gz + sha256
- `action.yml:70-160` — install: download release asset, verify sha256, fall back to `cargo install polint --locked`
- `scripts/install.sh:29-66` — same download-and-verify pattern
- `examples/basic/.polint/rules/{Cargo.toml,src/main.rs,src/no_raw_colors.rs}` — canonical rule pack shape
- `examples/go-sensitive-writes/.polint/rules/src/no_sensitive_balance_writes.rs` — a `Symbols`/`References` rule (the "expressive Rust" case)
- Commit `3c0f4dfa` — *"perf(action): key rule-host builds on compiler inputs and prune them before saving (#100)"*

### 13.2 External sources — fetched and verified in this environment

All fetched 2026-08-25. Only `doc.rust-lang.org` was reachable from this environment.

1. **Type layout** — <https://doc.rust-lang.org/reference/type-layout.html> — *"Type layout can be changed with each compilation. Instead of trying to document exactly what is done, we only document what is guaranteed today."* `repr(Rust)` guarantees only field-alignment divisibility, type alignment ≥ max field alignment, and non-overlapping struct fields; *"There are no other guarantees of data layout made by this representation."* → **no stable Rust ABI**.
2. **Linkage** — <https://doc.rust-lang.org/reference/linkage.html> — `rlib` is *"an intermediate artifact … interpreted by the compiler in future linkage"*; `cdylib` is *"used when compiling a dynamic library to be loaded from another language"*; the page makes **no** dylib ABI-stability claim.
3. **Codegen options** — <https://doc.rust-lang.org/rustc/codegen-options/index.html> — `-C metadata` *"may be used … to differentiate symbols between two different versions of the same crate being linked"*; `-C prefer-dynamic`, `-C lto`, `-C codegen-units`, `-C linker-plugin-lto`; `-C incremental` *"inhibits certain optimizations and is not recommended for release builds."*
4. **Cargo profiles** — <https://doc.rust-lang.org/cargo/reference/profiles.html> — dev: `opt-level=0`, `debug=true`, `codegen-units=256`, `incremental=true`; release: `opt-level=3`, `codegen-units=16`, `incremental=false`, `lto=false`.
5. **Cargo build cache** — <https://doc.rust-lang.org/cargo/reference/build-cache.html> — target/build directory layout, `.d` dep-info files, `CARGO_TARGET_DIR`; sccache via `RUSTC_WRAPPER` recommended for sharing dependencies across workspaces.
6. **Cargo dependency resolution** — <https://doc.rust-lang.org/cargo/reference/resolver.html> — feature unification; *"Platform-specific dependencies with the `[target]` table are resolved as-if all platforms are enabled."*; resolver v2 exception for target-specific dependency features.
7. **`cargo fetch`** — <https://doc.rust-lang.org/cargo/commands/cargo-fetch.html> — *"If `--target` is not specified, then all target dependencies are fetched."*; `--offline`, `--frozen`.
8. **`cargo vendor`** — <https://doc.rust-lang.org/cargo/commands/cargo-vendor.html> — vendoring + `.cargo/config.toml` source replacement for offline builds; vendored sources are read-only.
9. **Cargo unstable features** — <https://doc.rust-lang.org/cargo/reference/unstable.html> — artifact dependencies (`-Z bindeps`, allows depending on `bin`/`cdylib`/`staticlib`) and `-Z build-std` are **nightly-only**, no stabilization date.
10. **Platform support** — <https://doc.rust-lang.org/rustc/platform-support.html> — Tier 1 with host tools includes `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`; `x86_64-apple-darwin` and `x86_64-unknown-linux-musl` are Tier 2 with host tools.
11. **`wasm32-wasip2`** — <https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html> — Tier 2; `std` supported; emits **components**, needs a component-model runtime (Wasmtime 17+); `-Cpanic=abort` by default; *"not tested in CI at this time."*
12. **Profile-guided optimization** — <https://doc.rust-lang.org/rustc/profile-guided-optimization.html> — `-Cprofile-generate` / `llvm-profdata merge` / `-Cprofile-use`; *"All `rustc` flags must match between instrumentation and optimization phases"*; use absolute paths under Cargo.
13. **`rustc_metadata`** — <https://doc.rust-lang.org/nightly/nightly-rustc/rustc_metadata/index.html> — documents a `METADATA_HEADER` including `METADATA_VERSION` and a `dependency_format` module ("Resolution of mixing rlibs and dylibs"), and makes **no statement of cross-version metadata compatibility**. Recorded as a negative result: there is no documented stability guarantee to rely on.

### 13.3 External sources — NOT fetched (network access to these domains was unavailable in this environment)

Every claim below is flagged in the text where it appears and is **from model knowledge, not verified here**. Each should be checked before it influences a decision. `WebSearch` was also unavailable.

| Claim used | Where to verify |
|---|---|
| **Dylint** loads Rust lints as dynamic libraries built against `rustc_private` on a pinned nightly per library, with the toolchain encoded in the library name | <https://github.com/trailofbits/dylint> |
| **golangci-lint** replaced Go `plugin`-based `.so` loading with "module plugins" that **rebuild the linter binary** with your linters linked in | <https://golangci-lint.run/plugins/module-plugins/> |
| **CodeQL query packs** can be published precompiled so consumers do not recompile queries (author-compiles / customer-consumes precedent) | <https://docs.github.com/en/code-security/codeql-cli/using-the-advanced-functionality-of-the-codeql-cli/creating-and-working-with-codeql-packs> |
| **Wasmtime AOT**: `wasmtime compile`, `Module::serialize` / `Module::deserialize`; precompiled artifacts are Wasmtime-version and target specific; `deserialize` is `unsafe` because the input is trusted | <https://docs.wasmtime.dev/> · <https://docs.rs/wasmtime/latest/wasmtime/struct.Module.html> |
| **WASI capability model** — no ambient filesystem/network; directories must be preopened | <https://component-model.bytecodealliance.org/> · <https://wasi.dev/> |
| **sccache** shared/cloud backends (S3, GCS, Redis) for cross-machine compile caching | <https://github.com/mozilla/sccache> |
| **cargo-dist** / **cargo-binstall** for prebuilt-binary distribution | <https://github.com/axodotdev/cargo-dist> · <https://github.com/cargo-bins/cargo-binstall> |
| **rust-analyzer proc-macro server** — a separate process bridging ABI-versioned proc-macro dylibs, with per-rustc-version ABI shims (precedent for version-keyed native artifacts) | <https://github.com/rust-lang/rust-analyzer> |
| **Extism** — WASM plugin framework with host SDKs | <https://extism.org/> |
| **Bazel / Buck2 remote caching** as an alternative build-reuse substrate | <https://bazel.build/remote/caching> · <https://buck2.build/> |
| **Nix** binary caches for reproducible prebuilt dependency closures | <https://nixos.org/> |
| **`rustc_codegen_cranelift`** — faster debug codegen, nightly component | <https://github.com/rust-lang/rustc_codegen_cranelift> |
| Cargo's target-filtered *downloading* during `cargo build` (as distinct from the platform-agnostic *resolve* documented in 13.2 §6) | Verify empirically in E1 by diffing `CARGO_HOME/registry` after a clean single-target build |

---

*Research and report only. `/workspace/polint` was not modified; no implementation code was written.*

---
