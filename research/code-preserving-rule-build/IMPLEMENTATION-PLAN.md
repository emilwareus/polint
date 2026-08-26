# polint — Code-Preserving Build Architecture: Implementation Plan

**Target repository:** `emilwareus/polint` (`/workspace/polint`), `main` @ `b272b378` (v0.2.1), clean tree.
**Document type:** implementation plan. Every path, symbol, and line range below was read from the working tree at that commit.

---

## 0. Provenance and verification note

A prior research report exists at `/opt/data/research/polint-builds-code-preserving-2026-08-25/report.md`. Its headline figures entered this plan as **unverified reported measurements**, and Phase A existed to replace them with measured values before any design decision leaned on them. **Phase A has now run.** Claims verified directly against the repository are marked **[verified]**; claims replaced by measurement are marked **[measured]** and cite the cell of `research/evaluation-harness/baselines/build-cost.json` that carries them.

The measured cells are `examples/basic` × {`cold`, `warm-noop`, `warm-rule-edit`, `warm-source-edit`, `test-suite`}, release profile, median of three runs on one machine (`environment.label = linux-container-6cpu`, `cargo`/`rustc` 1.95.0). Reproduce with `make build-cost`; rewrite with `make build-cost-baseline BUILD_COST_LABEL=<machine> BUILD_COST_RUNS=<n>`.

| Reported claim | Status | Evidence |
| --- | --- | --- |
| "~238k-line polint engine" | **[verified]** | `find crates/polint/src -name '*.rs' \| xargs wc -l` → `238359 total` |
| "~223 compiled units" for a rule-pack build | **[measured]** — it is **225** | `cold` cell: `compiled_units = 225`, `rustc_invocations = 241` (the other 16 are Cargo probes), `cargo_invocations = 1`. Identical across all three runs. The pre-measurement estimate (129 packages in the leak-probe lockfile plus `lang-go`, `lang-typescript`'s 18 `oxc*` packages, build scripts and proc macros) was in range, but the number is now observed rather than inferred |
| "~185.4s cold build" | **[measured]** — **187.3 s** on the recorded machine, and **not portable** | `cold` cell: `wall_clock_ms = 187303` (runs: 185121, 243199, 187303). The same cell on the same host measured 417.6 s while the host was contended. Treat wall-clock as machine- and load-specific; the counts are not |
| "~537MB target retention" | **[measured]** — **582.7 MB** across 1,708 files, and it is a different quantity | `cold` cell: `rules_target_bytes_after = 582705787`, `rules_target_files_after = 1708`, identical across all three runs. 537 MB was the GitHub Action's figure *after* it prunes the rule package's own output (`docs/GITHUB-ACTION.md:162-166`); the harness prunes nothing. The control surface still exists: `action.yml` input `build-cache-max-size-mb` (default `""`, i.e. no ceiling) |
| "Cargo runs on every scan, not only the first" | **[measured]** | `warm-noop` and `warm-source-edit`: `cargo_invocations = 1`, `compiled_units = 0`, `wall_clock_ms` 157 and 163. A rule edit is `cargo_invocations = 1`, `compiled_units = 1`, 735 ms. So §1.1's "zero Cargo invocations when nothing changed" removes a real per-scan cost, not a hypothetical one |
| "shells out to `cargo run` and recompiles the whole engine" | **[verified]** | `crates/polint/src/cli/mod.rs:4260-4319` (`run_local_rule_host_kind`), `:4350-4407` (`run_local_rule_host_inspect`), `crates/polint/src/rule_test.rs:323-373` (`run_rule_host_check`), `crates/polint/src/analysis/extensions/host.rs:128-162` (extension host) |

---

## 1. Executive goal and invariants

### 1.1 Goal

Make a customer scan with repo-local rules stop paying for a full compile of the polint engine and its dependency closure, **without changing what a polint rule is**. A rule stays real, typed, expressive Rust. The engine becomes a prebuilt host that produces facts; the repo-local rule binary links only a thin SDK, deserializes one fact snapshot, and runs the author's Rust over borrowed typed views.

Concretely, after this work:

* `polint check` in a repo with `.polint/rules` performs **zero** Cargo invocations when the rule sources and toolchain are unchanged since the last build.
* When a rebuild *is* needed, the compile covers the rule crate plus a thin SDK closure — not `oxc_*`, `tree-sitter*`, `rusqlite`, `petgraph`, `rayon`, `ignore`, `clap`, `tracing-subscriber`, or the 238k-line engine.
* `polint test` builds the rule host **once** for the whole fixture suite instead of once per fixture case (today: `crates/polint/src/rule_test.rs:184-195` loops `run_case` → `run_rule_host_check`, one `cargo run` per case).

### 1.2 Non-negotiable product invariants

These are **decisions already made**, not open questions.

**I1 — Rules stay Rust.** No DSL, no YAML/TOML policy language, no interpreted rule format as the primary model. A declarative policy language is explicitly rejected as the product's authoring model. Code-generating conveniences (`polint new-rule --template …`, `crates/polint/src/cli/mod.rs:1732-1865`) remain strictly secondary and continue to emit ordinary Rust.

**I2 — Existing rule source stays byte-identical.** Every one of these must still compile unchanged:

| Construct | Current source of truth |
| --- | --- |
| `use polint::sdk::prelude::*;` | `crates/polint/src/sdk/mod.rs:28-61` |
| `#[polint::rule(id = …, description = …, severity = …, kind = …)]` | `crates/polint-macros/src/lib.rs:8-125` |
| `fn f(ctx: &mut RuleCtx<'_>, imports: Imports<'_>, …) -> RuleResult` | `crates/polint/src/core/rule.rs:155-249`, `crates/polint/src/sdk/facts.rs` |
| `polint::runner::run_cli(vec![…])` in `.polint/rules/src/main.rs` | `crates/polint/src/runner/mod.rs:144-157` |
| ordinary Rust helpers, control flow, `?`, `anyhow::bail!` | `crates/polint/src/rule_error.rs:10-20` |
| the `polint::sdk::prelude` allowlist (116 names) | `crates/polint/tests/public_surface_leak.rs:41+` |

**I3 — Borrowed typed views stay borrowed.** `SourceFiles<'a>`, `Imports<'a>`, `Symbols<'a>` … are `Copy` structs holding `&'a AnalysisDb` and returning `&'a [Fact]` / `impl Iterator<Item = &'a Fact>` (`crates/polint/src/sdk/facts.rs:23-52, 258-291, 495-575`). Signatures and lifetimes are unchanged; only what `AnalysisDb` *is* inside the rule process changes. No view may start returning owned `Vec<Fact>`.

**I4 — Output compatibility.** `PolintReport` (`crates/polint/src/diagnostics/mod.rs:117-124`, schema `docs/schemas/polint-report-v1.json`), `InspectRuleReport` (`crates/polint/src/rule_manifest.rs:162-168`, schema `docs/schemas/polint-rule-inspect-v1.json`), the AI-friendly report (`docs/schemas/polint-ai-friendly-v1.json`), SARIF, and the rule-test report (`docs/schemas/polint-test-report-v1.json`) keep byte-identical shapes for identical inputs.

**I5 — Determinism.** Diagnostic ordering, dedup, stable fingerprints, and cache digests must be unchanged. The snapshot round-trip must be order-preserving for every fact family, because view methods document "deterministic database order".

### 1.3 Explicitly out of scope

* **DSL-first policy authoring.** Not built, not designed for, not a fallback.
* **Native `cdylib` / stable-ABI rule plugins.** Rust has no stable ABI; a `dlopen`-ed rule object would couple rule builds to the exact engine rustc version and give the rule in-process access to the host. The process boundary is the design, not a limitation to be optimised away.
* **Remote-first execution.** No "upload your repo and we analyse it" path. The host is a local prebuilt binary.
* **WASM as the authoring path.** WASM appears only as a *later, optional distribution backend* behind a decision gate (Phase K), and even then the authoring source is the same Rust.
* **Removing Cargo entirely.** Cargo remains the compiler driver for rule crates; the goal is to stop invoking it when nothing changed and to shrink what it compiles when it must run.

---

## 2. Current architecture: exact call, data, and build path

### 2.1 Package graph today

`Cargo.toml:1-25` — workspace members are `crates/polint`, `crates/polint-bench`, `crates/polint-eval`, `crates/polint-macros`, and 17 `examples/*/.polint/rules` packs. `tests/fixtures/public-surface-leak-probe` is `exclude`d.

Two crates publish: `polint` and `polint-macros` (`scripts/publish-crates.sh:11-15`), asserted by `crates/polint/tests/internal_architecture.rs:14-33`, which also forbids the seven previously-split packages (`polint-core`, `polint-ir`, `polint-analysis-api`, `polint-frontend-api`, `polint-analysis`, `polint-go`, `polint-ts`) from reappearing.

`ARCHITECTURE.md:26-91` documents the module layering that *replaced* those crates: `internal_core` → `ir` → `analysis_api` → `frontend_api` → `analysis_neutral` → {`go`, `ts`} → facade. `crates/polint/tests/internal_architecture.rs:35-81` enforces the directions with textual `crate::…` exclusion checks.

Sizes (`wc -l`, Rust only):

| Module | LOC | Role |
| --- | --- | --- |
| `crates/polint/src` (total) | **238,359** | whole engine |
| `crates/polint/src/core/db.rs` | 5,801 | `AnalysisDb` |
| `crates/polint/src/cli/mod.rs` | 4,914 | CLI, rule-host spawn, scaffolding |
| `crates/polint/src/policy_queries.rs` | 3,580 | policy-view query engine |
| `crates/polint/src/diagnostics/mod.rs` | 2,833 | diagnostics + renderers |
| `crates/polint/src/sdk/facts.rs` | 1,985 | typed fact views |
| `crates/polint/src/analysis_plan.rs` | 1,762 | capability planning |
| `crates/polint/src/sdk/policy.rs` | 958 | policy query vocabulary |
| `crates/polint/src/rule_test.rs` | 752 | `polint test` fixture runner |
| `crates/polint/src/runner/mod.rs` | 524 | `run_cli` (rule-host CLI) |
| `crates/polint/src/rule_manifest.rs` | 452 | manifest + inspect wire types |
| `crates/polint/src/core/rule.rs` | 451 | `Rule`, `RuleCtx`, `run_rules` |
| `crates/polint/src/sdk/scope.rs` | 183 | glob scoping helpers |
| `crates/polint/src/internal_core/*` | 1,468 | IDs, spans, diagnostics, language |
| `crates/polint/src/analysis_api/*` | 2,944 | provider/fact contracts |

### 2.2 The scan path, end to end

1. `crates/polint/src/main.rs:1-11` → `polint::run_main()` (`lib.rs:15-17`) → `cli::run()` (`cli/mod.rs:633`).
2. `cli/mod.rs:661` dispatches `Command::Check` → `check(cwd, args)` (`cli/mod.rs:3418`).
3. `check` calls `discover_local_rule_hosts(&root)` (`cli/mod.rs:3872-3882`), which reads `[rules] paths` from `.polint.toml` and collects each `<path>/Cargo.toml` that exists.
4. If any exist → `check_local_rule_hosts(root, args, &manifests)` (`cli/mod.rs:3949-4046`). **This is the fork where the parent CLI stops doing analysis and delegates everything to a subprocess.**
5. For each manifest, `run_local_rule_host` (`cli/mod.rs:4236-4244`) → `run_local_rule_host_kind` (`cli/mod.rs:4252-4343`):
   * program = `$POLINT_CARGO` ?? `$CARGO` ?? `"cargo"` (`:4260-4262`);
   * `cargo run --quiet [--release | --profile <p>] --manifest-path <pack>/Cargo.toml -- check --format json --fail-on none --ignore-comments <bool> --kind <check|review> [--changed-files F] [--profile P] [--no-cache] [--only-rule P] <paths…>` (`:4264-4312`);
   * env: `POLINT_CACHE_DIR=<cache root>`, `CARGO_TARGET_DIR=<cache>/rules-target` (`:4295-4297`), optional `RUSTUP_TOOLCHAIN` from `POLINT_RULES_TOOLCHAIN` (`:4307-4311`);
   * profile: `apply_local_rule_host_profile` (`:4442-4452`) — default **`--release`** (`LocalRuleHostProfile::from_env_value` returns `Release` when `POLINT_RULES_PROFILE` is unset, `:4429-4439`);
   * on failure → `rules_host_error::rules_host_error_message` (`cli/rules_host_error.rs:8-37`);
   * on success → `diagnostics_and_rule_execution_from_public_json_report(stdout)` (`:4337`).
6. Inside the child, `polint::runner::run_cli(rules)` (`runner/mod.rs:144-157`) → `run` (`:159-167`) → `check` (`:233-289`) → `analyze_and_run` (`:381-448`):
   * `load_config_for_check` (`:478-487`);
   * `crate::cache::Cache::default_for_repo` (`:392`);
   * `RulePlanInputs::collect(rules, enabled)` (`:406`), `AnalysisPlan::from_inputs` (`:411`);
   * `AnalysisKernel::run(KernelInput { loaded, cache, config_digest, rule_digest, plan, parallel: true })` (`:415-422`; kernel at `analysis_kernel/mod.rs:353-382`);
   * optional `output.db.set_changeset(changeset)` for `polint review` (`:427-430`);
   * `run_rules_with_runtime_provider_blockers(&output.db, rules, &options, Some(&exact_enabled), true, &output.capability_support, &output.runtime_blocked_rules)` (`:431-439`; impl `core/rule.rs:307-374`);
   * renders `--format json` via `render_with_sarif_help` (`:284`) and exits.
7. Back in the parent, `check_local_rule_hosts` merges rows (`merge_rule_execution_rows`, `cli/mod.rs:4345-4348`), optionally reloads a *scoped second copy* of the file set for ignore-comments/`--stat` (`cli/mod.rs:3973-3985`), applies baseline, renders, exits.

### 2.3 Where the cost is

* **The child links the whole engine.** The generated pack manifest (`pack_cargo_toml`, `cli/mod.rs:1130-1158`) writes `polint = { version = "…", default-features = false, features = ["lang-go", "lang-typescript"] }` — the rule crate depends on the crate that *contains* the parser frontends, the kernel, the analysis stack, and the CLI. `enabled_language_features()` (`cli/mod.rs:1119-1128`) mirrors the parent CLI's features into the pack.
* **The parent also does work twice.** `check_local_rule_hosts` loads sources again in-process for ignores/`--stat` (`cli/mod.rs:3975`, `:3982`).
* **`polint inspect rule` is a second full `cargo run`** (`cli/mod.rs:4350-4407`).
* **`polint test` is N `cargo run`s** — `run_rule_tests` loops cases (`rule_test.rs:184-195`), each `run_case` spawns per rule-host manifest (`rule_test.rs:262-280` → `:323-373`).
* **Extension hosts add more.** `ExtensionHost::command_spec` (`analysis/extensions/host.rs:128-162`) spawns `cargo run --manifest-path <ext>/Cargo.toml` with `CARGO_TARGET_DIR=<cache>/extensions-target`.
* **CI already fights this.** `action.yml:22-33, 210-263` restores/saves `.polint/cache/rules-target` under a build-input key and prunes every rule package's own output before saving (`scripts/action/prepare-build-cache-save.sh`), because a cached host binary must not outlive its sources (`crates/polint/src/cache/mod.rs:370-388`).

### 2.4 Assets that make this tractable

Five pieces of the target design already exist in the repository:

1. **An out-of-process host protocol with handshake, timeouts, output limits, env allowlist, and schema versioning** — the extension host: `analysis_neutral/extensions/protocol.rs:1-128` (`polint-extension-handshake-v1`, `polint-extension-provider-run-v1`, `deny_unknown_fields`, `validate_schema`), `analysis/extensions/host.rs:17-19` (`DEFAULT_TIMEOUT` 30 s, stdout limit 1 MiB, stderr limit 16 KiB), `:274-297` (`ExtensionHostFailureKind`), `:332-369` (`EXTENSION_ENV_ALLOWLIST` + `env_clear()`).
2. **Fact rows already `Serialize`/`Deserialize`** — `analysis_api/syntax_facts.rs:11-145` (all 10 syntax families plus `CachedFileFacts`, which is literally a per-file fact section), `analysis_api/symbol_facts.rs` (13 derives), `analysis_api/module_facts.rs` (8), `internal_core/ids.rs:3-53`, `internal_core/span.rs:5-35`, `internal_core/lang.rs:4-14`.
3. **A read-only fact query layer that already takes a trait object** — `analysis_neutral/symbol_graph/query.rs:6-102` operates on `&dyn FactDatabase` (`analysis_api/provider/mod.rs:55-139`), not on the concrete `AnalysisDb`.
4. **A cost-measurement harness** — `crates/polint/src/golden_cost.rs:1-132` (wall-clock + peak RSS sidecars, env `POLINT_GOLDEN_COST_PATH`) wired into `runner/mod.rs:162-164` and gated by `crates/polint/tests/golden.rs:28-31` (`MAX_COST_RATIO = 1.50`, floors 100 ms / 16 MiB).
5. **No built-in rules to migrate.** `analyze_and_run` in the *parent* CLI runs with `let rules: Vec<Rule> = Vec::new();` (`cli/mod.rs:3806`) — the engine never executes user rules in-process on the product path. Only tests do.

---

## 3. Target package graph

### 3.1 The Cargo cycle question — resolved

The apparent cycle is: rule packs depend on `polint`; the engine must understand rule manifests and diagnostics; if the engine also depended on rule packs there would be a cycle. **There is no cycle**, because dependencies point one way: rule packs → SDK ← engine.

**Chosen graph (4 packages, 2 new):**

```
polint-macros   (proc-macro, unchanged)
        ▲
        │
polint-sdk      (NEW, thin)  ── rule packs depend on THIS
        ▲
        │
polint-engine   (NEW name for today's heavy crate content)
        ▲
        │
polint          (facade + `polint` binary; keeps the crates.io name and `cargo install polint`)
```

* `polint-sdk` depends on `polint-macros`, `serde`, `serde_json`, `thiserror`, `anyhow`, `globset`, `toml`. Nothing else.
* `polint-engine` depends on `polint-sdk` (snapshot/protocol/diagnostic types) plus everything heavy.
* `polint` depends on `polint-engine` and `polint-sdk`, owns `src/main.rs`, and re-exports `pub use polint_sdk::{sdk, runner, rule};` for backward compatibility.
* `polint-macros` gains **no** dependency on either — it emits `::polint::…` paths textually.

**The byte-identical trick.** Generated pack manifests change *one line*:

```toml
polint = { package = "polint-sdk", version = "0.3", default-features = false, features = ["lang-go", "lang-typescript"] }
```

Cargo's `package =` rename makes the extern crate name `polint`, so `use polint::sdk::prelude::*;`, `#[polint::rule]` (which expands to `::polint::sdk::__private::…`, `polint-macros/src/lib.rs:69, 82, 99-119`), and `polint::runner::run_cli` all stay byte-identical. **Rule `.rs` files do not change at all.** `polint-sdk` keeps `lang-go`/`lang-typescript` as accepted no-op features precisely so existing manifests keep resolving.

### 3.2 Alternatives considered and rejected

| Alternative | Why rejected |
| --- | --- |
| **A. Make `polint` itself thin; move engine + CLI to `polint-cli`** | Breaks `cargo install polint --locked` (`README.md:54`, `Makefile:6-7`, `tests/cargo_install_smoke.rs:36-48`) and every install doc. The crates.io name `polint` must keep meaning "the tool". |
| **B. Keep one crate; gate the engine behind a Cargo feature; have packs use `default-features = false`** | Feature unification: the moment anything in the same resolve graph enables `engine`, packs get it back. Also cannot drop `clap`/`tracing-subscriber`, which are unconditional dependencies today (`crates/polint/Cargo.toml`). |
| **C. Two crates only: `polint-sdk` + `polint` (heavy: engine + CLI + facade)** | Viable and simpler, but leaves the 238k-line engine and the 4.9k-line CLI inside the publish facade, so `cargo publish -p polint` keeps shipping everything and the engine can never gain independent versioning for prebuilt-artifact distribution (§6.10). The chosen graph keeps that door open for one extra package. |
| **D. Duplicate the SDK: thin `polint-sdk` for new packs, keep the fat `polint::sdk` for old packs** | Two `AnalysisDb` types, two fact-view implementations, two prelude allowlists, two leak gates. The engine has **no built-in rules** (`cli/mod.rs:3806`), so there is exactly one rule-execution surface and it should have exactly one implementation. |

### 3.3 Package count and public API deltas

| | Before | After |
| --- | --- | --- |
| Workspace member packages | 21 (4 + 17 example packs) | 23 |
| Published to crates.io | 2 (`polint`, `polint-macros`) | **3** (`polint`, `polint-macros`, `polint-sdk`); `polint-engine` is `publish = false` initially, promoted only if Phase J needs it |
| `polint::sdk::prelude` names | 116 | **116** (frozen; `tests/public_surface_leak.rs` unchanged) |
| New public API | — | `polint_sdk::protocol::*` and `polint_sdk::snapshot::*`, both `#[doc(hidden)]` |
| Removed public API | — | none |
| `polint::_bench` (`lib.rs:62-143`) | engine types | re-pointed at `polint_engine::_bench`; `polint-bench` unchanged |

`tests/internal_architecture.rs:4-12` (`REMOVED_PACKAGES`) stays as-is — `polint-sdk` and `polint-engine` are new names, not resurrections — and gains a positive assertion that the publishable set is exactly `{polint, polint-macros, polint-sdk}`.

---

## 4. Boundary design

### 4.1 What moves to `polint-sdk`

| From | To `polint-sdk` module | Notes |
| --- | --- | --- |
| `internal_core/{ids,span,lang,stable_key}.rs` (~330 LOC) | `polint_sdk::core` | verbatim; already serde |
| `internal_core/diagnostic.rs` (1,079) | `polint_sdk::diagnostics::model` | `Diagnostic`, `Severity`, `Label`, `Evidence`, `Fix`, `Suggestion`, `StructuredEvidenceV1`, `fingerprint`, `diagnostic_fingerprint` |
| `diagnostics/mod.rs` **types only**: `OutputFormat` (`:58-66`), `JsonReportMeta` (`:68-74`), `ColorChoice` (`:76-96`), `RenderOpts` (`:98-107`), `PolintToolInfo` (`:109-115`), `PolintReport` (`:117-124`), `RuleExecutionRow` (`:150-168`), `POLINT_REPORT_JSON_SCHEMA_V1_URL` (`:20-21`), `diagnostics_from_json_report` (`:227-230`), `sort_diagnostics` (`:647`), `dedupe_diagnostics` (`:678`) | `polint_sdk::diagnostics` | Renderers (`render_human` `:1081`, `render_sarif` `:1176`, `render_with_sarif_help` `:740`, AI-friendly builders `:326-511`) **stay in the engine** — they are `pub(crate)`. `RenderOpts.rule_execution` is `pub(crate)` (`:106`), so moving the type is safe. |
| `analysis_api/{syntax_facts,symbol_facts,module_facts,source_file}.rs` (~1,000) | `polint_sdk::facts` | fact row structs only; **not** `FactStore`, `FactMetaStore`, `Provider`, `ProviderCtx`, `FactDatabase` |
| `core/facts.rs`, `core/ids.rs`, `core/span.rs`, `core/lang.rs`, `core/stable_key.rs`, `core/review.rs`, `core/capability.rs` (289) | `polint_sdk::core` | `Capabilities`, `CapabilitySupport{,Status,View}` already serde (`core/capability.rs:15, 226, 238, 258`) |
| `core/rule.rs:18-249` (`RuleConfigValue`, `RuleKind`, `RuleMeta`, `Rule`, `RuleOptions`, `RuleCtx`) | `polint_sdk::rule` | `Rule::run` keeps `&AnalysisDb` — now the SDK's snapshot type |
| `core/rule.rs:277-410` (`run_rules*`, `rule_id_matches`, `internal_rule_error*`, `has_blocking_capability`) | `polint_sdk::rule::exec` | **drops `rayon`**: replace `par_iter()` (`:361-368`) with `std::thread::scope` over a pool sized by `std::thread::available_parallelism()`. Output order is already normalised by `dedupe_diagnostics` + `sort_diagnostics`. |
| `rule_error.rs` (20) | `polint_sdk::rule::error` | verbatim |
| `sdk/{mod,facts,policy,scope}.rs` (3,385) | `polint_sdk::sdk` | views re-pointed at the snapshot db; policy views become RPC (§4.5) |
| `rule_manifest.rs` (452) | `polint_sdk::manifest` | needed by **both** sides |
| `runner/mod.rs` (524) | `polint_sdk::runner` | rewritten as a protocol client (§5) |
| `path_context.rs:6-64` (`PathContextIndex`) | `polint_sdk::core::path_context` | drop the `PathContextsConfig`-taking `build` (engine-side); keep `related_paths` plus a `from_pairs` constructor for snapshot rehydration |
| `cache/mod.rs:872-885` `stable_hash` | `polint_sdk::hash::stable_hash` | **must keep separator byte `0xfe`** (`:881`), not `0xff` — `analysis_neutral/hash.rs:13` uses `0xff` and is a *different* function. `PolicyViolation::stable_key` (`sdk/policy.rs:161-182`) depends on the `0xfe` variant. |

**Total moved: ~7,000 LOC of the 238,359.**

**Why that boundary is the right one, checked rather than assumed.** `sdk/facts.rs`
— the file every rule actually reads facts through — imports exactly three things
beyond `crate::core` (`sdk/facts.rs:7-18`):

| Import | Disposition |
| --- | --- |
| `crate::sdk::policy::{EventPattern, FlowQuery, GuardQuery, LifecycleQuery, PolicyViolation, ReachQuery}` | moves to the SDK with the rest of `sdk/` |
| `crate::policy_queries::{matching_events, forbidden_reachable, missing_guards, missing_cleanup, forbidden_flows}` | becomes host RPC (§4.5) — the only five call sites |
| `crate::symbol_graph::query` | a two-line re-export of `analysis_neutral/symbol_graph/query.rs`, whose 8 public functions take `&dyn FactDatabase` and are reimplemented in the SDK against `&AnalysisDb` (D5) |

Nothing in the typed fact surface reaches `analysis_kernel`, `go`, `ts`,
`analysis`, `cache`, or `config`. That is what makes the split mechanical rather
than a redesign.

### 4.2 What stays in `polint-engine`

Everything else: `analysis/*`, `analysis_neutral/*`, `analysis_api/*` (contracts, stores, providers, digests), `analysis_kernel/*`, `analysis_plan.rs`, `frontend*/`, `go/`, `ts/`, `ir/`, `module_graph/`, `symbol_graph/`, `cache/`, `config/`, `fs/`, `git/`, `ignores.rs`, `baseline.rs`, `metrics.rs`, `measure.rs`, `repo_fs.rs`, `policy_queries.rs`, `core/db.rs`, `core/fact_store.rs`, `core/labels.rs`, `core/metadata.rs`, the diagnostic **renderers**, `rule_test.rs`, `golden_cost.rs`, and `cli/`.

**Rename:** engine-internal `core::AnalysisDb` (`core/db.rs:142-156`) becomes `polint_engine::core::HostFactDb`. This is not a public break: `AnalysisDb` is only reachable publicly via `polint::sdk::__private::AnalysisDb` (`sdk/mod.rs:66`, `#[doc(hidden)]`) and `polint::_bench::core::AnalysisDb` (`lib.rs:75`, feature-gated internal). After the split, `polint::sdk::__private::AnalysisDb` names the **SDK snapshot type**, which is exactly what the macro's generated `|db: &::polint::sdk::__private::AnalysisDb, …|` closure (`polint-macros/src/lib.rs:113`) needs. **The macro requires no change.**

### 4.3 `AnalysisDb` → `FactSnapshot` → snapshot-backed `AnalysisDb`

Three types, with the name preserved where it matters:

```rust
// polint-engine: the live, mutable, provider-facing database (today's AnalysisDb).
pub struct HostFactDb { /* core/db.rs:143-156 unchanged */ }

impl HostFactDb {
    /// Project the requested families into an owned, order-preserving snapshot.
    pub fn snapshot(&self, request: &SnapshotRequest) -> FactSnapshot;
}

// polint-sdk: the owned wire/disk form.
pub struct FactSnapshot { /* §4.4 */ }

// polint-sdk: what rules see. `polint::sdk::__private::AnalysisDb` == this.
pub struct AnalysisDb {
    snapshot: FactSnapshot,          // owned; every accessor borrows from it
    changeset: Option<ReviewChangeset>,
    path_contexts: Option<PathContextIndex>,
    policy: Option<PolicyChannel>,   // §4.5, RPC back to the host
}
```

`SnapshotRequest` is derived from the union of rule manifests, so **the whole fact DB is never serialized**:

```rust
pub struct SnapshotRequest {
    /// Capability names from `Capabilities::requested_names()` (core/capability.rs:193-222).
    pub families: BTreeSet<FactFamilyId>,
    /// Repo-relative globs from the resolved plan; families are filtered to these files.
    pub file_scope: Option<Vec<String>>,
    /// Include `SourceFile.source` text. Required whenever `syntax` is requested.
    pub include_source_text: bool,
}
```

**Lifetime / borrow preservation.** Today `SourceFiles<'a> { db: &'a AnalysisDb }` returns `self.db.files() -> &'a [SourceFile]`. After the change the identical code compiles: `AnalysisDb::files(&self) -> &[SourceFile]` returns a slice borrowed from the owned `FactSnapshot` field. The `'a` in every view is the borrow of `AnalysisDb`, unchanged. `FactView::build(db: &'a AnalysisDb)` (`sdk/facts.rs:1098-1145`) is unchanged. **No view signature or lifetime changes anywhere.**

Two accessors need explicit handling:

* `AnalysisDb::resolve_stable_key(id) -> Arc<str>` (used at `sdk/facts.rs:519, 524, 597`) — the snapshot carries `stable_keys: Vec<String>` indexed by `StableKeyId.0`; the SDK rebuilds a `StableKeyInterner` (`internal_core/stable_key.rs:31-84`) at load so ids resolve to identical text. Ordering in `references_for_file`/`unresolved_references` (`analysis_neutral/symbol_graph/query.rs:43-97`) depends on this text, so id→text fidelity is a correctness requirement.
* `AnalysisDb::definition_for_symbol(symbol)` (`sdk/facts.rs:563`) — an engine index today. The SDK rebuilds a `BTreeMap<SymbolId, usize>` at snapshot load (O(n) once) rather than scanning per call.

The 8 functions of `analysis_neutral/symbol_graph/query.rs` are **reimplemented in `polint_sdk::sdk::symbol_query`** against `&AnalysisDb` directly (335 LOC; pure filters plus one comparator). The SDK does **not** take `analysis_api::FactDatabase` (`analysis_api/provider/mod.rs:55-139`) — that trait has 30+ mutating provider methods and would drag the contract layer across the boundary. The engine keeps using the trait; the SDK gets a byte-equivalent copy whose equivalence is locked by a differential test (§9, T-EQ-3).

### 4.4 Snapshot format

**Container `PSNAP1`.** Written by the host to a file; the path is passed in the run request (not piped — §5.4).

```
offset  size  field
0       6     magic  b"PSNAP1"
6       2     container_version: u16 = 1            (little-endian, all ints LE)
8       4     header_len: u32
12      H     header: JSON (SnapshotHeader)
12+H    …     section payloads, contiguous, in header.sections order
```

```rust
pub struct SnapshotHeader {
    pub schema: String,              // "polint-fact-snapshot-v1"
    pub polint_version: String,      // engine CARGO_PKG_VERSION
    pub sdk_abi: u32,                // bumped on ANY fact-row field change
    pub run_id: String,
    pub plan_digest: String,         // AnalysisPlan::digest() (analysis_plan.rs:167)
    pub config_digest: String,       // cache::keys::config_hash
    pub content_digest: String,      // stable_hash over every section digest, in order
    pub sections: Vec<SnapshotSection>,
}

pub struct SnapshotSection {
    pub family: String,              // "source_files" | "imports" | "symbols" | …
    pub codec: SectionCodec,         // Json | Raw
    pub offset: u64,
    pub len: u64,
    pub row_count: u64,
    pub digest: String,              // stable_hash of the payload bytes
}
```

**Section list** — `family` values are exactly the `Capabilities::requested_names()` strings from `core/capability.rs:193-222`, plus three infrastructure sections:

| Section | Payload type | Gated on capability | Engine source |
| --- | --- | --- | --- |
| `source_files` | `Vec<SourceFileWire>` | always | `HostFactDb::files()` |
| `stable_keys` | `Vec<String>` | when `symbols`/`references` | `StableKeyInterner` |
| `path_contexts` | `PathContextIndexWire` | when configured | `path_context.rs:7-10` |
| `syntax` | `Vec<PackageFact>`, `Vec<FunctionFact>` | `syntax` | `analysis_api/syntax_facts.rs:13-40` |
| `imports` | `Vec<ImportFact>` | `imports` | `:44-51` |
| `resolved_imports` | `Vec<ResolvedImportFact>` | `resolved_imports` | `analysis_api/module_facts.rs` |
| `module_graph` | `Vec<ModuleNode>`, `Vec<ModuleEdge>` | `module_graph` | ditto |
| `symbols` | `Vec<SymbolFact>`, `Vec<DefinitionFact>` | `symbols` | `analysis_api/symbol_facts.rs` |
| `references` | `Vec<ReferenceFact>` | `references` | ditto |
| `branch_obligations` | `Vec<BranchObligation>` | `branch_obligations` | `syntax_facts.rs:55-64` |
| `go_tests` | `Vec<TestFact>` | `go_tests` | `:71-82` |
| `coverage_facts` | `Vec<CoverageFact>` | `coverage_facts` | `:86-90` |
| `string_literals` | `Vec<StringLiteralFact>` | `string_literals` | `:94-99` |
| `ts_components` | `Vec<TsComponentFact>` | `ts_components` | `:103-108` |
| `ts_classes` | `Vec<TsClassFact>` | `ts_classes` | `:112-118` |
| `jsx_attributes` | `Vec<JsxAttributeFact>` | `jsx_attributes` | `:122-127` |
| `file_metrics` / `function_metrics` / `complexity_metrics` | respective `Vec<…Fact>` | same names | `symbol_facts.rs` |

Not sections: `changeset` and `capability_support` travel in the **run request** (§5.3), because the changeset is deliberately excluded from cache identity (`core/db.rs:149-155`) and the support view is plan metadata, not facts. `cfg`, `call_graph`, and `test_suite_metrics` have no rows today (reserved views, `sdk/facts.rs:869-888, 983-988`) and get no section.

`SourceFileWire` mirrors `analysis_api/source_file.rs:11-18` but is serde-able (the live type is not, because of `Arc<str>`): `{ id: FileId, path: String, relative_path: String, language: Language, content_hash: String, source: Option<String> }`. `source` is `Some` iff `include_source_text`; the loader turns it into `Arc<str>` — one allocation per file, exactly what `HostFactDb` does today, so no regression.

**Codec decision.** v1 uses `SectionCodec::Json` (`serde_json`, already a dependency; zero new deps). The per-section `codec` field exists so switching to a binary codec is a section-level change, not a redesign. Adopting `postcard` is a **decision gate requiring benchmark evidence** (§10.5, KC-4): adopt only if snapshot encode+decode exceeds 15% of warm end-to-end wall-clock on the medium corpus.

**Version negotiation.** Mismatch in `magic`, `container_version`, `schema`, or `sdk_abi` is a hard, non-silent error (§5.7). `content_digest` mismatch on read is corruption → hard error. Never "best-effort decode".

### 4.5 Policy views: host-side RPC

`Events`, `Calls`, `ControlFlow`, `DataFlow` (`sdk/facts.rs:883-962`) delegate to `policy_queries::{matching_events, forbidden_reachable, missing_guards, missing_cleanup, forbidden_flows}` (`policy_queries.rs:23-49`). That module is 3,580 LOC over `DataFlowStore`, IFDS taint search (`analysis/ifds`), reachability roots, refined calls, places, and MIR ids (`policy_queries.rs:1-21`). Those stores cannot move to the SDK, and the queries take **runtime-constructed patterns** (`FlowQuery::new(SourcePattern::http_request(), SinkPattern::call("readFile"))`, `cli/mod.rs:1750-1755`), so precomputation is impossible.

**Decision: the four policy view methods become synchronous RPC to the host.**

```rust
// polint-sdk
impl<'a> DataFlow<'a> {
    pub fn forbidden(self, query: FlowQuery) -> Vec<PolicyViolation> {
        self.db.policy_query(PolicyRequest::Flow(query))   // blocks on the channel
    }
}
```

Rule source is unaffected — the signature is identical (`sdk/facts.rs:959-961`).

Required changes:

* Add `Serialize`/`Deserialize` to `ReachQuery`, `GuardQuery`, `LifecycleQuery`, `FlowQuery`, `EventPattern`, `SourcePattern`, `SinkPattern`, `GuardPattern`, `BarrierPattern`, `PolicyStatus`, `PolicyPrecision`, `PolicyConfidence`, `PolicyOperation`, `PolicyViolation` (`sdk/policy.rs:16-620`). All are plain data with private fields and `new()` constructors; derives are additive and do not change the prelude allowlist (trait impls are not names in `tests/public_surface_leak.rs`).
* `PolicyViolation` round-trips its private fields (`sdk/policy.rs:92-101`) so `stable_key()` (`:161-182`) and `diagnostic()` (`:195-210`) produce identical output on the rule side. Locked by a proptest round-trip (§9, T-EQ-5).
* The engine gains `PolicyServer::answer(&HostFactDb, PolicyRequest) -> Vec<PolicyViolation>` — a thin dispatch over the five existing entry points. No query logic moves or changes.
* Concurrency: `PolicyChannel` is a `Mutex<Framed>`; a query serialises the round-trip. Rules run concurrently, but concurrent policy queries block on each other. This is deliberate: it avoids a response-multiplexing dispatcher and keeps determinism trivially. Policy queries are seconds-scale IFDS searches, so contention is not the bottleneck. If measurement contradicts that (§13.3, D-2), the fallback is `request_id`-tagged frames plus a reader thread — designed, not built.
* If the host declines a query (support view says unsupported), it returns `Vec::new()`, matching today's behaviour where unsupported patterns yield no matches (`sdk/policy.rs:1-7`).

### 4.6 Capability support, options, changeset, diagnostics

* **Capability support**: `CapabilitySupportView` is already `Serialize + Deserialize` (`core/capability.rs:258-289`) → travels verbatim in the run request; `RuleCtx::with_capability_support` (`core/rule.rs:171-184`) is unchanged.
* **Options**: `RuleOptions` (`core/rule.rs:129-148`) is **not** serde today (it holds `toml::Value`). Add a crate-internal `RuleOptionsWire` in `polint_sdk::protocol` with exactly the 8 fields (`severity`, `files`, `allow_files`, `allow`, `max`, `deny`, `forbidden_imports`, `settings`) plus `From`/`Into`. Do **not** derive serde on the public `#[non_exhaustive]` `RuleOptions` — that would add public trait impls to a frozen surface. `settings: BTreeMap<String, toml::Value>` serialises because `toml::Value` is serde; this is why `polint-sdk` keeps the `toml` dependency (closure: `indexmap`, `serde_core`, `serde_spanned`, `toml_datetime`, `toml_parser`, `toml_writer`, `winnow`).
* **Review changesets**: `ReviewChangeset`/`ChangedFile`/`ChangeStatus` (`core/review.rs`) are already serde and already written to a file by the parent (`write_review_changeset`, `cli/mod.rs:4181-4199`). They move to the SDK and travel **inline in the run request**, replacing the `--changed-files <path>` flag (`runner/mod.rs:96-97, 427-430`). Excluded from `content_digest`, preserving `core/db.rs:149-155`.
* **Diagnostics**: the rule process returns `Vec<Diagnostic>` in the run response; the **host** computes `RuleExecutionRow`s via `AnalysisPlan::rule_execution_rows(&options, &file_paths, &diagnostics)` (`analysis_plan.rs:221`). This deletes the round-trip through `PolintReport` JSON on stdout (`runner/mod.rs:284` → `cli/mod.rs:4337`) and removes the report renderers from the SDK. The `PolintReport` *type* still lives in the SDK because it is in the prelude allowlist (`tests/public_surface_leak.rs:107`).

### 4.7 Thin-SDK dependency budget

| Dep | Why | Notes |
| --- | --- | --- |
| `serde` + `serde_json` | snapshot, protocol, diagnostics | unavoidable |
| `thiserror` | `RuleError` (`rule_error.rs:14-17`) | tiny |
| `anyhow` | `RuleError(#[from] anyhow::Error)`; authors use `bail!` | public contract |
| `toml` | `RuleConfigValue = toml::Value` (`core/rule.rs:22`) | public contract; ~10 units |
| `globset` | `sdk/scope.rs:36` `file_in_scope`/`glob_matches` | prelude names |
| `polint-macros` | `#[polint::rule]` | proc-macro (`syn`/`quote`/`proc-macro2`) — already paid today |

**Dropped versus today's `polint` dependency list** (`crates/polint/Cargo.toml`): `clap` (the rule binary's argv becomes a fixed protocol handshake parsed by ~40 lines of hand-rolled matching; it was never a user-facing CLI, see `runner/mod.rs:22-36`), `tracing` + `tracing-subscriber` (`runner/mod.rs:145-149` → an env-gated `eprintln!` behind `POLINT_RULE_LOG`), `rayon` (§4.1), `rusqlite` (bundled SQLite, a C build), `petgraph`, `ignore`, `oxc_*` (×18), `tree-sitter*` (×2), `json-strip-comments`, `serde_norway`, `tempfile`, `libc`/`rustix`/`windows-sys`.

**Budget: ≤ 35 compiled units for `polint-sdk` + rule crate**, enforced by Phase B's guard test.

---

## 5. Rule protocol design

### 5.1 Shape: one process, framed stdio

**Decision: one rule process per rule pack per `polint` invocation**, spawned once, driven through several request/response exchanges, then shut down. Not two (a "manifest" process then a "run" process) — that re-pays process start and, worse, re-pays a Cargo freshness check per phase, which is exactly what today's `inspect` + `check` double-`cargo run` (`cli/mod.rs:4350-4407` and `:4252-4343`) costs.

Framing on stdio, modelled on the extension host (`analysis/extensions/host.rs`):

* **host → rule**: stdin, length-prefixed frames — `u32` LE byte length, then JSON.
* **rule → host**: stdout, same framing.
* **stderr**: free-form; captured, bounded, surfaced only on failure.

Length-prefixing (rather than the extension host's one-shot `serde_json::from_slice` over all of stdout, `host.rs:199`) is required because the rule process emits an unbounded number of policy-query frames interleaved with the final result.

### 5.2 Manifest handshake

```rust
// host → rule, first frame
struct HelloRequest {
    schema: String,           // "polint-rule-host-hello-v1"
    host_version: String,
    sdk_abi_min: u32,
    sdk_abi_max: u32,
}
// rule → host
struct HelloResponse {
    schema: String,           // "polint-rule-host-hello-v1"
    sdk_version: String,      // polint-sdk CARGO_PKG_VERSION
    sdk_abi: u32,
    pack_name: String,
    rules: Vec<RuleManifest>, // rule_manifest.rs:11-21, one per registered Rule
}
```

`RuleManifest` already carries `id`, `description`, `severity`, `fact_views`, `capabilities`, `options`, `sdk_version`, and its own `schema_version` (`RULE_MANIFEST_INTERNAL_SCHEMA`, `rule_manifest.rs:8`). `Rule::manifest(options)` (`core/rule.rs:110-117`) is unchanged.

The handshake **replaces both** `polint inspect rule` (`cli/mod.rs:4350-4407`, `runner/mod.rs:175-208`) and the implicit capability discovery that `RulePlanInputs::collect` (`analysis_plan.rs:463`) does today inside the child. The host now plans from the handshake, then produces exactly the facts the plan needs.

`polint inspect rule --format json` output (`InspectRuleReport`, `rule_manifest.rs:162-243`) is rebuilt **by the host** from `HelloResponse.rules` plus `plan.support_view()` via the existing `RuleManifestWire::from_manifest` (`rule_manifest.rs:245-302`). Byte-identical output; `host_path` is set by the host as it is today (`:304-307`).

### 5.3 Run request

```rust
struct RunRequest {
    schema: String,                     // "polint-rule-host-run-v1"
    run_id: String,
    kind: RuleKind,                     // Check | Review  (core/rule.rs:29-37)
    snapshot_path: String,              // absolute; §4.4 container
    snapshot_content_digest: String,    // must equal SnapshotHeader.content_digest
    enabled_rule_ids: Vec<String>,      // exact ids after profile + --only-rule
    options: BTreeMap<String, RuleOptionsWire>,
    capability_support: CapabilitySupportView,
    runtime_blocked_rules: BTreeSet<String>,
    changeset: Option<ReviewChangeset>,
    parallel: bool,
    policy_enabled: bool,               // false ⇒ policy views return empty without a round-trip
    deadline_ms: u64,
}

struct RunResponse {
    schema: String,                     // "polint-rule-host-run-v1"
    run_id: String,
    diagnostics: Vec<Diagnostic>,
    rules_executed: Vec<String>,        // ids that actually ran (for RuleExecutionRow)
}
```

Every field maps to something the host passes today: `enabled_rule_ids` ← `exact_enabled` (`runner/mod.rs:408`), `options` ← `plan_inputs.rule_options_from_config` (`:409`), `capability_support`/`runtime_blocked_rules` ← `KernelOutput` (`analysis_kernel/mod.rs:362-375`), `changeset` ← `--changed-files` (`runner/mod.rs:427-430`), `kind` ← `--kind` (`:93-95`).

### 5.4 Snapshot transfer

**File, not pipe.** The host writes `<cache>/snapshots/<content_digest>.polintsnap` with mode `0600` (Unix) via the existing atomic-write helper family in `repo_fs.rs`, and passes the path. Reasons: (a) a repo with several rule packs (`discover_local_rule_hosts`, `cli/mod.rs:3872-3882`) writes once and every pack reads the same file; (b) multi-MB payloads through a pipe interleave badly with policy-query frames on the same fd; (c) a file is inspectable when debugging.

Lifetime: deleted after the last pack's `RunResponse`, unless `POLINT_KEEP_SNAPSHOT=1`. The `snapshots` directory joins `CacheManagedCategory` (`cache/mod.rs:651-696`) with role `Scratch`, which automatically gives it `polint cache status|clean|prune` coverage and excludes it from `action.yml` caching (`cache/mod.rs:616-643`). The contract test `cache_layout_matches_the_github_action_contract` enforces that a new directory cannot appear in one place and be missing from the others.

### 5.5 Policy query frames

```rust
struct PolicyQueryFrame {                  // rule → host, mid-run
    schema: String,                        // "polint-rule-host-policy-v1"
    request_id: u64,
    request: PolicyRequest,                // Events|Reach|Guard|Lifecycle|Flow + query
}
struct PolicyResultFrame {                 // host → rule
    schema: String,
    request_id: u64,
    violations: Vec<PolicyViolation>,
    truncated: bool,                       // host-side budget hit
}
```

The host answers on the main thread between `RunRequest` and `RunResponse`; `HostFactDb` is fully built at that point, so answering is a pure read.

### 5.6 Limits, timeouts, cancellation

| Control | Value | Precedent |
| --- | --- | --- |
| stdout frame max | 64 MiB (run response) / 8 MiB (policy result) | extension host uses 1 MiB (`host.rs:18`); diagnostic volumes are larger |
| stderr capture | 16 KiB, truncated with a marker | `host.rs:19` |
| total run deadline | 600 s default, `POLINT_RULE_TIMEOUT_SECS` override | extension host: 30 s (`host.rs:17`), too short for whole-repo rule execution |
| per-policy-query deadline | 60 s, then `truncated: true` + empty | new |
| child env | `env_clear()` + allowlist | `EXTENSION_ENV_ALLOWLIST`, `host.rs:332-369`, extended with `POLINT_RULE_LOG` |
| process containment | Job Objects (Windows) / process-group kill (Unix) | `windows-sys` `Win32_System_JobObjects` is already a dependency of `crates/polint`; `host.rs:508` |

**Cancellation.** On host SIGINT or deadline: close stdin (the rule's next read returns EOF → `run_cli` exits 0 with no response), wait 2 s, then kill the process group. The rule process treats stdin EOF mid-run as "abandon, exit" and must not write partial results.

### 5.7 Exit and error protocol

| Condition | Rule exit | Host behaviour |
| --- | --- | --- |
| normal | 0 after `RunResponse` | proceed |
| `sdk_abi` outside `[sdk_abi_min, sdk_abi_max]` | 3, no frames | hard error naming both versions + `cargo update -p polint-sdk` remedy |
| snapshot digest mismatch / corrupt container | 4 | hard error; delete the snapshot; suggest `polint cache clean --category snapshots` |
| unknown schema string | 5 | reuse `ExtensionProtocolError::UnsupportedProtocol` wording (`extensions/protocol.rs:96-128`) |
| rule panicked | 0, diagnostics contain `internal/<rule_id>` | already the behaviour (`core/rule.rs:354-358`); preserved verbatim |
| rule returned `Err` | 0, `internal/<rule_id>` diagnostic | `core/rule.rs:356` |
| binary missing/unexecutable | (spawn error) | fall back to a Cargo build (§6.3), then retry once |
| non-zero exit without frames | — | reuse `rules_host_error_message` (`cli/rules_host_error.rs:8-37`) so MSRV/network/manifest/rustc hints survive |

**Version negotiation** is `sdk_abi` (a `u32` bumped on any fact-row field change), not semver strings. The host declares an accepted range; the rule declares one number. This lets one prebuilt engine serve packs pinned to several SDK patch versions.

**Determinism.** The rule process must produce identical `RunResponse.diagnostics` for identical `(snapshot bytes, RunRequest)`. Enforced by: `dedupe_diagnostics` + `sort_diagnostics` before responding (`diagnostics/mod.rs:647, 678`), no wall-clock or RNG in the SDK, `BTreeMap`/`BTreeSet` everywhere, and the thread-scope executor collecting per-rule outputs into an index-ordered `Vec` before flattening (mirroring `core/rule.rs:361-368`).

### 5.8 Compatibility with current JSON schemas

* `polint check --format json` — the **host** renders `PolintReport` exactly as today (`diagnostics/mod.rs:199-218`), now from the run response instead of parsing the child's stdout. `docs/schemas/polint-report-v1.json` unchanged.
* `polint inspect rule --format json` — host-rendered from `HelloResponse` (§5.2). `docs/schemas/polint-rule-inspect-v1.json` unchanged.
* `polint test` — `RuleTestReport` (`rule_test.rs:52-59`) unchanged; only the execution engine underneath changes.
* AI-friendly and SARIF are host-side already (`cli/mod.rs:4006-4028`).

---

## 6. Build and artifact lifecycle

### 6.1 Source fingerprint

One key decides whether a rule binary may be executed without Cargo.

```rust
// polint-engine::rules_artifact
pub const RULE_ARTIFACT_KEY_SCHEMA: &str = "polint-rule-artifact-key-v1";

pub struct RuleArtifactKeyInputs<'a> {
    pub schema: &'a str,               // RULE_ARTIFACT_KEY_SCHEMA
    pub sdk_abi: u32,                  // §4.4
    pub sdk_version: &'a str,          // polint-sdk CARGO_PKG_VERSION the host ships
    pub host_triple: &'a str,          // env!("TARGET") captured in polint-engine's build.rs
    pub rustc_verbose_digest: &'a str, // stable_hash of the full `rustc -vV` stdout
    pub cargo_profile: &'a str,        // "dev" | "release" | custom (cli/mod.rs:4417-4439)
    pub rustflags_digest: &'a str,     // RUSTFLAGS + CARGO_ENCODED_RUSTFLAGS + `[build] rustflags`
    pub manifest_digest: &'a str,      // bytes of <pack>/Cargo.toml
    pub lockfile_digest: &'a str,      // bytes of <pack>/Cargo.lock, or "absent"
    pub toolchain_digest: &'a str,     // <repo>/rust-toolchain.toml + <pack>/rust-toolchain.toml
    pub source_tree_digest: &'a str,   // §6.2
    pub target_dir_digest: &'a str,    // canonicalized CARGO_TARGET_DIR (cache/mod.rs:382-388)
}
```

`key = cache::stable_hash(&[...])` (`crates/polint/src/cache/mod.rs:874-885`, the `0xfe` variant), rendered as the 16-hex-char string the codebase already uses for cache filenames (`cli/mod.rs:4191-4194`, `:106-107`).

Deliberately **excluded**: `.polint.toml` contents, `--profile`, `--only-rule`, the file set, the changeset. Those change *what the host asks the rule binary to do*, never *what the binary is*; including them would defeat the cache on every scope change.

Deliberately **included**: `rustc -vV` in full (it carries the commit hash and host triple), and `target_dir_digest`, because Cargo's own freshness lives in that directory and `POLINT_RULES_TARGET_DIR` (`cache/mod.rs:14, 382-388`) can relocate it.

### 6.2 `source_tree_digest`

Deterministic walk of the rule package directory:

1. Enumerate recursively from `<pack>/`, skipping `target/` and anything named in `<pack>/.polintignore` if present.
2. Reject before hashing: any path `crate::repo_fs::normalize_repo_relative_input` rejects; any symlink whose resolved target escapes `<pack>/`; any non-file, non-dir entry.
3. Sort by repo-relative, `/`-normalized path.
4. For each: feed `path`, `"x"`/`"-"` (Unix executable bit), `len`, and `stable_hash` of contents.
5. `stable_hash` of the concatenation.

Content-based, not mtime-based, so a `git checkout` restoring identical bytes is a cache hit — the case CI hits constantly and where `Swatinem/rust-cache` + `actions/cache` still pay a Cargo freshness scan today.

### 6.3 Current-artifact detection and direct execution

Layout under the cache root (`CacheLayout`, `cache/mod.rs:344-392`):

```
.polint/cache/rules-bin/<pack-id>/<key>/polint-local-rules[.exe]
.polint/cache/rules-bin/<pack-id>/<key>.json          # RuleArtifactRecord
```

`<pack-id>` = `sanitize_name` (`cli/mod.rs:4470-4480`) of the pack's repo-relative directory.

```rust
pub struct RuleArtifactRecord {
    pub schema: String,          // "polint-rule-artifact-v1"
    pub key: String,
    pub binary_relative: String,
    pub binary_sha256: String,   // sha2, engine-only dep (§6.11)
    pub binary_len: u64,
    pub sdk_abi: u32,
    pub built_unix: u64,
    pub rule_ids: Vec<String>,   // from the post-build handshake; enables `inspect` without a spawn
}
```

`resolve_rule_binary(pack) -> RuleBinary` decision table:

| Check | Failure action |
| --- | --- |
| record file parses, `schema` matches | rebuild |
| `record.key == recomputed key` | rebuild |
| binary exists, is a regular file, not a symlink | rebuild |
| `binary_len` and `binary_sha256` match | rebuild + `polint/cache` warn diagnostic |
| parent directory not group/world-writable (Unix) | hard error, refuse to execute (§7.8) |
| `record.sdk_abi` inside the host's accepted range | rebuild |

On success the host executes the binary directly:

```
<binary> --polint-rule-protocol 1
```

No Cargo process, no manifest parse, no lockfile read, no registry touch. `POLINT_RULE_LOG` is the only optional extra env beyond the allowlist (§5.6).

On any rebuild path:

```
cargo build --locked --quiet
      --manifest-path <pack>/Cargo.toml
      --profile <dev|release|custom>
      --message-format json-render-diagnostics
      --config <cache>/rules-cargo/config.toml        # §6.6
      [--offline]                                     # §6.5
```

The host parses the stream for `{"reason":"compiler-artifact", "target":{"kind":["bin"],…}, "executable":"…"}`, hard-links (falling back to copy) the executable into `<key>/`, computes the sha256, writes the record with the existing atomic-write helpers (`repo_fs::write_repo_file_atomic`), then runs the handshake and back-fills `rule_ids`.

`cargo build` replaces `cargo run` deliberately: `cargo run` re-checks freshness and then execs the binary itself, so the host never learns the artifact path and can never skip Cargo next time. **This single change is what makes the whole cache possible.**

### 6.4 Single build for all fixture cases

Today `run_rule_tests` (`rule_test.rs:184-195`) loops cases; `run_case` (`:244-303`) spawns per rule-host manifest; `run_rule_host_check` (`:323-373`) does a `cargo run` per case with `CARGO_TARGET_DIR` = the *repo's* `rules-target` and `POLINT_CACHE_DIR` = the *temp fixture's* cache root.

New shape:

1. `run_rule_tests` resolves each manifest in `options.rule_host_manifests` (`rule_test.rs:108`) to a `RuleBinary` **once**, before the case loop.
2. For each case: copy the fixture tree (`copy_fixture_tree`, `:375`), `ensure_case_config`, build a snapshot for the temp repo with the engine, and drive the already-resolved binary through the protocol.
3. The rule process is **spawned once per manifest** for the whole suite and receives N `RunRequest` frames (one per case), each with its own `snapshot_path`. `enabled_rule_ids` carries the case's `--only-rule` equivalent (`rule_test.rs:346-348`).

Cargo invocations for `polint test`: **N × M → at most M** (M = number of rule packs), and 0 when artifacts are current. `RuleTestReport` (`rule_test.rs:52-59`) and `docs/schemas/polint-test-report-v1.json` are unchanged.

### 6.5 Offline and locked operation

| Flag | When |
| --- | --- |
| `--locked` | always when `<pack>/Cargo.lock` exists; matches `Makefile:11-19` and every CI job |
| `--offline` | when `POLINT_RULES_OFFLINE=1`, or `CARGO_NET_OFFLINE=true`, or the vendored SDK (§6.6) is in use and every other dependency resolves from the lockfile |
| `--frozen` | when `POLINT_RULES_OFFLINE=strict` — locked + offline, refusing any lockfile change |

`rules_host_error::is_network_error` (`cli/rules_host_error.rs:74-82`) already classifies fetch failures; the offline modes add a fourth hint: *"the rule pack needs a dependency that is not vendored or in the lockfile; run `polint rules vendor` or unset `POLINT_RULES_OFFLINE`."*

### 6.6 Vendored SDK

The `polint` binary embeds `polint-sdk` and `polint-macros` sources (produced at release time by `cargo package --list`-verified tarballs, checked into `crates/polint/assets/sdk-<version>.tar` and included via `include_bytes!`). On first use per version:

1. Materialize to `<user-cache>/sdk/<sdk-version>/{polint-sdk,polint-macros}/` (§6.8), atomically, `0755` dirs / `0644` files.
2. Generate `<cache>/rules-cargo/config.toml`:

```toml
[patch.crates-io]
polint-sdk    = { path = "<user-cache>/sdk/<v>/polint-sdk" }
polint-macros = { path = "<user-cache>/sdk/<v>/polint-macros" }
```

3. Pass it with `cargo build --config <path>`.

Effect: a rule pack builds with **no registry access at all** for the polint half of its graph, and the SDK version is always the one the host understands, so `sdk_abi` can never drift. If a pack pins a different `polint-sdk` version, `[patch]` overrides it and the host emits a `polint/rules-sdk-patched` info diagnostic naming both versions.

Decision-log item **D-7**: `[patch]` in a `--config` file must be confirmed against the pinned MSRV toolchain (`rust-toolchain.toml`; MSRV 1.95 per `Cargo.toml` `[workspace.package] rust-version`). Fallback if unsupported: generate a throwaway workspace manifest under `<cache>/rules-cargo/<pack-id>/Cargo.toml` with `[patch]` plus `[workspace] members = ["<abs path to pack>"]`, and build that. Never rewrite the user's `<pack>/Cargo.toml` — it is user-owned (`ARCHITECTURE.md:40`).

### 6.7 Target directory and cleanup

`CARGO_TARGET_DIR` stays `<cache>/rules-target` (`cache/mod.rs:370-388`), because `action.yml:210-220` restores/saves that exact path and `crates/polint/tests/github_action_cache.rs` (1,119 lines) asserts the contract.

New: after a successful build **and** artifact capture, the host prunes the pack's own output from `rules-target` — the same thing `scripts/action/prepare-build-cache-save.sh` does at save time — because the artifact under `rules-bin` is now the authoritative copy. Dependency intermediates stay, so an SDK-level rebuild is still incremental. This is what turns "hundreds of MB retained" into "dependency intermediates plus a few MB of binaries".

Two new `CacheManagedCategory` variants (`cache/mod.rs:651-696`); adding them is what wires `polint cache status|clean|prune` and the action contract test automatically:

| Variant | `name()` | `role()` | Cached by `action.yml`? |
| --- | --- | --- | --- |
| `RulesBin` | `"rules-bin"` | `CompilerOutput` | yes, same key family as `rules-target` |
| `Snapshots` | `"snapshots"` | `Scratch` | no (like `Review`, `cache/mod.rs:411-419`) |

`CacheCategoryArg` in `cli/mod.rs` must gain matching values — `cache_category_arg_covers_every_managed_category` (`cli/mod.rs:4497-4530`) fails the build otherwise, which is exactly the guardrail wanted.

### 6.8 User-level cache

`POLINT_USER_CACHE_DIR`, defaulting to `$XDG_CACHE_HOME/polint` → `~/.cache/polint` (Unix), `~/Library/Caches/polint` (macOS), `%LOCALAPPDATA%\polint\cache` (Windows).

Holds **only version-addressed, non-repo-specific content**:

* `sdk/<sdk-version>/` — materialized vendored SDK sources (§6.6).
* `artifacts/<key>.polintrule` — downloaded prebuilt rule artifacts (§6.10).

It deliberately does **not** hold locally-built rule binaries. Sharing those across repos would require the artifact key to be path-independent, which would mean `--remap-path-prefix` and therefore wrong paths in `Diagnostic.file`. Repo-local `rules-bin` stays repo-local.

### 6.9 Disk ceilings and LRU

`CacheLayout::prune` (`cache/mod.rs:494-579`) already implements exactly the needed policy: collect files, sort by `(mtime, path)`, evict oldest-first until under `max_bytes`, plus an optional `max_age`. Reuse it verbatim.

| Directory | Default ceiling | Env override | Enforcement point |
| --- | --- | --- | --- |
| `rules-bin` | 512 MiB | `POLINT_RULES_BIN_MAX_MB` | after each successful artifact write |
| `rules-target` | none by default (Cargo owns it) | `POLINT_RULES_TARGET_MAX_MB` | after each Cargo build |
| `snapshots` | 256 MiB | `POLINT_SNAPSHOT_MAX_MB` | after each run, plus unconditional deletion of the run's own snapshot |
| `<user-cache>/artifacts` | 1 GiB | `POLINT_ARTIFACT_CACHE_MAX_MB` | after each download |

LRU touch: on an artifact cache hit, `File::open` + `set_modified` on the record file, so prune ordering is genuinely last-*used*, not last-*built*.

CI already has the ceiling knob (`action.yml:30-33`, `build-cache-max-size-mb`, default `""`); it gains `rules-bin` in the same restore/save paths and its size in the `rule-build-cache-size-mb` output.

### 6.10 Prebuilt artifacts for non-authors and CI

For a team, only a handful of people edit `.polint/rules`. Everyone else, plus CI, should download rather than compile.

`polint rules build --emit-artifact <dir>` produces `<dir>/<key>.polintrule` — an uncompressed tar containing:

```
manifest.json     # RuleArtifactRecord + HelloResponse.rules (so `inspect` needs no spawn)
bin/polint-local-rules[.exe]
```

Consumption is declared in `.polint.toml`:

```toml
[rules]
paths = [".polint/rules"]
execution = "artifact-preferred"        # native | artifact-preferred | artifact-only | none
artifact_sources = ["https://artifacts.example.com/polint/${key}.polintrule"]
```

Resolution order under `artifact-preferred`: repo-local `rules-bin` → `<user-cache>/artifacts` → each `artifact_sources` entry (`${key}` substituted) → local Cargo build. Under `artifact-only`, a miss is a hard error naming the exact `polint rules build` command the pack owner should run.

Because the key includes `rustc_verbose_digest` and `host_triple`, a downloaded artifact can only match a machine with the same toolchain and target — the correct, conservative behaviour. Teams wanting broad reuse pin a toolchain in `rust-toolchain.toml`, which `ensure_repo_rust_toolchain_shim` (`cli/mod.rs:723-742`) already writes when absent.

### 6.11 Signing and digests

* Every artifact carries `binary_sha256` inside `manifest.json`.
* A repo pins what it accepts in **`.polint/rules-artifacts.lock`** (tracked in git):

```toml
schema = "polint-rules-artifacts-lock-v1"

[[artifact]]
pack = ".polint/rules"
key = "9f31c0a4b7de2211"
sha256 = "…"
```

* `artifact-preferred`/`artifact-only` **refuse any downloaded artifact absent from the lock**, unless `--trust-artifacts` is passed explicitly on that invocation. There is deliberately no env var for this — an ambient variable that disables integrity checking is a footgun.
* The engine gains `sha2` (pure Rust, ~4 units, engine-only, never `polint-sdk`). The repository already treats sha256 as the release-asset convention (`scripts/sha256_file.py`).
* Detached ed25519 signatures are **deferred** (D-11): they need a key-distribution story that digest pinning does not, and pinning already defeats substitution for the concrete threat (§7.8).

### 6.12 Explicit native trust mode

`execution` in `[rules]` is the single switch, with the CLI override `--rules-execution <mode>`:

| Mode | Build repo-local rules? | Execute a native binary? | Intended for |
| --- | --- | --- | --- |
| `native` | yes | yes | your own repo (default) |
| `artifact-preferred` | yes (fallback) | yes | CI on your own repo |
| `artifact-only` | no | yes, pinned artifacts only | consumers of a shared rule pack |
| `none` | no | no | scanning a repository you do not trust |

`--untrusted` is sugar for `--rules-execution none` plus a `polint/untrusted-rules` info diagnostic per skipped pack, so the report says plainly which policy did not run. The engine-only analysis still runs (`analyze_and_run`, `cli/mod.rs:3794-3829`, already works with zero rules).

---

## 7. Security threat model and controls

### 7.1 Scope

**Asset:** the developer workstation or CI runner executing `polint check`, its credentials, and its network position.
**Adversary:** the author of a repository being scanned. Anything under the repo root is attacker-controlled: `.polint.toml`, `.polint/rules/**`, `rust-toolchain.toml`, `.cargo/config.toml`, and every source file.
**Trust anchor:** the `polint` binary and the SDK sources it embeds.

### 7.2 The honest baseline

`polint check` on an untrusted repo has **always** been equivalent to `cargo build` on that repo, because `run_local_rule_host_kind` (`cli/mod.rs:4252-4343`) spawns `cargo run` on attacker-authored manifests. This plan does not make that worse, and adds two genuinely new mitigations: an explicit no-build mode (§6.12) and manifest validation (§7.3). It does **not** claim to make native mode safe against a hostile repo.

### 7.3 Attacker-controlled `Cargo.toml`

Before any build, `validate_rule_manifest(<pack>/Cargo.toml)` rejects:

| Construct | Reason |
| --- | --- |
| any `path` dependency resolving outside the repo root | reads/links code the user did not check out |
| `[patch]` / `[replace]` sections | would override the vendored SDK (§6.6) and re-point `polint-sdk` |
| `[source]` replacement | redirects the registry |
| `git` dependencies (unless `POLINT_RULES_ALLOW_GIT=1`) | fetches unpinned remote code at build time |
| `[workspace]` inheriting from outside the repo | pulls in an unseen manifest |
| non-`bin` target kinds, `proc-macro = true`, `crate-type = ["cdylib"]` in the pack itself | the pack is a rule host, nothing else |

Violations produce a `polint/rules-manifest` **error** diagnostic naming the offending key and line, and the pack is skipped — the scan continues with the other packs, mirroring the transactional-refusal style already used by the scaffolder (`cli/mod.rs:871-895`). The generated manifest (`pack_cargo_toml`, `cli/mod.rs:1130-1158`) and all 17 example packs pass unchanged.

### 7.4 `build.rs`, proc macros, and dependencies

**Native mode cannot contain these.** A build script or proc macro in the pack, or in *any* transitive dependency, runs with the user's full privileges at compile time, before any polint control point exists. `--offline` + `--locked` + the vendored SDK reduce the *supply* of new code (no unpinned fetch), and manifest validation reduces the *reach*, but neither confines execution.

Stated in the docs verbatim: *"Building a repository's rule pack executes that repository's build scripts and procedural macros with your privileges. If you do not trust the repository, run `polint check --untrusted`."*

The only real confinement options are (a) don't build (`--rules-execution none`), or (b) run the whole `polint` invocation inside a container or VM. `POLINT_RULE_SANDBOX=<program>` lets an operator interpose a wrapper (`bwrap`, `firejail`, `sandbox-exec`) around **both** the Cargo build and the rule execution; polint validates that the program exists and prepends it, but makes no claim about the policy inside it.

### 7.5 The rule binary at run time

Controls actually applied when the host executes a rule binary:

| Control | Implementation | Precedent |
| --- | --- | --- |
| clean environment | `Command::env_clear()` + allowlist | `analysis/extensions/host.rs:332-379` |
| working directory | a fresh empty dir under `<cache>/snapshots/<run>/cwd`, **not** the repo root | new |
| stdio only | stdin/stdout framed; no inherited fds | §5.1 |
| wall-clock deadline | 600 s default | `host.rs:17` pattern |
| output caps | 64 MiB stdout frame, 16 KiB stderr | `host.rs:18-19` |
| process-tree kill | Job Objects on Windows (`Win32_System_JobObjects` already a dependency), process-group kill on Unix | `host.rs:508`, `:813-823` |
| no privilege escalation | polint never elevates | — |
| **not** provided | network isolation, filesystem confinement, seccomp | documented as such |

A rule binary can read and write the whole filesystem as the invoking user. That is unchanged from today and is the reason `--untrusted` exists.

### 7.6 Snapshot files

* Written under `<cache>/snapshots/<run-id>/`, where `<run-id>` is a per-invocation random 128-bit value; directory created with `O_EXCL`, mode `0700`; file mode `0600`.
* The rule process verifies `SnapshotHeader.content_digest == RunRequest.snapshot_content_digest` before decoding a single section, and exits 4 otherwise (§5.7). A local attacker who can swap the file cannot make the rule process consume it.
* Deleted at end of run; the `Scratch` role (§6.7) keeps them out of CI cache entries, so a snapshot never travels between machines.
* The snapshot contains repository source text when `include_source_text` is set. It must never be written outside the cache root, and `POLINT_KEEP_SNAPSHOT=1` prints the retained path so it is never silently persisted.

### 7.7 Path traversal

`[rules] paths` is attacker-controlled config read by `discover_local_rule_hosts` (`cli/mod.rs:3872-3882`), which today does `root.join(rule_path).join("Cargo.toml")` with no normalization.

New pre-check, reusing existing helpers:

1. `repo_fs::normalize_repo_relative_input` — rejects absolute paths, `..`, and non-UTF-8.
2. `repo_fs::repo_write_target`-style ancestor walk — rejects any symlinked ancestor and any ancestor that is a regular file (the scaffolder already does exactly this at `cli/mod.rs:871-895`).
3. Canonicalize and assert the result is still under the canonicalized repo root.

Same treatment for `artifact_sources` local-file entries and for the `<pack>` directory walk in `source_tree_digest` (§6.2).

### 7.8 Artifact substitution and cache poisoning

| Attack | Control |
| --- | --- |
| swap a cached binary under `rules-bin` | record stores `binary_sha256` + `binary_len`; mismatch forces a rebuild and emits `polint/cache` warn |
| symlink `rules-bin/<key>/polint-local-rules` at `/bin/sh` | refuse to execute non-regular files or symlinks |
| another local user writes into the cache root | refuse to execute if the artifact's parent directory is group- or world-writable (Unix); on Windows, refuse if the directory's owner is not the current user |
| serve a malicious `.polintrule` from `artifact_sources` | digest must appear in `.polint/rules-artifacts.lock`; otherwise refuse unless `--trust-artifacts` |
| poison another repository's cache | rule binaries are repo-local; the user cache holds only version-addressed SDK sources and digest-pinned artifacts |
| downgrade to an older `sdk_abi` | host declares `[sdk_abi_min, sdk_abi_max]`; the rule declares one number; out-of-range is exit 3, not a fallback |

### 7.9 Default behaviour by scenario

| Scenario | Default mode | Builds? | Executes? | Extra requirement |
| --- | --- | --- | --- | --- |
| Your own repository, local dev | `native` | yes | yes | none — same trust level as `cargo build`, which you already run |
| Your own repository, CI | `artifact-preferred` (set by `action.yml`) | yes, on miss | yes | none |
| A shared rule pack from another team | `artifact-only` | no | yes | digest pinned in `.polint/rules-artifacts.lock` |
| An arbitrary untrusted repository | `none` when `--untrusted`/`POLINT_UNTRUSTED=1`; otherwise `native` with a one-time stderr notice naming the packs about to be compiled | no | no | — |

The one-time notice is deliberate: silently compiling a stranger's code is the status quo, and the notice is the cheapest honest fix short of flipping the default, which would break every existing user.

---

## 8. Phased implementation plan

Each task lists **files**, **depends on**, and **acceptance**. Tasks are sized to be one reviewable PR or smaller.

Throughout Phases C–G a single feature flag governs the new path:

* Cargo feature `rule-protocol` on `polint-engine` (default **off** until Phase H closes).
* Runtime override `POLINT_RULE_BACKEND=legacy|protocol` for A/B in CI and for user escape after release.
* Legacy path = today's `run_local_rule_host_kind` (`cli/mod.rs:4252-4343`), kept intact and compiled in until Phase I.

### Phase A — Measurement and baseline harness

No product behaviour changes. This phase exists so every later claim is checkable, and so the report's unverified numbers (§0) are replaced with measured ones.

**A1, A3, and A4 have landed** (`perf(bench): add the rule-host build-cost baseline harness`). A2, A5, and A6 have not. The table below records what was built, including where it diverged from the original task text, so a later agent does not re-derive a decision that was already made.

| # | Task | Status | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- | --- |
| A1 | `polint-bench build-cost` subcommand: runs a scenario, counts Cargo invocations, records wall-clock, compiled units, `target`/cache bytes+files before/after, and the `CARGO_HOME/registry` byte delta | **done** | `crates/polint-bench/src/build_cost/{mod,scratch,shim}.rs`; `crates/polint-bench/src/main.rs`, `src/lib.rs`; `crates/polint-bench/Cargo.toml` | — | `polint-bench build-cost --repo examples/basic --scenario cold` emits a `polint-build-cost-1` report carrying the whole `METRIC_KEYS` set, with any metric it could not observe `null` and named in `limits` |
| A2 | Widen the matrix: the five scenarios over `examples/go-import-boundaries` and the pinned scale repos, alongside `examples/basic` | **outstanding** | `crates/polint-bench/src/build_cost/scratch.rs`; `scripts/fetch-scale-repos.py`; `research/evaluation-harness/suites/*-scale.toml` | A1 | every requested cell runs; missing scale checkouts skip loudly, matching `tests/golden.rs` behaviour for optional targets. Needs `scanned_sources` to walk subdirectories first — it reads only repository-root files today, so `warm-source-edit` and `test-suite` fail on a repo that nests its sources |
| A3 | Commit measured baseline | **done for one machine** | `research/evaluation-harness/baselines/build-cost.json` (schema `polint-build-cost-1`); `research/evaluation-harness/README.md` | A1 | the file parses as the schema, carries no machine-local path, and states its own limits (both asserted by `the_committed_baseline_matches_the_schema_and_has_no_machine_paths`). The 2 vCPU / 4 GB rig is **not** recorded; absent machines stay absent rather than estimated |
| A4 | `make build-cost` / `make build-cost-baseline` | **done** | `Makefile` (after `scale-corpus-run`) | A3 | `make build-cost` re-runs the matrix and prints the measured/baseline ratio per headline metric; `make build-cost-baseline BUILD_COST_LABEL=<machine> BUILD_COST_RUNS=<n>` rewrites the artifact |
| A5 | Extend the per-check cost record with `cargo_invocations: u32`, `rule_build_ms: u64`, `snapshot_bytes: u64`, `snapshot_encode_ms`/`decode_ms: u64`; bump `SCHEMA_VERSION` to `polint-golden-cost-2` | **outstanding** | `crates/polint/src/golden_cost.rs:20, 23-46`; `crates/polint/tests/golden.rs:29, 41-49`; all sidecars under `tests/golden/outputs/` | — | `POLINT_UPDATE_GOLDEN_COSTS=1 cargo test -p polint --test golden` regenerates; `cargo test -p polint --test golden` green |
| A6 | CI job `build-cost` (non-blocking; uploads the JSON artifact) | **outstanding** | `.github/workflows/ci.yml`, after `gates` | A4 | job runs on PRs touching `crates/**`. Note the cost: a cold cell is a full rule-host build, so this job is minutes, not seconds |

Decisions A1 made that the original task text did not anticipate:

* **Compiled units are counted, not read from `cargo build --timings`.** The Cargo shim installs itself as `RUSTC_WRAPPER` and counts `rustc` invocations that carry `--crate-name` and no `--print`/`-vV` probe. This observes the number instead of parsing a report, and it is the same mechanism a later "0 Cargo invocations" assertion needs. The cost is that `RUSTC_WRAPPER` participates in Cargo's fingerprint, so every cell primes its own state and numbers are not comparable to a run taken without the harness — recorded in the artifact's `limits`.
* **The shim is the `polint-bench` binary itself**, selected through `POLINT_CARGO`, not a generated shell script: one binary, no shell-quoting or Windows `.cmd` problem.
* **`compiler_peak_rss_bytes` is never observed.** Rule-host peak RSS comes from the `POLINT_GOLDEN_COST_PATH` sidecar the engine already writes, and A1 deliberately added no instrumentation to `crates/polint/src`. Cargo and `rustc` memory would need process-level instrumentation the harness does not have, so the metric is `null` in every report. **This is why A3's acceptance is not "every metric non-null."**
* **Fixture cases for `test-suite` are generated**, because no example repository ships `.polint/tests`. A generated case asserts nothing, so its pass/fail tally carries no signal; the case count and the Cargo invocations it causes do.

**Rollback:** delete the bench subcommand and the CI job. No product code touched except additive `golden_cost.rs` fields.

### Phase B — Dependency-closure and feature-leak guard

Built *before* the SDK exists, against a probe, so the guard fails loudly the moment Phase C regresses.

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| B1 | `SDK_ALLOWED_CLOSURE` allowlist + closure test: shells `cargo tree -p polint-sdk --edges normal --prefix none --locked --no-dedupe`, parses names, asserts `set ⊆ allowlist` and `count ≤ 35` | new `crates/polint/tests/sdk_dependency_closure.rs` | C1 for a real target; runs against `tests/fixtures/public-surface-leak-probe` until then | test compiles, `#[ignore]`d until C1, then un-ignored |
| B2 | Forbidden-crate assertion in CI, mirroring the existing parser-isolation check | `.github/workflows/ci.yml:100-118` (`language-features` job) — add an `sdk closure` matrix entry running `cargo tree -p polint-sdk` and `! grep -Eq '(^\| )(oxc_\|tree-sitter\|rusqlite\|rayon\|petgraph\|ignore\|clap\|clap_builder\|tracing-subscriber\|serde_norway\|json-strip-comments)'` | B1 | job fails if any forbidden crate appears |
| B3 | Feature-leak guard: assert `polint-sdk`'s `lang-go`/`lang-typescript` features enable **zero** optional dependencies (they exist only for manifest compatibility, §3.1) | `crates/polint/tests/sdk_dependency_closure.rs` | C1 | `cargo tree -p polint-sdk --features lang-go,lang-typescript` closure equals the default closure exactly |
| B4 | Layering guard for the new crates | `crates/polint/tests/module_layering.rs` (216); `crates/polint/tests/internal_architecture.rs:4-33` | C1 | `crates/polint-sdk/src/**` contains no `polint_engine::`, `crate::analysis`, `crate::cache`, `crate::config`; `REMOVED_PACKAGES` still absent; new positive assertion that the publishable set is exactly `{polint, polint-macros, polint-sdk}` |
| B5 | Compile-time size budget: assert `polint-sdk`'s own `src` LOC ≤ 8,000 | `crates/polint/tests/sdk_dependency_closure.rs` | C-final | fails if engine code drifts back into the SDK |

**Rollback:** the guards are tests; disabling one is a one-line `#[ignore]`.

### Phase C — SDK extraction (mechanical, behaviour-preserving)

No behaviour change. At the end of Phase C the product still runs the legacy path; the only difference is that `polint::sdk` and `polint::runner` are re-exports from a new thin crate. Extraction follows the layering diagram (`ARCHITECTURE.md:56-84`) bottom-up, so each PR compiles.

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| C1 | Create `crates/polint-sdk` (lib only, `publish = true`, `version.workspace`, `rust-version.workspace`, `extern crate self as polint;`) with `core` = moved `internal_core/{ids,span,lang,stable_key}.rs` (~330 LOC) | new `crates/polint-sdk/{Cargo.toml,src/lib.rs,src/core/*}`; `Cargo.toml` members + `[workspace.dependencies] polint-sdk`; `crates/polint/src/internal_core/mod.rs:1-29` becomes `pub use polint_sdk::core::*;` | — | `cargo build --workspace --locked` green; `cargo test -p polint --lib` green; B1 un-ignored and passing |
| C2 | Move `internal_core/diagnostic.rs` (1,079) → `polint_sdk::diagnostics::model` | as above + `crates/polint/src/diagnostics/mod.rs:6-17` | C1 | `cargo test -p polint --test '*'` green |
| C3 | Move fact rows: `analysis_api/{syntax_facts,symbol_facts,module_facts,source_file}.rs` (~1,000) → `polint_sdk::facts`; `analysis_api/mod.rs:16-56` re-exports from the SDK | `crates/polint/src/analysis_api/mod.rs`, the four files | C1 | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` green |
| C4 | Move `core/capability.rs` (289), `core/review.rs` (45), `path_context.rs:6-64`, `cache/mod.rs:872-885` `stable_hash` → `polint_sdk::{core::capability, core::review, core::path_context, hash}`; `cache::stable_hash` becomes a re-export | those files + `crates/polint/src/core/mod.rs:100-105`, `crates/polint/src/cache/mod.rs:872-885` | C1 | separator byte still `0xfe`; new unit test asserts `stable_hash(&["a","b"])` equals the pre-move value |
| C5 | Move `core/rule.rs:18-249` + `rule_error.rs` → `polint_sdk::rule`; keep `Rule::run(&AnalysisDb, &mut RuleCtx)`; `AnalysisDb` is a **temporary alias** to `polint_engine::core::HostFactDb` behind a `sdk-engine-db` feature so this PR compiles unchanged | `crates/polint/src/core/rule.rs`, `crates/polint/src/core/mod.rs:113-114`, `crates/polint/src/rule_error.rs` | C1–C4 | `cargo test -p polint --lib --all-features` green; `tests/consumer_api_compat.rs` compiles untouched |
| C6 | Move `core/rule.rs:277-410` executor to `polint_sdk::rule::exec`, replacing `rayon::par_iter` (`:361-368`) with `std::thread::scope` + index-ordered collection | `crates/polint/src/core/rule.rs` | C5 | diagnostics for `examples/*` byte-identical (`cargo test -p polint --test golden`); `rayon` gone from the SDK closure (B2) |
| C7 | Move `rule_manifest.rs` (452) → `polint_sdk::manifest` | `crates/polint/src/rule_manifest.rs`, `crates/polint/src/runner/mod.rs:11`, `crates/polint/src/cli/mod.rs` | C5 | `inspect_rule_report_sorts_rules_and_uses_stable_top_level_fields` (`rule_manifest.rs:406-451`) passes with the exact expected JSON string |
| C8 | Move `sdk/{mod,facts,policy,scope}.rs` (3,385) → `polint_sdk::sdk`; add `Serialize`/`Deserialize` to the 14 policy types (`sdk/policy.rs:16-620`); `polint::sdk` becomes `pub use polint_sdk::sdk;` | those four files + `crates/polint/src/lib.rs:10` | C5–C7 | `cargo test -p polint --test public_surface_leak` green with `ALLOWED_PRELUDE` **unchanged** (all 116 names) |
| C9 | Move `diagnostics/mod.rs` types (§4.1 list) to `polint_sdk::diagnostics`; renderers stay | `crates/polint/src/diagnostics/mod.rs` | C2 | `cargo test -p polint --test golden`, `--test cli` green; `docs/schemas/polint-report-v1.json` unchanged |
| C10 | Split the heavy crate: `crates/polint` keeps `src/main.rs` plus a facade `lib.rs` re-exporting `polint_sdk::{sdk, runner, rule}` and `polint_engine::run_main`; everything else moves to new `crates/polint-engine` | `Cargo.toml`, new `crates/polint-engine/Cargo.toml`, a `git mv` of `crates/polint/src/*` except `main.rs`/`lib.rs`; `crates/polint/tests/*` move to `crates/polint-engine/tests/` **except** `consumer_api_compat.rs`, `public_surface_leak.rs`, `cargo_install_smoke.rs`, which stay with `polint` | C1–C9 | `make check` green; `cargo install --locked --path crates/polint --force` produces a working `polint` (`tests/cargo_install_smoke.rs`) |
| C11 | Rename `polint_engine::core::AnalysisDb` → `HostFactDb`; drop the C5 alias; `polint::sdk::__private::AnalysisDb` (`sdk/mod.rs:66`) now names an SDK type with the same accessor surface, backed by `FactSnapshot` | `crates/polint-engine/src/core/db.rs:142`, `crates/polint-engine/src/core/mod.rs:106`, `crates/polint-sdk/src/sdk/db.rs` (new) | C10, D1–D4 | `polint-macros` unchanged (verified by `cargo test -p polint-macros`) |

**Rollback:** each task is a `git mv` plus a re-export; reverting a PR restores the prior module. The `sdk-engine-db` alias in C5 is the seam that lets C1–C10 land before Phase D exists.

### Phase D — `FactSnapshot` and serialization

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| D1 | Container codec: `PSNAP1` header/section reader+writer, `SnapshotHeader`, `SnapshotSection`, `SectionCodec` (§4.4) | new `crates/polint-sdk/src/snapshot/{mod,container}.rs` | C1 | round-trip unit tests; truncated/garbage input yields typed errors, never a panic |
| D2 | Wire row types: `SourceFileWire`, `PathContextIndexWire`, `RuleOptionsWire`, section payload structs | `crates/polint-sdk/src/snapshot/wire.rs` | C3, C4 | `RuleOptionsWire` covers all 8 `RuleOptions` fields (`core/rule.rs:131-147`); compile-time exhaustiveness test via destructuring |
| D3 | `HostFactDb::snapshot(&SnapshotRequest) -> FactSnapshot`: per-family projection, file-scope filtering, `stable_keys` export, `content_digest` | `crates/polint-engine/src/core/db.rs` (new `impl` block); new `crates/polint-engine/src/snapshot_build.rs` | D1, D2 | requesting only `{syntax, imports}` produces exactly 3 sections (`source_files`, `syntax`, `imports`) |
| D4 | SDK loader: `AnalysisDb::from_snapshot_file(path, expected_digest)`; rebuild `StableKeyInterner`, the `definition_for_symbol` index, `Arc<str>` sources; implement every accessor used by `sdk/facts.rs` | `crates/polint-sdk/src/sdk/db.rs` | D1–D3, C11 | all 27 fact views build and iterate; `cargo test -p polint-sdk` green |
| D5 | Reimplement the 8 symbol-graph queries against `&AnalysisDb` | `crates/polint-sdk/src/sdk/symbol_query.rs` (from `crates/polint-engine/src/analysis_neutral/symbol_graph/query.rs:6-102`) | D4 | differential test T-EQ-3 (§9) proves engine and SDK produce identical ordering, including the `reference_order` stable-key tiebreak (`query.rs:77-97`) |
| D6 | Property test: `HostFactDb -> FactSnapshot -> AnalysisDb` preserves every family's row order and content | new `crates/polint-engine/tests/snapshot_roundtrip.rs` (uses `proptest`, already a dev-dep) | D3, D4 | 1,000 cases green |
| D7 | Version/corruption errors (§5.7 codes 4 and 5) with actionable messages | `crates/polint-sdk/src/snapshot/error.rs`; `crates/polint-engine/src/cli/rules_host_error.rs` | D1 | flipping one byte in a section is detected by `content_digest`; the message names the file and the remedy |
| D8 | Snapshot size/latency instrumentation into the A5 cost record | `crates/polint-engine/src/golden_cost.rs` | A5, D3 | `snapshot_bytes`, `snapshot_encode_ms`, `snapshot_decode_ms` populated |

**Rollback:** the snapshot path is unreachable until Phase E wires it; reverting D leaves an unused module.

### Phase E — Host/rule protocol

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| E1 | Protocol types + schema constants (`polint-rule-host-hello-v1`, `-run-v1`, `-policy-v1`), all `#[serde(rename_all="snake_case", deny_unknown_fields)]` | new `crates/polint-sdk/src/protocol/{mod,wire}.rs`, modelled on `crates/polint-engine/src/analysis_neutral/extensions/protocol.rs:1-128` | C-final, D2 | unknown-field rejection tests mirroring `extensions/protocol.rs:134-162` |
| E2 | Length-prefixed frame codec (`u32` LE + JSON), reader/writer, 64 MiB cap | `crates/polint-sdk/src/protocol/frame.rs` | E1 | unit tests: zero-length, oversized, truncated, split-across-reads |
| E3 | Rewrite `runner::run_cli` as the protocol client: parse `--polint-rule-protocol <n>`, Hello, loop on RunRequest, execute via `polint_sdk::rule::exec`, respond; **no clap, no tracing-subscriber** | `crates/polint-sdk/src/runner/mod.rs` (from `crates/polint-engine/src/runner/mod.rs`, 524) | E1, E2, D4 | `polint::runner::run_cli(vec![…])` signature unchanged (`ExitCode` return); B2 shows `clap` and `tracing-subscriber` gone |
| E4 | Engine-side `RuleHost` client: spawn, Hello, plan, snapshot, RunRequest, collect | new `crates/polint-engine/src/rule_host/{mod,client}.rs` | E1–E3, D3 | drives `examples/basic` end-to-end behind `POLINT_RULE_BACKEND=protocol` |
| E5 | Policy RPC: `PolicyChannel` in the SDK (`Mutex<Framed>`), `PolicyServer` in the engine dispatching to `policy_queries::{matching_events, forbidden_reachable, missing_guards, missing_cleanup, forbidden_flows}` (`policy_queries.rs:23-49`) | `crates/polint-sdk/src/sdk/facts.rs:883-962`, `crates/polint-sdk/src/sdk/policy_channel.rs`, `crates/polint-engine/src/rule_host/policy.rs` | E4, C8 | a rule generated by `polint new-rule --template unsafe-deserialization` produces byte-identical diagnostics under both backends |
| E6 | Process controls: `env_clear()` + allowlist, empty cwd, deadlines, output caps, process-group/Job-Object kill, stdin-EOF cancellation | `crates/polint-engine/src/rule_host/process.rs`, reusing `crates/polint-engine/src/analysis/extensions/host.rs:332-379` and the Windows Job Object code | E4 | a rule that spawns a child and sleeps is fully reaped on timeout (test mirrors `host.rs:790-830`) |
| E7 | Error taxonomy: reuse `rules_host_error_message` (`cli/rules_host_error.rs:8-37`) for spawn/build failures; add protocol codes 3/4/5 | `crates/polint-engine/src/cli/rules_host_error.rs`, `crates/polint-engine/src/rule_host/error.rs` | E4 | MSRV/network/manifest/rustc hint tests (`rules_host_error.rs:101-112`) still pass |
| E8 | `sdk_abi` constant + negotiation | `crates/polint-sdk/src/lib.rs`, `crates/polint-engine/src/rule_host/client.rs` | E1 | a binary built against `sdk_abi = N-1` is rejected with exit 3 and a message naming both numbers |

**Rollback:** feature `rule-protocol` off, or `POLINT_RULE_BACKEND=legacy`.

### Phase F — Fingerprint, artifact cache, direct execution

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| F1 | `RuleArtifactKeyInputs` + `source_tree_digest` (§6.1, §6.2) with traversal hardening (§7.7) | new `crates/polint-engine/src/rules_artifact/{mod,key}.rs`; `crates/polint-engine/src/repo_fs.rs` (reuse) | — | identical trees on two machines with the same toolchain yield the same key; a symlink escaping the pack is rejected |
| F2 | `RuleArtifactRecord` store; `CacheManagedCategory::{RulesBin, Snapshots}` + `CacheCategoryArg` values | `crates/polint-engine/src/cache/mod.rs:651-696, 581-591`; `crates/polint-engine/src/cli/mod.rs` (`CacheCategoryArg`, `category_arg_to_managed`) | F1 | `cache_category_arg_covers_every_managed_category` (`cli/mod.rs:4497-4530`) passes; `polint cache status --format json` lists both new rows and validates against `docs/schemas/polint-cache-status-v1.json` (schema gains two enum values) |
| F3 | Build+capture: `cargo build --locked --quiet --message-format json-render-diagnostics --config <generated>`; parse `compiler-artifact`; hard-link into `rules-bin`; prune the pack's own output from `rules-target` | `crates/polint-engine/src/rules_artifact/build.rs` | F1, F2 | one Cargo invocation produces a runnable artifact; `rules-target` no longer contains the pack's `.rlib`/binary after capture |
| F4 | `resolve_rule_binary` + direct exec (§6.3), including the ownership/permission refusals (§7.8) | `crates/polint-engine/src/rules_artifact/resolve.rs`, `crates/polint-engine/src/rule_host/client.rs` | F3, E4 | second `polint check` in an unchanged repo performs **0** Cargo invocations (asserted via the A1 `POLINT_CARGO` shim) |
| F5 | LRU ceilings for `rules-bin`, `snapshots`, user artifact cache; touch-on-hit | `crates/polint-engine/src/cache/mod.rs:494-579` (reuse `prune`) | F2 | exceeding `POLINT_RULES_BIN_MAX_MB` evicts oldest-used first |
| F6 | Vendored SDK: embed sources, materialize to the user cache, generate the `--config` patch file; `--offline`/`--locked`/`--frozen` selection | new `crates/polint/assets/`, `crates/polint-engine/src/rules_artifact/vendor.rs`, `crates/polint-engine/src/rules_artifact/build.rs` | F3 | a build succeeds with `CARGO_NET_OFFLINE=true` and an empty `CARGO_HOME/registry` for a pack whose only dependency is `polint` |
| F7 | Manifest validation (§7.3) | `crates/polint-engine/src/rules_artifact/manifest_check.rs` | F1 | all 17 `examples/*/.polint/rules/Cargo.toml` and the `pack_cargo_toml` output pass; each forbidden construct has a negative test |
| F8 | `polint rules build [--emit-artifact DIR]` subcommand | `crates/polint-engine/src/cli/mod.rs` (new `Command::Rules`) | F3 | produces `<key>.polintrule`; `--help` documented |

**Rollback:** `POLINT_RULE_BACKEND=legacy` bypasses F entirely; the caches are additive directories that `polint cache clean` removes.

### Phase G — Runner and CLI integration

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| G1 | Rewrite `check_local_rule_hosts` (`cli/mod.rs:3949-4046`): validate packs → resolve binaries → build **one** plan from all packs' handshakes → build **one** snapshot → drive each pack → assemble | `crates/polint-engine/src/cli/mod.rs:3949-4046, 4236-4343` | E4, F4 | `polint check` on all 17 examples produces byte-identical `--format json` under both backends |
| G2 | Drop the duplicate source load for ignores/`--stat` (`cli/mod.rs:3973-3985`) — the host already has `HostFactDb` | `crates/polint-engine/src/cli/mod.rs:3949-3985`; `backfill_diagnostic_files` (`:3926-3947`) retained for out-of-scope diagnostics | G1 | `--stat`/`--shortstat` output unchanged; measured file reads halve on `examples/go-import-boundaries` |
| G3 | `inspect rule` from the handshake (or from `RuleArtifactRecord.rule_ids` with no spawn); delete `run_local_rule_host_inspect` (`cli/mod.rs:4350-4407`) | `crates/polint-engine/src/cli/mod.rs` | E4 | `polint inspect rule --format json` byte-identical; **0** Cargo invocations when the artifact is current |
| G4 | `review` (`cli/mod.rs:4054+`): changeset travels in `RunRequest.changeset`; retire `--changed-files` and `write_review_changeset` (`:4181-4199`) | `crates/polint-engine/src/cli/mod.rs`, `crates/polint-sdk/src/runner/mod.rs` | E4 | `examples/review-rules` and `examples/gorm-review-indexes` produce identical output; the `review/` cache dir keeps working for the legacy backend until Phase I |
| G5 | `polint test`: one binary resolve and one process per pack for the whole suite (§6.4) | `crates/polint-engine/src/rule_test.rs:184-195, 244-303, 323-373` | E4, F4 | Cargo invocations for `polint test` on `examples/multiple-rules` drop from N to ≤ 1; report byte-identical |
| G6 | Baseline path (`collect_diagnostics_for_baseline`, `cli/mod.rs:3751-3792`) uses the same resolve+drive helper | `crates/polint-engine/src/cli/mod.rs:3751-3792` | G1 | `--baseline`/`--new-only` behaviour unchanged |
| G7 | Extension host reuses `rules_artifact` for build+cache (its `command_spec`, `analysis/extensions/host.rs:128-162`, is a `cargo run` with the same problem) | `crates/polint-engine/src/analysis/extensions/host.rs` | F4 | extension handshake works with 0 Cargo invocations when unchanged; `extensions-target` pruned like `rules-target` |

**Rollback:** every G task keeps the legacy function until Phase I; a revert restores the old call site.

### Phase H — Test harness and golden equivalence

| # | Task | Files | Depends | Acceptance |
| --- | --- | --- | --- | --- |
| H1 | Dual-backend golden runner: every case in `tests/golden-corpus/inputs.toml` runs under both backends and the outputs must be **identical** | `crates/polint-engine/tests/golden.rs:60-70` (`polint_cmd` gains a backend param) | G1 | `cargo test -p polint-engine --test golden` green for both |
| H2 | Capability-matrix dual run | `crates/polint-engine/tests/capability_matrix.rs` (913), `tests/capability-matrix/matrix.toml` | G1 | all matrix cells identical across backends |
| H3 | `cli.rs` integration updates: new assertions for 0-Cargo warm runs, `--rules-execution`, artifact corruption, `sdk_abi` mismatch | `crates/polint-engine/tests/cli.rs` (12,464) | F4, E8 | new tests green; no existing test deleted, only re-pointed |
| H4 | Action-contract test covers `rules-bin` + `snapshots` | `crates/polint-engine/tests/github_action_cache.rs` (1,119) | F2, I2 | `cache_layout_matches_the_github_action_contract` green |
| H5 | Flip the default: `rule-protocol` on by default; `POLINT_RULE_BACKEND=legacy` still available | `crates/polint-engine/Cargo.toml`, `crates/polint/Cargo.toml` | H1–H4 | full `make check` green |

### Phase I — Docs, action, release

| # | Task | Files | Depends |
| --- | --- | --- | --- |
| I1 | §12 documentation updates | `README.md`, `docs/CONSUMER-SETUP.md`, `docs/GITHUB-ACTION.md`, `ARCHITECTURE.md`, `AGENTS.md`, `docs/AGENT-PLAYBOOK.md`, `docs/RELEASING.md`, `docs/API-VISIBILITY-PLAN.md`, `examples/*/README.md` | H5 |
| I2 | Action: cache `rules-bin`, add a `rules-execution` input, default `artifact-preferred`, report `rules-bin` size | `action.yml:18-63, 210-263`, `scripts/action/resolve-cache-inputs.sh`, `scripts/action/prepare-build-cache-save.sh` | F2, H4 |
| I3 | Generated skill text | `crates/polint-engine/src/cli/skill.rs:185, 209-246, 287-288, 330-344, 418-433` | I1 |
| I4 | Publish `polint-sdk` | `scripts/publish-crates.sh:11-15` (`PACKAGES=(polint-macros polint-sdk polint)`), `scripts/bump-workspace-version.py`, `.github/workflows/release.yml`, `release-dry-run.yml` | C10 |
| I5 | Schema additions | `docs/schemas/polint-cache-status-v1.json` (two enum values); new `docs/schemas/polint-fact-snapshot-v1.json`, `polint-rule-host-protocol-v1.json`, `polint-rule-artifact-v1.json` | F2, D1, E1 |
| I6 | Delete the legacy path: `run_local_rule_host_kind`, `run_local_rule_host_inspect`, `write_review_changeset`, `--changed-files`, `CacheManagedCategory::Review` | `crates/polint-engine/src/cli/mod.rs`, `cache/mod.rs` | one minor release after H5 |

### Phase J — Optional prebuilt artifact path

J1 `.polint.toml` `[rules] execution` + `artifact_sources` parsing (`crates/polint-engine/src/config/`). J2 `.polint/rules-artifacts.lock` reader/writer + `polint rules lock`. J3 artifact fetch with sha256 verification (`sha2`, engine-only). J4 `--trust-artifacts`. J5 action input `rules-artifact-sources`.

**Acceptance:** a runner with **no Rust toolchain at all** completes `polint check` against a pinned artifact.

### Phase K — WASM backend decision gate

Not built in this plan. Gate criteria, all of which must hold before any WASM work is scheduled:

1. Phase A/H measurements show native artifact resolution is already at target (§10) — i.e. WASM is being considered for *sandboxing*, not speed.
2. A concrete customer requirement exists for scanning untrusted third-party repositories with rules executing (today's honest answer is `--untrusted`).
3. A prototype shows the SDK compiles to `wasm32-wasip2` with the snapshot loader intact and the policy RPC expressed as host functions.
4. Measured WASM rule execution is within 3× native on the medium corpus.

If the gate opens, the design is: the same rule source, a second target triple in the artifact key, `.polintrule` carrying a `.wasm` module, and the protocol expressed as WASI host imports instead of stdio frames. **The authoring path never changes.**

---

## 9. Testing and verification matrix

No entry below asserts that a test currently passes — these are the tests to write and the assertions they must make.

### 9.1 Unit

| ID | Subject | Command | Assertion |
| --- | --- | --- | --- |
| U-1 | Snapshot container | `cargo test -p polint-sdk snapshot::container` | round-trip; truncated header, bad magic, bad `container_version`, section offset past EOF → typed error, no panic |
| U-2 | `stable_hash` preservation | `cargo test -p polint-sdk hash::` | `stable_hash(&["a","b"]) == "<pre-move value>"`; separator byte is `0xfe`, distinct from `analysis_neutral::hash` (`0xff`) |
| U-3 | `RuleOptionsWire` exhaustiveness | `cargo test -p polint-sdk protocol::wire` | destructuring `RuleOptions { severity, files, allow_files, allow, max, deny, forbidden_imports, settings }` fails to compile if a field is added and not mapped |
| U-4 | Frame codec | `cargo test -p polint-sdk protocol::frame` | 0-length, 64 MiB+1 rejected, split reads reassembled |
| U-5 | Artifact key | `cargo test -p polint-engine rules_artifact::key` | changing any one of the 12 inputs changes the key; changing none does not |
| U-6 | `source_tree_digest` traversal | `cargo test -p polint-engine rules_artifact::key::traversal` | escaping symlink, absolute path, `..` all rejected; executable-bit change alters the digest |
| U-7 | Manifest validation | `cargo test -p polint-engine rules_artifact::manifest_check` | each §7.3 construct rejected with the offending key named; `pack_cargo_toml` output accepted |
| U-8 | Cache category coverage | `cargo test -p polint-engine cli::tests::cache_category_arg_covers_every_managed_category` | existing test (`cli/mod.rs:4497-4530`) passes with `rules-bin` and `snapshots` added |

### 9.2 Integration

| ID | Command | Assertion |
| --- | --- | --- |
| I-1 | `cargo test -p polint-engine --test cli --all-features --locked` | full existing suite green |
| I-2 | `cargo test -p polint-engine --test cli warm_check_performs_no_cargo_invocations` | with a `POLINT_CARGO` shim logging invocations, the second `polint check` logs **0** lines |
| I-3 | `cargo test -p polint-engine --test cli rule_edit_triggers_exactly_one_cargo_invocation` | touching a rule `.rs` → exactly 1 |
| I-4 | `cargo test -p polint-engine --test cli source_edit_triggers_no_cargo_invocation` | touching a `.go`/`.ts` file → 0 |
| I-5 | `cargo test -p polint-engine --test cli inspect_rule_uses_artifact_record` | `polint inspect rule --format json` → 0 Cargo invocations, 0 rule-process spawns |
| I-6 | `cargo run -p polint -- test` in `examples/multiple-rules` | Cargo invocations ≤ 1; rule-process spawns ≤ 1 |
| I-7 | `cargo test -p polint-engine --test cli extension_host_uses_artifact_cache` | extension handshake with 0 Cargo invocations when unchanged |

### 9.3 Golden equivalence

| ID | Command | Assertion |
| --- | --- | --- |
| G-1 | `cargo test -p polint-engine --test golden` | every case in `tests/golden-corpus/inputs.toml` matches `tests/golden/outputs/**` |
| G-2 | `POLINT_RULE_BACKEND=legacy cargo test -p polint-engine --test golden` | same goldens |
| G-3 | `cargo test -p polint-engine --test golden_corpus` | corpus-level invariants hold |
| G-4 | `cargo test -p polint-engine --test capability_matrix` under both backends | identical cells |
| T-EQ-3 | `cargo test -p polint-engine --test symbol_query_equivalence` | for 200 generated fact sets, `polint_engine::analysis_neutral::symbol_graph::query::*` and `polint_sdk::sdk::symbol_query::*` return identical sequences, **including** the `reference_order` stable-key tiebreak (`query.rs:77-97`) |
| T-EQ-5 | `cargo test -p polint-engine --test policy_violation_roundtrip` | proptest: `PolicyViolation` → JSON → `PolicyViolation` preserves `stable_key()` and `diagnostic(id, msg)` byte-for-byte |

### 9.4 Public API leak

| ID | Command | Assertion |
| --- | --- | --- |
| A-1 | `cargo test -p polint --test public_surface_leak` | `ALLOWED_PRELUDE` (116 names, `tests/public_surface_leak.rs:41+`) unchanged; the probe compiles with `#![no_implicit_prelude]` and a single `use ::polint::sdk::prelude::*;` |
| A-2 | `cargo test -p polint --test consumer_api_compat` | `Span`/`DiagnosticRange` struct literals, `RuleId(..)`, `Language`/`Severity` comparisons, `Diagnostic::error(..).with_evidence(..)` all still compile |
| A-3 | `cargo test -p polint-engine --test internal_architecture` | `REMOVED_PACKAGES` still absent; publishable set is exactly `{polint, polint-macros, polint-sdk}` |
| A-4 | `cargo test -p polint-engine --test module_layering` | `crates/polint-sdk/src/**` free of `polint_engine::`, `crate::analysis`, `crate::cache`, `crate::config` |
| A-5 | `cargo test -p polint --test sdk_dependency_closure` | closure ⊆ allowlist, count ≤ 35; SDK `src` LOC ≤ 8,000 |
| A-6 | `cargo tree -p polint-sdk --features lang-go,lang-typescript --locked` | output identical to the default-feature closure (B3) |

### 9.5 Capability and determinism

| ID | Command | Assertion |
| --- | --- | --- |
| C-1 | `cargo test -p polint-engine --test capability_matrix` | a rule requesting a family absent from the snapshot request is planned as `SetupMissing`/`Unsupported`, never silently empty |
| C-2 | `cargo test -p polint-engine --test snapshot_roundtrip` | row order preserved per family (D6) |
| C-3 | `for i in 1 2 3 4 5; do cargo run -p polint -- check --format json examples/basic; done` | all five stdouts byte-identical |
| C-4 | `cargo test -p polint-engine determinism::parallel_rule_execution` | `parallel: true` and `parallel: false` produce identical diagnostics for all examples |
| C-5 | `cargo test -p polint-engine --test cli policy_query_order_is_stable` | a rule issuing 3 policy queries from concurrently-executing rules gets identical results across 20 runs |

### 9.6 Corruption, version mismatch, failure

| ID | Scenario | Expected |
| --- | --- | --- |
| F-1 | flip one byte in a snapshot section | rule exits 4; host message names the file and `polint cache clean --category snapshots` |
| F-2 | rule binary built against `sdk_abi = N-1` | exit 3; message names both ABI numbers and the remedy |
| F-3 | truncate the artifact binary | sha256 mismatch → automatic rebuild + `polint/cache` warn |
| F-4 | replace the artifact binary with a symlink | refuse to execute; hard error |
| F-5 | `chmod 0777` the artifact parent dir (Unix) | refuse to execute; hard error |
| F-6 | rule process sleeps past the deadline | killed with its children; `Timeout` failure kind; bounded stderr surfaced |
| F-7 | rule writes 100 MiB to stdout | frame cap hit; `MalformedResponse`; process killed |
| F-8 | rule panics | `internal/<rule_id>` error diagnostic, exit 0, other rules still reported (`core/rule.rs:354-358`) |
| F-9 | rule pack `Cargo.toml` with `path = "../../../etc"` | `polint/rules-manifest` error, pack skipped, other packs run |
| F-10 | `[rules] paths = ["../outside"]` | rejected by traversal normalization; error names the config key |
| F-11 | downloaded artifact absent from `.polint/rules-artifacts.lock` | refused; message shows the digest to pin |

### 9.7 Offline / no-Cargo / cross-platform

| ID | Command | Assertion |
| --- | --- | --- |
| O-1 | `PATH` without `cargo`, artifact current, `polint check` | succeeds |
| O-2 | `PATH` without `cargo`, artifact stale | fails with a message naming `cargo` and `--rules-execution artifact-only` |
| O-3 | `CARGO_NET_OFFLINE=true` + empty `CARGO_HOME/registry` + vendored SDK | build succeeds (F6) |
| O-4 | `POLINT_RULES_OFFLINE=strict` with a lockfile-changing edit | fails with the `--frozen` message |
| X-1 | `.github/workflows/ci.yml` `test-platform` matrix (`windows-latest`, `macos-latest`) | `cargo test -p polint-sdk --lib` and `-p polint-engine --lib` green |
| X-2 | `gates` matrix (`ubuntu-latest`, `macos-latest`) | leak gate green on both, as `tests/public_surface_leak.rs:12-14` requires |
| X-3 | Windows | Job-Object containment test; `.exe` suffix handling in `RuleArtifactRecord.binary_relative`; `\`→`/` normalization in `source_tree_digest` |
| X-4 | `cargo test -p polint --test cargo_install_smoke --locked -- --ignored` | `cargo install --locked --path crates/polint` still yields a working `polint --version` |
| X-5 | `cargo check --workspace --all-targets --all-features --locked` on toolchain `1.95` | MSRV job green for all three published crates |

### 9.8 Performance regression gates

| ID | Command | Assertion |
| --- | --- | --- |
| P-1 | `cargo test -p polint-engine --test golden` | per-case `wall_clock_ms` and `peak_rss_delta_bytes` within `MAX_COST_RATIO = 1.50` of the committed sidecar, or under the 100 ms / 16 MiB floors (`tests/golden.rs:28-31`) |
| P-2 | `make build-cost` | every metric within the §10.4 budget; kill criteria (§10.5) not tripped |
| P-3 | `cargo test -p polint-engine --test golden snapshot_cost` | `snapshot_encode_ms + snapshot_decode_ms` ≤ 15% of `wall_clock_ms` on the medium corpus, else open decision gate D-1 |

---

## 10. Performance budgets and experiment design

### 10.1 Measured vs. asserted

**Measured today, in-repo:** per-golden-case `wall_clock_ms`, `peak_rss_bytes`, `peak_rss_delta_bytes` (`golden_cost.rs:23-46`, sidecars under `tests/golden/outputs/`), gated at 1.50× (`tests/golden.rs:28-31`); scale-corpus LOC/RSS/wall-clock (`scripts/run-scale-corpus.py` → `research/evaluation-harness/baselines/scale-corpus-run.json`).

**Measured by Phase A:** `research/evaluation-harness/baselines/build-cost.json` — per cell, Cargo starts, `rustc` starts, compiled units, end-to-end and in-Cargo wall-clock, rule-host wall-clock and peak RSS, bytes before/after/written and files retained for the rule-host target directory and the polint cache, and the `CARGO_HOME/registry` delta. §0 lists the headline values. Regenerate with `make build-cost`.

**Superseded:** the reported 223 compiled units, 185.4 s cold build, and 537 MB retention. §0 records what each became and why one of the three is not the same quantity at all.

**Still asserted, not measured:** every budget in §10.4 and every kill criterion in §10.5. Each is a ratio against the Phase A cell for the same repository, scenario, and machine — never against the superseded figures, and never against a cell measured on different hardware. Two inputs the baseline does not yet have: the M-CI rig (§10.2) and any repository other than `examples/basic` (Phase A2).

### 10.2 Machines and corpora

| Machine | Spec | Purpose |
| --- | --- | --- |
| **M-CI** | 2 vCPU, 4 GB RAM, cold page cache, empty `CARGO_HOME` | worst case; matches a constrained runner. `ubuntu-latest` is 4 vCPU/16 GB, so M-CI must be a container with limits; the `test-linux` job already frees disk (`ci.yml:155-158`) |
| **M-DEV** | ≥ 8 cores, ≥ 16 GB, warm page cache, populated `CARGO_HOME`, `.cargo/config.toml` `incremental = false` | normal developer machine |

| Corpus | Source | Size |
| --- | --- | --- |
| **small** | `examples/basic` (1 rule, 2 views) | tens of files |
| **medium** | `examples/go-import-boundaries` + `examples/ts-design-tokens` | hundreds of files |
| **large** | pinned scale repos via `make fetch-scale-repos` (`research/evaluation-harness/suites/*-scale.toml`) | 100k+ LOC |

### 10.3 Scenarios

| Scenario | Definition |
| --- | --- |
| **cold** | empty `.polint/cache`, empty `CARGO_HOME/registry`, no artifact |
| **warm-noop** | immediately re-run, nothing changed |
| **warm-rule-edit** | one byte changed in a `.polint/rules/src/*.rs` |
| **warm-source-edit** | one byte changed in a scanned `.go`/`.ts` file |
| **repeat×10** | ten consecutive `warm-noop` runs |
| **test-suite** | `polint test` over all fixture cases |

`polint-bench build-cost` implements all of these except **repeat×10**, which is
`--scenario warm-noop --runs 10` read as a series rather than a median.

### 10.4 Budgets

Baseline = the Phase A measurement of the same cell on the same machine.

| Metric | cold | warm-noop | warm-rule-edit | warm-source-edit | test-suite |
| --- | --- | --- | --- | --- | --- |
| **Cargo invocations** | 1 | **0** | 1 | **0** | ≤ 1 per pack |
| **Compiled units** | ≤ 35 + pack | 0 | ≤ 1 + pack | 0 | ≤ 35 + pack |
| **Rule compile wall-clock (M-CI)** | ≤ 0.20 × baseline cold | 0 | ≤ 10 s | 0 | ≤ 0.20 × baseline |
| **End-to-end wall-clock (M-CI, medium)** | ≤ 0.35 × baseline | ≤ 0.30 × baseline | ≤ 0.45 × baseline | ≤ 0.30 × baseline | ≤ 0.25 × baseline |
| **End-to-end wall-clock (M-DEV, medium)** | ≤ 0.40 × baseline | ≤ 0.35 × baseline | ≤ 0.50 × baseline | ≤ 0.35 × baseline | ≤ 0.20 × baseline |
| **Bytes downloaded** | ≤ 0.20 × baseline (the vendored SDK removes the polint half entirely) | 0 | 0 | 0 | ≤ 0.20 × baseline |
| **Bytes written** | ≤ 0.30 × baseline | ≤ snapshot size | ≤ 0.10 × baseline | ≤ snapshot size | ≤ 0.30 × baseline |
| **Bytes retained (`rules-target` + `rules-bin`)** | ≤ 0.35 × baseline | unchanged | unchanged ± artifact | unchanged | ≤ 0.35 × baseline |
| **Peak RSS, rule process** | ≤ 1.2 × snapshot size + 64 MiB | same | same | same | same |
| **Peak RSS, host process** | ≤ 1.15 × baseline `peak_rss_bytes` | same | same | same | same |

Component budgets on the medium corpus, M-DEV:

| Component | Budget | Rationale |
| --- | --- | --- |
| host analysis (parse → facts) | ≤ 1.05 × baseline in-child analysis | it is the same code; the 5% allows the snapshot projection |
| snapshot encode | ≤ 60 ms | ~5–15 MB of JSON |
| snapshot decode | ≤ 80 ms | plus `Arc<str>` materialization |
| encode + decode combined | **≤ 15% of warm end-to-end** | trips KC-4 |
| rule process startup → Hello | ≤ 30 ms | thin binary, no clap, no tracing-subscriber init |
| artifact resolution (hash + verify) | ≤ 40 ms for a 1,000-file pack | `source_tree_digest` dominates; FNV over file contents |
| policy RPC round-trip overhead | ≤ 2 ms per query excluding host compute | framing only |

### 10.5 Kill criteria

Any of these, sustained across three runs on both machines, stops the phase and forces a documented decision:

* **KC-1** — warm-noop Cargo invocations > 0 after Phase F. The entire premise fails; investigate before proceeding to G.
* **KC-2** — SDK closure > 45 units. The thin SDK is not thin; re-examine `toml`/`globset`/`anyhow` in the public contract.
* **KC-3** — end-to-end warm-noop on M-CI worse than 0.60 × baseline. The process boundary plus snapshot cost more than the compile saved; consider an in-host fast path for packs whose rules request only `syntax`-family views.
* **KC-4** — snapshot encode+decode > 15% of warm end-to-end. Opens the binary-codec gate (adopt `postcard` behind `SectionCodec`, §4.4).
* **KC-5** — host peak RSS > 1.30 × baseline. The projection is copying where it should be moving; make `HostFactDb::snapshot` consume `self` on the final call.
* **KC-6** — any golden output differs between backends. Blocks H5 unconditionally.
* **KC-7** — retention reduction < 2×. The `rules-target` prune (§6.7) is not working; verify `cargo clean -p <pkgid>` equivalence.

### 10.6 Experiment protocol

Each cell: 5 runs, reported as a median. Warm cells take one unmeasured warm-up first; cold cells must not — a warm-up would destroy the state the cell is defined by, so `build-cost` re-primes them (deletes the cache and the rule-host target directory, re-materializes the repository) before each run instead. `--runs N` selects the run count; the committed baseline states its own in `limits` when it is 1. Fix `POLINT_RULES_PROFILE` per cell (`release` is the product default, `cli/mod.rs:4429-4439`; `dev` is what `tests/golden.rs:69` uses). Pin the toolchain to `rust-toolchain.toml`. Record `rustc -vV`, `cargo -V`, kernel, filesystem, and whether `sccache` is active (`.cargo/config.toml` makes it opt-in) in every result row. Cold cells run in a fresh container with an empty `CARGO_HOME`.

---

## 11. Migration and release plan

### 11.1 Principle

No user is required to migrate on any release. A rule pack pinned to `polint = "0.2"` keeps working unchanged; it simply keeps paying the old build cost. Migration is opt-in, one pack at a time, and reversible.

### 11.2 Version map

| Release | Ships | User-visible |
| --- | --- | --- |
| **0.3.0** | `polint-sdk` published; `polint` re-exports it; `polint-engine` internal; protocol behind `POLINT_RULE_BACKEND=protocol` (default `legacy`) | none by default. `polint rules migrate` available. |
| **0.4.0** | protocol default **on**; artifact cache on; `polint check --rules-execution`; action defaults to `artifact-preferred` | new packs generated with `package = "polint-sdk"`. Legacy packs auto-detected and run on the legacy path with a one-time info diagnostic. |
| **0.5.0** | legacy path removed (Phase I6); `--changed-files`, `run_local_rule_host_inspect`, `CacheManagedCategory::Review` deleted | packs still on `polint = "0.2"`/`"0.3"` fail with an actionable error naming `polint rules migrate` |

Minimum deprecation window: **0.4.0 → 0.5.0 ≥ 90 days**, and 0.5.0 must not ship until evidence (issue tracker plus the examples corpus) shows migration is mechanical.

### 11.3 Compatibility modes

Detection is on the pack manifest, not on a flag:

| Pack `[dependencies]` | Mode | Behaviour |
| --- | --- | --- |
| `polint = { package = "polint-sdk", … }` | **protocol** | new path |
| `polint = { version = "0.4", … }` (facade) | **protocol-via-facade** | works, but the pack compiles the engine too — correct output, no speedup; one-time `polint/rules-migrate` info diagnostic |
| `polint = { version = "0.2"/"0.3", … }` | **legacy** | old `cargo run` path (0.3/0.4 only) |
| `polint = { path = "…/crates/polint" }` (this repo's own examples) | resolved by path | `polint_deps_path_prefix` (`cli/mod.rs:1105-1117`) is extended to prefer `crates/polint-sdk` when present |

Mode is reported in `polint inspect rule --format json` as a new optional `execution_mode` field on `RuleManifestWire` (`rule_manifest.rs:170-182`), `#[serde(skip_serializing_if = "Option::is_none")]`, so `docs/schemas/polint-rule-inspect-v1.json` stays backward-compatible.

### 11.4 Generated manifests

`pack_cargo_toml` (`cli/mod.rs:1130-1158`) changes exactly one line:

```rust
// before
format!(r#"polint = {{ version = "{version}", default-features = false, features = [{features}] }}"#)
// after
format!(r#"polint = {{ package = "polint-sdk", version = "{version}", default-features = false, features = [{features}] }}"#)
```

`enabled_language_features()` (`:1119-1128`) is unchanged, and `polint-sdk` accepts `lang-go`/`lang-typescript` as no-op features (§3.1) so the generated line keeps resolving. `polint_deps_path_prefix` (`:1105-1117`) gains a `crates/polint-sdk` probe. **`initial_pack_main` (`:1160-1173`) and `register_rule_in_pack_main` (`:1175+`) are untouched** — `polint::runner::run_cli(vec![…])` is still the generated `main`.

### 11.5 `polint rules migrate`

New subcommand. Transactional, using the existing `ScaffoldWrite`/`commit_new_rule_scaffold_with`/`rollback_new_rule_scaffold` machinery (`cli/mod.rs:752-779, 918-1033`) so a partial failure restores every file byte-for-byte.

It edits **only** `<pack>/Cargo.toml`:

1. rewrite the `polint` dependency to `{ package = "polint-sdk", version = "<current>", … }`, preserving `default-features` and `features` verbatim;
2. leave every other dependency, `[lints]`, and `[workspace]` untouched;
3. print a diff and require `--yes` in non-interactive contexts;
4. `--check` mode exits 1 with the diff and writes nothing.

It never touches `src/**`. Migration for the 17 in-repo example packs is a single mechanical PR.

### 11.6 Error messages

| Situation | Message |
| --- | --- |
| pack on `polint = "0.2"` under 0.5.0 | `polint: rules host: <pack>/Cargo.toml depends on polint 0.2, which predates the rule protocol. Run \`polint rules migrate <pack>\` (rewrites one dependency line; your rule sources are unchanged). See docs/CONSUMER-SETUP.md#migrating-rule-packs` |
| protocol-via-facade | `polint: <pack> depends on the polint facade rather than polint-sdk, so it recompiles the analysis engine on every rule change. Run \`polint rules migrate <pack>\` to cut build time. This is informational; the scan is correct.` |
| `sdk_abi` mismatch | `polint: rules host: <pack> was built against polint-sdk ABI <n>; this polint accepts <min>..=<max>. Run \`cargo update -p polint-sdk --manifest-path <pack>/Cargo.toml\`, or \`polint cache clean --category rules-bin\` to force a rebuild.` |
| `artifact-only` miss | `polint: no rule artifact for key <key> (<pack>). Ask the pack owner to run \`polint rules build --emit-artifact <dir>\` on a matching toolchain (<rustc version>, <triple>), or set \`execution = "native"\`.` |
| unpinned artifact | `polint: refusing artifact <url> — sha256 <digest> is not in .polint/rules-artifacts.lock. Add it with \`polint rules lock\`, or pass --trust-artifacts for this run.` |

All reuse the `polint: rules host:` prefix convention (`cli/rules_host_error.rs:3`) so existing user-side log filters keep working.

### 11.7 Existing rule packs in this repository

The 17 `examples/*/.polint/rules` packs use `polint = { workspace = true }` (`examples/basic/.polint/rules/Cargo.toml`), resolved from `Cargo.toml` `[workspace.dependencies] polint = { path = "crates/polint", version = "0.2.1" }`. The migration is a **single** workspace-dependency change:

```toml
polint = { package = "polint-sdk", path = "crates/polint-sdk", version = "0.3.0" }
```

Every example pack then migrates with zero per-pack edits, and `scripts/bump-workspace-version.py` (which rewrites internal path-dependency pins, per its docstring at lines 9-11) needs one added pattern for the renamed dependency.

### 11.8 Release mechanics

`scripts/publish-crates.sh:11-15` → `PACKAGES=(polint-macros polint-sdk polint)`, in that order, reusing the existing `crate_version_exists`/`wait_for_crate_version` gating. `polint-engine` starts `publish = false`; if Phase J needs it published (so third parties can build a host), it is inserted between `polint-sdk` and `polint`. `.github/workflows/release.yml` and `release-dry-run.yml` gain the extra dry-run step. `docs/RELEASING.md` documents the three-crate order and the `sdk_abi` bump rule: **any change to a fact-row field, a snapshot section, or a protocol frame bumps `sdk_abi`, and `sdk_abi` never decreases.**

---

## 12. Documentation plan

| File | Change |
| --- | --- |
| `README.md` | Rework the install/first-run narrative: `cargo install polint --locked` still installs the CLI (`:54`); a first scan compiles a *thin* rule pack, not the engine; warm scans run no Cargo at all. Update the cache-layout block (`:186-192`) with `rules-bin` and `snapshots` and their roles. Replace the "fully cold first run can still pay…" paragraph (`:464-469`) with measured Phase A/§10 numbers. Add a short trust-model paragraph pointing at `--untrusted`. |
| `docs/CONSUMER-SETUP.md` | Rewrite the Rust-toolchain section (`:3-10`): the toolchain is needed only to *build* rule packs, and not at all in `artifact-only` mode. Update the env table (`:113-119`) with `POLINT_RULE_BACKEND`, `POLINT_RULES_OFFLINE`, `POLINT_RULES_BIN_MAX_MB`, `POLINT_SNAPSHOT_MAX_MB`, `POLINT_USER_CACHE_DIR`, `POLINT_RULE_TIMEOUT_SECS`, `POLINT_RULE_LOG`, `POLINT_KEEP_SNAPSHOT`, `POLINT_RULE_SANDBOX`. Update the cache-dir table (`:133-138`). **Delete the hand-rolled CI cache recipe (`:199-243`)** — it hand-runs `cargo clean --release -p <pkgid>`, which the artifact cache makes obsolete and actively wrong. Add "Migrating rule packs" (§11.5) and "Running polint on a repository you do not trust" (§7). Extend the rules-host troubleshooting list (`:251-261`) with the protocol failure kinds. |
| `docs/GITHUB-ACTION.md` | Document the new `rules-execution` and `rules-artifact-sources` inputs, the extended `rule-build-cache-*` outputs, and that `rules-bin` is cached alongside `rules-target`. State that a runner with no Rust toolchain works in `artifact-only`. |
| `ARCHITECTURE.md` | `:26-91` — replace "The product publishes two Cargo packages" with the three-package graph and a new mermaid diagram; keep the private-module table but scope it to `polint-engine`. Add "Host and rule-process boundary" covering the snapshot, the protocol, and the policy RPC. Update `:112-127` (supported surface) to say the surface now lives in `polint-sdk` and is reached as `polint::sdk` either way. Update `:86-90` to name `sdk_dependency_closure.rs` alongside the existing gates. |
| `AGENTS.md`, `docs/AGENT-PLAYBOOK.md` | Update every "the rule pack depends on `polint`" statement; add the `package = "polint-sdk"` line; note that editing a rule triggers a thin rebuild and editing sources triggers none. |
| `docs/RELEASING.md` | Three-crate publish order; the `sdk_abi` bump rule; the vendored-SDK asset regeneration step. |
| `docs/API-VISIBILITY-PLAN.md` | Record that the 116-name prelude moved crates without changing; add the promotion record for `polint_sdk::{protocol, snapshot}` as `#[doc(hidden)]` non-API. |
| `docs/facts/*.md` | Add a "Snapshot section" line to each fact document naming the section that carries it and whether it is capability-gated. |
| `docs/schemas/` | Update `polint-cache-status-v1.json` (two enum values). Add `polint-fact-snapshot-v1.json`, `polint-rule-host-protocol-v1.json`, `polint-rule-artifact-v1.json`. **Unchanged:** `polint-report-v1.json`, `polint-rule-inspect-v1.json`, `polint-test-report-v1.json`, `polint-ai-friendly-v1.json`, `polint-explain-v1.json`, `polint-facts-v1.json`, `polint-ignores-v1.json`, `polint-unknowns-v1.json`. |
| `crates/polint-engine/src/cli/skill.rs` | The generated skill is user-facing documentation. Update `:185` (`allowed-tools`; `Bash(cargo:*)` may no longer be needed for warm runs), `:287-288` (pack layout), `:330-344` (registration snippet — unchanged source, but add the manifest note), `:418-433` (review section, `--changed-files` removal). |
| `examples/*/README.md` | Regenerate the "how this pack is built" sentence; no rule source changes. |
| `examples/README.md`, `research/README.md` | Cross-link the new architecture section. |
| **New** `docs/RULE-EXECUTION.md` | The single reference for the protocol, the snapshot, the artifact cache, the fingerprint inputs, the trust modes, and the failure taxonomy. Linked from README, CONSUMER-SETUP, and ARCHITECTURE. |

Claims that must be **removed or corrected**, not just amended:

* `README.md:225` — "For shared compile reuse across worktrees, install sccache" is no longer the primary answer for rule packs; the artifact cache is.
* `docs/CONSUMER-SETUP.md:251` — "When the parent CLI runs `cargo run --manifest-path …/.polint/rules/Cargo.toml`" is factually superseded.
* `ARCHITECTURE.md:28-31` — "The product publishes two Cargo packages" becomes three.
* `docs/CONSUMER-SETUP.md:8-10` — the MSRV failure mode changes: with the vendored SDK, the MSRV that matters is `polint-sdk`'s, not the engine's.

---

## 13. Risks, unresolved decisions, and the decision log

<a id="phase-13"></a>

### 13.1 Risks

| ID | Risk | Likelihood | Impact | Mitigation | Trigger to act |
| --- | --- | --- | --- | --- | --- |
| R-1 | Snapshot cost exceeds compile savings on small repos | medium | high | measure per corpus size; if small-repo warm runs regress, add an in-host fast path for packs whose views are all snapshot-cheap | KC-3 |
| R-2 | Policy RPC serialization loses fidelity, changing `PolicyViolation::stable_key` and therefore diagnostic fingerprints | medium | high | T-EQ-5 proptest; goldens for all four policy templates | KC-6 |
| R-3 | Symbol-query reimplementation diverges in ordering (the `reference_order` stable-key tiebreak, `query.rs:77-97`) | medium | high | T-EQ-3 differential test on generated fact sets | KC-6 |
| R-4 | `[patch]` in a `--config` file is unsupported on MSRV 1.95 | medium | medium | fallback: generated throwaway workspace manifest (§6.6) | D-7 resolution |
| R-5 | Phase C's 11 mechanical moves conflict with in-flight feature work | high | medium | land C in one week, freeze `crates/polint/src` moves, prefer `git mv` + re-export so merges are textual | — |
| R-6 | `sdk_abi` churn during development invalidates every cached artifact daily | high | low | dev builds key on `sdk_version` including a git-describe suffix; only releases bump `sdk_abi` | — |
| R-7 | Source text dominates snapshot size on large repos | medium | medium | `include_source_text` is already request-gated; if it dominates, add per-file lazy load keyed on `content_hash` — this changes `SourceFile.source` from a plain field to a materialized-on-load field, so it needs a design note first | KC-4/KC-5 |
| R-8 | Users depend on `polint::sdk::__private` or `polint::_bench` | low | low | both are `#[doc(hidden)]`/feature-gated; the leak gate covers the supported surface | — |
| R-9 | Hard-link fallback to copy inflates `rules-bin` on Windows / cross-device caches | medium | low | LRU ceiling (§6.9); record `binary_len` so status reporting is honest | KC-7 |
| R-10 | Extension host and rule host diverge into two protocols | medium | medium | Phase G7 makes the extension host reuse `rules_artifact`; a follow-up may unify the protocols, but not in this plan | — |
| R-11 | Removing the child's `tracing-subscriber` init (`runner/mod.rs:145-149`) breaks someone's `RUST_LOG` workflow | low | low | `POLINT_RULE_LOG` replacement documented; host stderr still surfaces rule output | — |
| R-12 | `polint test` behaviour changes because one process now serves many fixture repos, leaking state between cases | medium | high | `AnalysisDb` is per-`RunRequest`, so facts, options, and diagnostics cannot cross cases. One piece of process-global state does survive: `sdk/scope.rs`'s `cached_matcher` memo (`OnceLock<RwLock<HashMap<String, Option<GlobMatcher>>>>`, `sdk/scope.rs:51-63`). It is a pure function of the pattern text, so reuse across cases is correct, but any future process-global state must clear the same bar. Add an explicit test that case N's diagnostics are unaffected by case N-1 | KC-6 |

### 13.2 Decisions already fixed

| ID | Decision |
| --- | --- |
| DF-1 | Rules stay Rust; no DSL as primary model |
| DF-2 | Rule `.rs` sources stay byte-identical; only the generated `Cargo.toml` changes |
| DF-3 | Four packages: `polint-sdk`, `polint-engine`, `polint`, `polint-macros`; `polint` keeps the CLI and the crates.io name |
| DF-4 | Rule packs depend on `polint-sdk` renamed to `polint` via `package =` |
| DF-5 | Engine-internal `AnalysisDb` is renamed `HostFactDb`; `polint::sdk::__private::AnalysisDb` names the SDK snapshot type; the macro is unchanged |
| DF-6 | One rule process per pack per invocation, framed stdio, file-based snapshot transfer |
| DF-7 | Policy views become host-side RPC |
| DF-8 | `cargo build` + artifact capture replaces `cargo run` |
| DF-9 | No native `cdylib` ABI; no remote execution; WASM only as a later distribution backend |
| DF-10 | Native mode cannot confine build scripts or proc macros; `--untrusted` is the honest control |

### 13.3 Unresolved — require benchmark or verification evidence

| ID | Question | Evidence needed | Gate |
| --- | --- | --- | --- |
| D-1 | Is a binary section codec needed instead of JSON? | Phase D8 + P-3 on medium/large | KC-4 |
| D-2 | Does the serialized policy-RPC mutex cost real time? | count and duration of policy queries on template-generated packs | > 10% of warm wall-clock ⇒ build the `request_id` dispatcher |
| D-3 | Should the thin SDK keep `toml` (for `RuleConfigValue`) or introduce an SDK-owned value enum? | B1 closure count with and without | only if KC-2 trips |
| D-4 | Should source text be lazily loaded per file? | R-7 measurement | KC-4/KC-5 |
| D-5 | Should `polint-engine` be published? | whether third parties need to build hosts (Phase J) | Phase J scoping |
| D-6 | Is `parallel` rule execution in the rule process worth keeping without `rayon`? | C6 measurement vs. serial | if `std::thread::scope` is within 5% of serial on all examples, consider dropping the flag |
| D-7 | Does `[patch]` work in `cargo --config <file>` on MSRV 1.95? | direct experiment | R-4 fallback |
| D-8 | Should the default for an unknown repo flip from `native` to `none`? | user/issue evidence after 0.4.0 | a major release only |
| D-9 | Should ed25519 signing replace digest pinning? | key-distribution design | deferred past Phase J |
| D-10 | Can the extension protocol and the rule protocol be unified? | after both ship | post-0.5.0 |
| D-11 | Detached artifact signatures | see D-9 | deferred |

### 13.4 Decision log template

Every resolved decision lands as one file at `docs/decisions/DL-<nnn>-<slug>.md`, linked from `docs/RULE-EXECUTION.md`.

```markdown
# DL-<nnn>: <one-line decision>

- **Status:** proposed | accepted | superseded by DL-<nnn> | rejected
- **Date:** YYYY-MM-DD
- **Phase:** A–K
- **Decides:** D-<n> from docs/RULE-EXECUTION.md §Unresolved
- **Owner:** <name>

## Context
What forced the choice. Link the exact repository symbols/paths and the
measurement that raised the question.

## Options considered
| Option | Cost | Risk | Reversibility |
| --- | --- | --- | --- |

## Evidence
Commands run, machine, corpus, scenario, median and p90. Link the artifact
under research/evaluation-harness/baselines/.

## Decision
One paragraph. State what is now true.

## Consequences
- Invariants affected (I1–I5): …
- Public surface affected: …
- sdk_abi bump required: yes/no
- Tests added or changed: …
- Docs changed: …

## Revisit if
Concrete, measurable condition.
```

---

## 14. Recommended order of execution and the first PR

<a id="phase-14"></a>

### 14.1 Order

```
A (measure)                     ─ 1 week, no product change
   └─ B (guards)                ─ 3 days, tests only
        └─ C1..C11 (SDK split)  ─ 2 weeks, mechanical, one PR per task
             ├─ D (snapshot)    ─ 1.5 weeks   ┐ D and E overlap: E1/E2 need
             └─ E (protocol)    ─ 2 weeks     ┘ only D2's wire types
                  └─ F (artifacts) ─ 1.5 weeks
                       └─ G (CLI integration) ─ 1.5 weeks
                            └─ H (equivalence, flip default) ─ 1 week
                                 └─ I (docs, action, release 0.4.0)
                                      └─ J (prebuilt artifacts, optional)
                                           └─ K (WASM gate — evaluate, do not build)
```

Sequencing rules:

* **A before everything.** Without the baseline, no budget in §10 is checkable and the three unverified numbers in §0 stay unverified.
* **B before C.** The closure guard must exist before the SDK does, so the first regression is caught by CI rather than by review.
* **C is a freeze window.** Eleven `git mv`-shaped PRs against `crates/polint/src` will conflict with anything else touching those trees. Land them fast and serially.
* **D and E overlap deliberately**, joined at D2.
* **F before G.** The CLI rewrite should target the artifact resolver, not `cargo run`, so G is written once.
* **H gates the default flip.** KC-6 (any golden divergence) blocks H5 unconditionally.
* **I6 (deleting the legacy path) is a separate release**, ≥ 90 days after 0.4.0.

### 14.2 The first implementation PR — landed

**Title:** `perf(bench): add the rule-host build-cost baseline harness`
**Phase:** A (tasks A1, A3, A4). **Product behaviour change: none; no file under `crates/polint/src` changed.**

**Files, as landed:**

* new `crates/polint-bench/src/build_cost/mod.rs` — scenario matrix, metric set, report schema, baseline diff table
* new `crates/polint-bench/src/build_cost/scratch.rs` — scratch repository materialization, manifest rewrite, directory accounting, scenario edits
* new `crates/polint-bench/src/build_cost/shim.rs` — the Cargo and `rustc` shims that count invocations and compiled units
* `crates/polint-bench/src/main.rs`, `src/lib.rs`, `Cargo.toml` — subcommand dispatch, shim-mode detection before argument parsing, `serde`/`serde_json`/`toml` and a `tempfile` dev-dependency
* new `research/evaluation-harness/baselines/build-cost.json` — schema `polint-build-cost-1`, one machine
* `Makefile` — `build-cost` and `build-cost-baseline` after `scale-corpus-run`
* `research/evaluation-harness/README.md` — metric definitions, scenarios, and limits

Divergences from the pre-implementation sketch, and why: one module became three (the shim has to be reachable before argument parsing, and scratch management is the largest single concern); the smoke test lives in the bench crate's own `#[cfg(test)]` modules rather than in `crates/polint-engine/tests/` — which does not exist until Phase C10, so the original file path was unbuildable at Phase A; and compiled units are counted through `RUSTC_WRAPPER` rather than parsed from `cargo build --timings`.

**What it measures** (per cell of {repo} × {cold, warm-noop, warm-rule-edit, warm-source-edit, test-suite}): Cargo invocation count and failures, Cargo wall-clock, `rustc` invocations, compiled units, end-to-end wall-clock, rule-host wall-clock and peak RSS, bytes before/after/written and files retained for both the rule-host `CARGO_TARGET_DIR` and the polint cache, the `CARGO_HOME/registry` byte delta, and the `polint test` tally. `POLINT_CARGO` (`cli/mod.rs:4260-4262`, `rule_test.rs:330-332`), `POLINT_RULES_TARGET_DIR`, `POLINT_RULES_PROFILE`, `POLINT_CACHE_DIR`, and `POLINT_GOLDEN_COST_PATH` all already exist, so no product code was needed.

**Acceptance, as met:**

1. `polint-bench build-cost --repo examples/basic --scenario cold` emits a `polint-build-cost-1` report carrying every `METRIC_KEYS` entry, with unobservable metrics `null` and named in `limits` — *not* "all metrics non-null", which `compiler_peak_rss_bytes` cannot satisfy without instrumenting the engine.
2. `make build-cost` runs the matrix and prints the measured/baseline ratio per headline metric. ✔
3. The committed baseline replaces the reported-but-unverified figures with measured ones **on one machine**. M-CI (2 vCPU / 4 GB) is not recorded and is absent rather than estimated; §10's ratio budgets are therefore concrete only for the recorded machine. ✔ (partial)
4. `cargo fmt`, `cargo clippy -p polint-bench --all-targets --all-features -- -D warnings`, and `cargo test -p polint-bench` green; no file under `crates/polint/src` changed. ✔

**Why this PR first:** it is the only piece with zero architectural risk; it is independently valuable (a build-cost regression gate the repo does not have today); it turns §0's three unverified claims into measured facts before any of them justifies a design; and it produces the `POLINT_CARGO` shim that every later phase's "0 Cargo invocations" assertion depends on.

**Next PR:** Phase A2 (widen the matrix, which needs `scanned_sources` to recurse) or Phase B1 (the closure guard against the leak probe). B does not depend on A2.

---

## 15. Traceability

| # | Requirement | Plan § | Phase / tasks | Acceptance tests |
| --- | --- | --- | --- | --- |
| 1 | Executive goal, invariants, out-of-scope (DSL-first, native cdylib ABI, remote-first) | §1.1–1.3 | governs all | A-1, A-2, G-1, G-2 |
| 2 | Current architecture, exact call/data/build path with paths and line ranges | §2.1–2.4 | A | A-3, A-4, P-2 |
| 3 | Target package graph; Cargo cycle resolved; alternatives; package/API deltas | §3.1–3.3 | C1, C10, C11 | A-3, A-4, A-5, X-4 |
| 4a | What moves to the SDK / what stays in the engine | §4.1–4.2 | C1–C10 | A-4, A-5, B5 |
| 4b | `AnalysisDb` → owned `FactSnapshot`; lifetime/borrow preservation | §4.3 | C11, D3, D4 | U-1, C-2, A-2 |
| 4c | Serialization format, schema/version/digests | §4.4 | D1, D2, D7 | U-1, F-1, F-2, C-2 |
| 4d | Capability support metadata | §4.6 | E1, E4 | C-1, G-4 |
| 4e | Review changesets | §4.6 | G4 | G-1 on `examples/review-rules`, `examples/gorm-review-indexes` |
| 4f | Options | §4.6 | D2 | U-3 |
| 4g | Diagnostics | §4.6 | E3, E4 | G-1, G-2, I-1 |
| 4h | Stable keys | §4.3, §4.4 | D3, D4, D5 | T-EQ-3, C-2, U-2 |
| 4i | Fact-family sections; avoiding whole-DB serialization | §4.4 | D3 | D3 acceptance (3 sections for `{syntax, imports}`), C-1 |
| 5a | Manifest handshake | §5.2 | E1, E4 | I-5, G3 acceptance |
| 5b | Run request; snapshot transfer | §5.3, §5.4 | E1, E4, D3 | I-1, C-2 |
| 5c | stdout/stderr limits, timeouts | §5.6 | E6 | F-6, F-7 |
| 5d | Exit/error protocol; version negotiation | §5.7 | E7, E8 | F-1, F-2, F-8, E7 acceptance |
| 5e | Determinism, cancellation | §5.6, §5.7 | E3, E6 | C-3, C-4, C-5, F-6 |
| 5f | One process vs two | §5.1 | E4 | I-5, I-6 |
| 5g | Compatibility with report/inspect JSON schemas | §5.8 | E4, G1, G3 | G-1, G-2, C7 acceptance |
| 6a | Source fingerprint inputs | §6.1, §6.2 | F1 | U-5, U-6 |
| 6b | Current-artifact detection | §6.3 | F4 | I-2, F-3, F-4, F-5 |
| 6c | Direct binary execution bypassing Cargo | §6.3 | F4 | I-2, I-4, O-1 |
| 6d | Single build for all fixture cases | §6.4 | G5 | I-6 |
| 6e | Cargo flags | §6.3, §6.5 | F3, F6 | O-3, O-4 |
| 6f | Offline/locked operation | §6.5 | F6 | O-3, O-4 |
| 6g | Vendored SDK | §6.6 | F6 | O-3 |
| 6h | Target dir location and cleanup | §6.7 | F2, F3 | H4, KC-7 |
| 6i | User-level cache | §6.8 | F6, J3 | O-3, Phase J acceptance |
| 6j | Disk ceilings, LRU | §6.9 | F5 | F5 acceptance |
| 6k | Prebuilt native artifacts | §6.10 | J1–J5, F8 | Phase J acceptance (no-toolchain runner) |
| 6l | Signing/digests | §6.11 | J2, J3 | F-11 |
| 6m | Explicit native trust mode | §6.12 | J1, J4 | H3, F-11 |
| 7a | Customer-controlled `Cargo.toml` | §7.3 | F7 | U-7, F-9 |
| 7b | build.rs, proc macros, dependencies | §7.4 | F7, §12 docs | I1 (docs claim), O-3 |
| 7c | Rule binary controls | §7.5 | E6 | F-6, F-7, X-3 |
| 7d | Snapshot files | §7.6 | D7, E6 | F-1 |
| 7e | Path traversal | §7.7 | F1, F7 | U-6, F-10 |
| 7f | Untrusted fresh repos | §7.4, §7.9 | J1 | H3 (`--rules-execution none`) |
| 7g | Artifact signatures | §6.11, §7.8 | J2, J3 | F-11 |
| 7h | Sandbox boundaries; what native cannot guarantee | §7.4, §7.5 | §12 docs | I1 |
| 7i | Defaults for owned / shared / untrusted repos | §7.9 | J1, I2 | H3 |
| 8a | Measurement/baseline harness | §8 Phase A | A1–A6 | A3, P-2 |
| 8b | Dependency-closure and feature-leak guard | §8 Phase B | B1–B5 | A-5, A-6, A-4 |
| 8c | SDK extraction | §8 Phase C | C1–C11 | A-1, A-2, A-3, A-4, G-1 |
| 8d | FactSnapshot and serialization | §8 Phase D | D1–D8 | U-1, C-2, T-EQ-3 |
| 8e | Host/rule protocol | §8 Phase E | E1–E8 | I-1, F-1..F-8, C-5 |
| 8f | Rule build fingerprint/cache and direct execution | §8 Phase F | F1–F8 | I-2, I-3, I-4, U-5, U-6 |
| 8g | Runner/CLI integration | §8 Phase G | G1–G7 | G-1, I-5, I-6, I-7 |
| 8h | Test harness / golden equivalence | §8 Phase H | H1–H5 | G-1..G-4, T-EQ-3, T-EQ-5 |
| 8i | Docs/action/release updates | §8 Phase I, §12 | I1–I6 | H4, X-4 |
| 8j | Optional prebuilt artifact path | §8 Phase J | J1–J5 | F-11, Phase J acceptance |
| 8k | Later WASM backend decision gate | §8 Phase K | gate only | gate criteria 1–4 |
| 9 | Testing/verification matrix | §9.1–9.8 | all | the matrix itself |
| 10 | Performance budgets and experiment design | §10.1–10.6 | A, plus gates in D/F/H | P-1, P-2, P-3, KC-1..KC-7 |
| 11 | Migration and release preserving existing users | §11.1–11.8 | I4, plus `polint rules migrate` in the F/G window | X-4, I-1, compatibility-mode tests in H3 |
| 12 | Documentation plan | §12 | I1, I3 | doc-claim review in I1; `cargo doc` job (`ci.yml:33`) |
| 13 | Risks, unresolved decisions, decision log template | §13.1–13.4 | continuous | DL files under `docs/decisions/` |
| 14 | Final order of execution and first PR | §14.1–14.2 | — | first-PR acceptance 1–4 |
