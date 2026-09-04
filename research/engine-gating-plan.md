# Engine Gating Plan — Deep Analysis Behind License Key

**Date:** 2026-07-28  
**Question:** How to keep syntax-level analysis free/OSS while gating the deep analysis engine behind an offline license key (`POLINT_LICENSE_KEY`, HMAC, no server)?  
**Sources inspected:** workspace `Cargo.toml`, `crates/polint/Cargo.toml`, `crates/polint/src/analysis/mod.rs` (+ submodule headers/LOC), `crates/polint/src/sdk/facts.rs`, `crates/polint/src/runner/mod.rs`, `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/src/analysis_plan.rs`, `crates/polint-macros/src/lib.rs`, `crates/polint/src/core/mod.rs` (capability support / rule runner).  
**Related:** `research/monetization-brief.md`, `research/monetization-deep-eval.md`, `research/monetization-cursor-eval.md`.

---

## Verdict

Gate at the **capability + kernel pipeline** boundary that already exists. Do **not** start with a crate split.

Paid unlock = the three deep public capabilities `calls`, `control_flow`, and `dataflow`. Those are the only SDK fact views that turn on `SEMANTIC_PIPELINE_*` / `CFG_CALL_*` / `FULL_REFINEMENT_*` / `DATA_FLOW_*` in `analysis_kernel/mod.rs`. Everything under `crates/polint/src/analysis/` is private engine code behind that pipeline (~113K LOC). Syntax, metrics, imports, module graph, and symbols stay free.

**Minimum viable change:** runtime license check → mark paid capabilities `LicenseRequired` when the key is missing/invalid → existing `has_blocking_capability` skips those rules and emits `polint/capability` diagnostics → AND the license into the four kernel pipeline booleans so deep providers never recompute. ~1 week of product code; weeks 2–3 for Stripe key issuance, Action wiring, docs, and optional `feature = "deep-analysis"` compile gate.

Confidence: **High** for the free/paid cut (matches kernel trigger constants). **Medium** for dual-license crate extraction (needed only if crates.io must ship without deep sources).

---

## 1. Module map: deep vs syntax-level

### 1.1 The real free/paid cut is not “folders named analysis”

`crates/polint/src/analysis/` is **already the deep semantic substrate**. It is `pub(crate)` and never part of the public SDK (`lib.rs` only exports `sdk` + `runner`). Free analysis lives mostly **outside** that tree:

| Area | Path | Role | Tier |
|------|------|------|------|
| Go syntax facts | `crates/polint/src/go/` (not `go/semantic`) | tree-sitter extract: functions, imports, tests, branch obligations, literals, … | **Free** |
| TS/JS syntax facts | `crates/polint/src/ts/` | Oxc extract: functions, imports, components, classes, JSX attrs, … | **Free** |
| Module graph | `crates/polint/src/module_graph/` | resolved imports + module nodes/edges | **Free** |
| Symbol graph | `crates/polint/src/symbol_graph/` | symbols, definitions, references | **Free** |
| Metrics | `crates/polint/src/metrics.rs` | file / function / complexity metrics | **Free** |
| Policy query façade | `crates/polint/src/policy_queries.rs` | implements `Events`/`Calls`/`ControlFlow`/`DataFlow` query methods | Free for `Events`; paid paths only reachable via paid views |
| Host / config / cache | `fs`, `cache`, `config`, `diagnostics`, `sdk`, `runner`, `cli` | product shell | **Free** |
| Deep engine | `crates/polint/src/analysis/**` | MIR → CFG → calls → domains → summaries → reachability → solver → dataflow | **Paid** |
| Go semantic sidecar | `crates/polint/src/go/semantic/` | RTA / Go semantic facts; kernel runs only when `run_full_refinement_pipeline` | **Paid** |

Kernel pipeline gates today (`analysis_kernel/mod.rs:31–60`):

```text
CROSS_FILE_ANALYSIS_TRIGGER   = resolved_imports | module_graph | symbols | references
                                | calls | control_flow | dataflow     ← free + paid
SEMANTIC_PIPELINE_TRIGGER     = calls | control_flow | dataflow       ← PAID
CFG_CALL_PIPELINE_TRIGGER     = calls | control_flow | dataflow       ← PAID
FULL_REFINEMENT_PIPELINE      = calls | control_flow | dataflow       ← PAID
DATA_FLOW_PIPELINE_TRIGGER    = dataflow                              ← PAID
```

`events` alone does **not** start the semantic pipeline (regression: `events_only_plan_stays_off_semantic_pipeline`).

### 1.2 Every `analysis/` submodule classified

LOC from `find … | wc -l` on 2026-07-28 (~112.5K under `analysis/`).

#### Deep — providers / algorithms (paid engine body)

| Module | Approx LOC | Kernel provider / role | Trigger flag |
|--------|-----------:|------------------------|--------------|
| `calls/` | 19.1K | `polint.calls` | semantic / cfg-call |
| `solver/` | 12.7K | `polint.solver` (+ Go RTA / TS object-model policies) | full refinement |
| `entrypoints/` | 8.1K | `polint.entrypoints` | full refinement |
| `summaries/` | 7.3K | `polint.direct_summaries` + SCC closure | full refinement |
| `mir/` | 6.3K | MIR lowering (Go/TS); fed by `analysis::provider` as `polint.semantic_mir` | semantic |
| `semantic_graph/` | 6.2K | `polint.semantic_graph` | full refinement |
| `cfg/` | 5.7K | `polint.cfg` | cfg-call |
| `domains/` | 5.4K | `polint.abstract_domains` | full refinement |
| `data_flow/` | 5.3K | `polint.data_flow` | data-flow |
| `refined_calls/` | 4.9K | `polint.refined_calls` | cfg-call |
| `types/` | 4.8K | `polint.type_value_alias` | full refinement |
| `evidence/` | 4.3K | `polint.evidence` | data-flow |
| `extensions/` | 4.0K | `polint.extensions` | full refinement |
| `reachability/` | 3.5K | `polint.reachability` (whole-program; ≠ local domain) | full refinement |
| `identity/` | 3.1K | `polint.identity` | full refinement |
| `demand/` | 2.2K | demand-driven query / SCC / quarantine substrate | deep infra |
| `slicing/` | 2.0K | local + interprocedural slicing helpers | deep infra (not a public capability yet) |
| `unknown_taxonomy/` | 1.6K | unknown / unsupported capability taxonomy for CLI/inspect | deep-adjacent; keep compiled with host or deep |
| `points_to/` | 1.3K | points-to fixpoint; composed into `solver` (D-03) | full refinement |
| `adaptation/` | 0.9K | repo-local adaptation models → semantic-graph constraints | full refinement |
| `aliases/` | 0.6K | type/value alias query stack | full refinement |
| `values/` | 0.3K | value / allocation token facts | deep infra |
| `access_paths/` | 0.2K | access-path facts | deep infra |

#### Deep — shared MIR / store glue under `analysis/` (paid)

| File | Approx LOC | Role |
|------|-----------:|------|
| `provider.rs` | 0.6K | `derive_semantic_mir_with_cache_stats` → `polint.semantic_mir` |
| `validate.rs` | 0.7K | MIR validation |
| `places.rs` | 0.5K | place facts |
| `store.rs` | 0.4K | `SemanticStore` |
| `ids.rs` | 0.3K | MIR/call/place IDs |
| `cache_key.rs` | 0.2K | semantic cache key helpers |
| `stable_key.rs` | ~65 | stable fact keys |
| `error.rs` | ~49 | `AnalysisError` |
| `mod.rs` | ~39 | module list |

#### Outside `analysis/` but paid

| Path | Approx LOC | Notes |
|------|-----------:|-------|
| `crates/polint/src/go/semantic/` | ~4.5K | `polint.go.semantic`; only when `run_full_refinement_pipeline` |

#### Free — stays open regardless of license

| Path | Approx LOC | Notes |
|------|-----------:|-------|
| `go/` excluding `semantic/` | ~3.9K of remaining go tree | `polint.go.syntax` |
| `ts/` | ~14.0K | `polint.ts.syntax` |
| `module_graph/` | ~14.6K | `polint.module_graph` (+ topology only when deep runs) |
| `symbol_graph/` | ~11.2K | `polint.symbol_graph` |
| `metrics.rs` | ~1.1K | `polint.metrics` |
| `sdk/`, `runner/`, `cli/`, `config/`, `cache/`, `fs/`, `diagnostics/`, `core/` fact DB shell | — | product + free facts |

**Interpretation:** ~113K LOC in `analysis/` + ~4.5K `go/semantic` is the paid engine. Free surface is still large (~syntax adapters + module/symbol graph + SDK) and matches existing examples (`examples/go-import-boundaries`, `examples/ts-complexity`, etc.).

### 1.3 Provider order (paid slice)

When a paid capability is requested **and** licensed, kernel order is already:

1. source → `go.syntax` → `ts.syntax` → `module_graph` → `symbol_graph` *(always for those plans)*  
2. `module_topology` → `semantic_mir` → `cfg` → `calls`  
3. `go.semantic` → `identity` → `abstract_domains` → `direct_summaries` (+ SCC) → `entrypoints` → `reachability` → `extensions` → `type_value_alias` → `semantic_graph` → `solver` → `refined_calls`  
4. `data_flow` → `evidence` *(only if `dataflow` requested)*  
5. `metrics` *(capability-driven; free)*

License gating must force steps 2–4 off when unlicensed, even if rules request paid capabilities.

---

## 2. How to split: feature vs crate vs MVP

### Recommendation ranking

| Approach | Use when | Effort | Recommendation |
|----------|----------|-------:|----------------|
| **A. Runtime capability gate** | Ship Pro unlock in one binary | ~3–5 days | **MVP — do this first** |
| **B. Cargo feature `deep-analysis`** | Shrink OSS builds / hide symbols | +2–4 days | Week 2–3 add-on |
| **C. Separate crate `polint-deep`** | Dual-license / stop shipping deep sources on crates.io | 1–2+ weeks | Deferred until licensing counsel / crates.io policy needs it |

### 2.1 Why not crate-split first

Deep modules are woven into one crate today:

- `AnalysisDb` in `core/mod.rs` holds MIR, CFG, calls, domains, summaries, reachability, solver, dataflow stores.
- `analysis_kernel/mod.rs` calls providers with a long sequential dependency chain.
- Eval / tests / `go::semantic` sit beside free adapters.

Extracting `polint-deep` means splitting `AnalysisDb`, provider registration, and ~hundreds of internal imports. Wrong first move for a 2–3 week revenue cut.

### 2.2 Minimum viable change (Approach A)

Keep a **single** `polint` crate (current layout). Add offline license resolution. When invalid:

1. Paid capabilities become non-`Supported` in the analysis plan.
2. Rules requesting them are skipped (`has_blocking_capability` already does this).
3. Kernel pipeline booleans stay false → deep providers emit empty digests and do not recompute.

No user-facing dual API. No `xxxLegacy`. Brave cut: paid = those three capabilities only.

### 2.3 Cargo feature (Approach B) — optional compile gate

```toml
# crates/polint/Cargo.toml
[features]
default = ["deep-analysis"]
deep-analysis = []
```

- Proprietary / GitHub release binaries: `--features deep-analysis` (default).
- crates.io “community” build later: `--no-default-features` if/when sources are dual-licensed or deep code is removed from the published tarball.
- Gate with `#[cfg(feature = "deep-analysis")]` on `mod analysis` submodules **and** the paid branches in `analysis_kernel/mod.rs`. Without the feature, `support_for("calls"|"control_flow"|"dataflow")` must return unsupported (or license-required) at compile time so macros still compile against the SDK types.

Feature alone does **not** monetize — anyone building from source with the feature enabled gets deep analysis. Feature is for distribution packaging; license is for unlock.

### 2.4 Separate crate (Approach C) — later dual-license shape

```text
crates/polint          MIT — syntax, SDK, runner, kernel shell, free providers
crates/polint-deep     Proprietary / commercial — analysis/*, go/semantic
```

`polint` depends on `polint-deep` optionally via feature. Requires moving stores onto a trait or `AnalysisDb` extension object. Do after MVP proves conversion.

### 2.5 Licensing note (product, not code)

Repo is MIT today (`workspace.package.license`). Runtime gating of MIT-published deep sources is a **commercial distribution** strategy (binary + key), not a substitute for open-core dual licensing. For true “deep not in OSS tarball,” plan B+C and stop publishing deep sources under MIT. Do not relicense the whole tree to BSL (see `monetization-cursor-eval.md`).

---

## 3. License key validation (offline HMAC)

### 3.1 Requirements (as specified)

- Read `POLINT_LICENSE_KEY` from the environment (CLI / CI / Action).
- Validate locally with HMAC — **no phone-home server**.
- Valid key unlocks deep modules; invalid/missing → free path only.

### 3.2 Key format

Use a parseable, versioned token (no opaque blob):

```text
polint_1.<payload_b64url>.<mac_b64url>
```

**Payload** (JSON, then URL-safe base64 without padding):

```json
{
  "v": 1,
  "tier": "pro",
  "sub": "cus_…_or_org_slug",
  "iat": 1780000000,
  "exp": 1782678400,
  "features": ["deep"]
}
```

| Field | Meaning |
|-------|---------|
| `v` | Format version |
| `tier` | `pro` \| `team` (Team may add org id later; same `deep` feature for MVP) |
| `sub` | Stripe customer / org id for support, not verified online |
| `iat` / `exp` | Issued / expiry unix seconds; reject if `now > exp` |
| `features` | Must contain `"deep"` to unlock paid capabilities |

**MAC:** `HMAC-SHA256(secret, "polint_1." || payload_b64url)` → URL-safe base64.

### 3.3 Where secret lives

| Side | Secret |
|------|--------|
| Key issuer (Stripe webhook / small admin script, **not in this repo**) | `POLINT_LICENSE_HMAC_SECRET` |
| `polint` binary | Same secret embedded at **build time** via `env!("POLINT_LICENSE_HMAC_SECRET")` or `include_str!` generated in CI release workflow |

Caveat (document honestly): HMAC with an embedded secret is reverse-engineerable. Acceptable for solo Pro/Team honesty-based enforcement; upgrade path is Ed25519 (embed verify key only). MVP sticks to HMAC as requested.

### 3.4 New module

**File:** `crates/polint/src/license.rs` (`pub(crate)`)

```text
LicenseStatus { Absent | Invalid { reason } | Valid { claims: LicenseClaims } }
LicenseClaims { tier, subject, expires_at, features: BTreeSet<String> }

fn resolve_from_env() -> LicenseStatus          // reads POLINT_LICENSE_KEY
fn validate_key(raw: &str, now: SystemTime) -> LicenseStatus
fn unlocks_deep(status: &LicenseStatus) -> bool // Valid && features.contains("deep") && !expired
```

Wire into `lib.rs`:

```rust
pub(crate) mod license;
```

### 3.5 Integration points

| File | Change |
|------|--------|
| `runner/mod.rs` | At start of `analyze_and_run` / `inspect_rule`: `let license = license::resolve_from_env();` pass into plan + kernel |
| `analysis_plan.rs` | After collecting capabilities: if capability is paid and `!unlocks_deep`, set status `LicenseRequired` with reason/hint |
| `analysis_kernel/mod.rs` | `let licensed = unlocks_deep(...);` then `run_semantic_pipeline &= licensed;` (same for cfg-call, full-refinement, data-flow) |
| `cli/mod.rs` | Optional `polint license` → prints status (tier, expiry, deep unlocked?) without leaking the key; used by Action / support |
| GitHub Action / docs | Document `POLINT_LICENSE_KEY` secret; fail soft with capability diagnostics (exit policy: prefer diagnostics + `fail-on`, not hard crash) |

### 3.6 Capability status variant

Today (`core/mod.rs`):

```rust
pub enum CapabilitySupportStatus {
    Supported,
    Unsupported,
    SetupMissing,
}
```

Add:

```rust
    /// Implemented, but this host build / run is not licensed for the capability.
    LicenseRequired,
```

Reuse existing machinery:

- `analysis_plan::capability_diagnostic` → emit `polint/capability` with hint: set `POLINT_LICENSE_KEY` or upgrade.
- `has_blocking_capability` treats any non-`Supported` as blocking → paid rules do not execute with empty facts (matches Go lifecycle / setup-missing contract in AGENTS.md).

Do **not** overload `SetupMissing` or `Unsupported` — inspect JSON and agent skills need an honest status.

### 3.7 Cache digests

License must participate in plan/kernel identity when it changes provider sets:

- Include `license.deep=0|1` (not the raw key) in `AnalysisPlan` digest inputs (`analysis_plan.rs` `plan_digest`) **or** in `InputSnapshot` alongside `rule_digest`.
- Switching from unlicensed → licensed must invalidate deep provider cache rows; flipping the pipeline flags without digest participation would serve stale empties.

### 3.8 Dev / test escape hatch

```text
POLINT_LICENSE_KEY=dev          # only when cfg(debug_assertions) OR env POLINT_ALLOW_DEV_LICENSE=1 in tests
```

Or inject `LicenseStatus` via `KernelInput` in unit tests without touching env. Never honor `"dev"` in release builds.

---

## 4. SDK fact views → free vs paid

Mapping source of truth: `crates/polint-macros/src/lib.rs` `capability_for_type` + doc comments on `sdk/facts.rs` + `analysis_plan::support_for`.

### 4.1 Free (stay open)

| Fact view | Capability | Backing providers |
|-----------|------------|-------------------|
| `SourceFiles`, `Packages`, `Functions` | `syntax` | source + go/ts syntax |
| `Imports` | `imports` | go/ts syntax |
| `GoTests` | `go_tests` | go syntax |
| `BranchObligations` | `branch_obligations` | go syntax |
| `TsComponents` | `ts_components` | ts syntax |
| `TsClasses` | `ts_classes` | ts syntax |
| `StringLiterals` | `string_literals` | go/ts syntax |
| `JsxAttributes` | `jsx_attributes` | ts syntax |
| `FileMetrics` | `file_metrics` | `polint.metrics` |
| `FunctionMetrics` | `function_metrics` | `polint.metrics` |
| `ComplexityMetrics` | `complexity_metrics` | `polint.metrics` |
| `ResolvedImports` | `resolved_imports` | `polint.module_graph` |
| `ModuleGraphFacts` | `module_graph` | `polint.module_graph` |
| `Symbols` | `symbols` | `polint.symbol_graph` |
| `References` | `references` | `polint.symbol_graph` |
| `ChangedFiles` | `changeset` | review injection (post-kernel) |
| `Events` | `events` | syntax-level call matching in `policy_queries` (no semantic pipeline) |

### 4.2 Paid (require license + deep pipeline)

| Fact view | Capability | What users buy |
|-----------|------------|----------------|
| `Calls` | `calls` | `forbidden_reachable` over refined call / reachability facts |
| `ControlFlow` | `control_flow` | `missing_guard` / `missing_cleanup` (CFG + refined calls) |
| `DataFlow` | `dataflow` | `forbidden` source→sink over data-flow + evidence |

These three are exactly `SEMANTIC_PIPELINE_TRIGGER_CAPABILITIES` / `CFG_CALL_*` / `FULL_REFINEMENT_*` (and `dataflow` alone for the data-flow slice).

### 4.3 Reserved / unsupported (not sold yet; when enabled → paid)

| Fact view | Capability | Status today |
|-----------|------------|--------------|
| `Cfg` | `cfg` | Unsupported (raw CFG not public); engine CFG is already behind paid pipeline |
| `CallGraph` | `call_graph` | Unsupported; use `Calls` policy view instead |
| `CoverageFacts` | `coverage_facts` | Unsupported |
| `TestSuiteMetrics` | `test_suite_metrics` | Unsupported |

When `cfg` / `call_graph` are promoted to Supported, classify them **paid** (they would need the same pipeline).

### 4.4 Capability dependency note

`capability_dependencies` today:

```text
calls | control_flow | dataflow  →  resolved_imports, module_graph, symbols, references
references → symbols
```

Unlicensed runs may still build free deps if other free rules request them. Paid rules blocked by `LicenseRequired` must **not** force the deep pipeline on. Implementation: plan marks paid caps `LicenseRequired` **before** kernel; kernel only ORs pipeline triggers when status would have been Supported **and** licensed (or simply: pipeline triggers ∩ licensed).

### 4.5 Public contract honesty

- Update `docs/facts/` (or add `docs/facts/license.md`) listing free vs Pro capabilities.
- `polint inspect rule` / manifests should show `LicenseRequired` when applicable.
- Generated skill text (`.claude/skills/polint`, `add-skill`) must say deep policy views need Pro.
- Examples using `Calls` / `ControlFlow` / `DataFlow` need a documented license in CI (dev key) or move to Pro example packs.

---

## 5. Two–three week implementation plan

### Week 1 — Runtime gate (shippable unlock)

| Day | Work | Files |
|-----|------|-------|
| 1 | Add `LicenseRequired` to `CapabilitySupportStatus`; wire serde, inspect JSON (`capability_status_json` / `capability_status_name` in `analysis_plan.rs` + `symbol_graph/mod.rs` if duplicated), public-surface leak probe expectations | `core/mod.rs`, `analysis_plan.rs`, `tests/fixtures/public-surface-leak-probe/`, any snapshot tests |
| 1–2 | Implement `license.rs` (parse, HMAC-SHA256, expiry, `features` contains `deep`); unit tests for valid/invalid/expired/tampered; debug-only `dev` key | **new** `crates/polint/src/license.rs`, `lib.rs` |
| 2 | Define `PAID_CAPABILITIES: &["calls","control_flow","dataflow"]` in `analysis_plan.rs`; in `support_for` / post-collect pass, set `LicenseRequired` + reason/hint/docs when unlicensed | `analysis_plan.rs` |
| 2–3 | Thread `LicenseStatus` into `KernelInput`; AND into `run_semantic_pipeline`, `run_cfg_call_pipeline`, `run_full_refinement_pipeline`, `run_data_flow_pipeline` | `analysis_kernel/mod.rs`, callers |
| 3 | Resolve license in `runner::analyze_and_run` and `inspect_rule`; include `license.deep=` in plan digest | `runner/mod.rs`, `analysis_plan.rs` |
| 3–4 | Integration tests: free rule (e.g. `Imports`) works without key; rule with `DataFlow`/`Calls` without key → capability diagnostic, no deep provider recomputes; with valid key → pipeline on | `crates/polint/tests/` temp-repo style (public SDK only) |
| 4–5 | `polint license` subcommand (status only); stderr one-liner on check when any rule was license-blocked | `cli/mod.rs`, optionally `runner/mod.rs` |
| 5 | Docs: `docs/facts/license.md` or section in capability docs; README Pro blurb | `docs/`, `README.md` |

**Exit criteria week 1:** Unlicensed CI with only syntax/symbol rules unchanged; unlicensed deep-rule packs fail closed with clear diagnostics; forged key rejected; valid HMAC key unlocks deep providers.

### Week 2 — Packaging + issuance

| Day | Work | Notes |
|-----|------|-------|
| 1–2 | Release CI embeds `POLINT_LICENSE_HMAC_SECRET`; publish binary artifacts | Do not commit secret |
| 2–3 | Small out-of-repo (or `tools/license-issue`) script: Stripe webhook / CLI → mint `polint_1.…` keys with 30-day or subscription-aligned `exp` | Keep issuer out of MIT crate if possible |
| 3 | GitHub Action: document `env.POLINT_LICENSE_KEY`; optional soft-fail messaging | Align with Action README |
| 4 | Add Cargo feature `deep-analysis` (default on); `cfg`-gate `mod analysis` paid entry from kernel **or** stub pipeline when feature off | `Cargo.toml`, `lib.rs`, `analysis_kernel/mod.rs` |
| 5 | Example audit: which `examples/*` request paid views; mark Pro or provide test license in their CI | `examples/go-sensitive-writes`, branch-obligation vs control-flow, etc. |

**Exit criteria week 2:** Paying customer can set one env var in CI and run Pro rules; OSS clone without secret builds free path (feature off optional).

### Week 3 — Harden + product polish

| Day | Work |
|-----|------|
| 1–2 | Cache invalidation proof: license flip changes digests; snapshot tests for inspect JSON with `license_required` |
| 2–3 | Skill / `add-skill` text: free vs Pro fact views |
| 3–4 | Decide crates.io story: keep publishing full MIT tree with runtime gate **or** start `polint-deep` extraction spike (design only unless counsel says ship) |
| 4–5 | Optional: Ed25519 design note as follow-up; Team tier claim field without new capabilities; changelog for v0.2.0 / Pro |

**Exit criteria week 3:** Documented Pro SKU, Action recipe, no silent empty-fact execution for paid rules, monetization eval Phase 1 checklist closed.

### Explicit non-goals (this window)

- Separate `polint-deep` crate merge.
- Online license activation / seat counting server.
- Gating free cross-file facts (`symbols`, `module_graph`) — keep funnel strong.
- Promoting raw `Cfg` / `CallGraph` SDK views.
- Relicensing entire repository.

---

## 6. Concrete code sketch (MVP glue)

### 6.1 Kernel (conceptual)

```rust
// analysis_kernel/mod.rs — inside AnalysisKernel::run
let deep_licensed = input.license.unlocks_deep();

let run_semantic_pipeline = deep_licensed
    && input.plan.requests_any_capability(SEMANTIC_PIPELINE_TRIGGER_CAPABILITIES);
let run_cfg_call_pipeline = deep_licensed
    && input.plan.requests_any_capability(CFG_CALL_PIPELINE_TRIGGER_CAPABILITIES);
let run_full_refinement_pipeline = deep_licensed
    && input.plan.requests_any_capability(FULL_REFINEMENT_PIPELINE_TRIGGER_CAPABILITIES);
let run_data_flow_pipeline = deep_licensed
    && input.plan.requests_any_capability(DATA_FLOW_PIPELINE_TRIGGER_CAPABILITIES);
```

Cross-file free analysis (`resolved_imports` / `symbols` / …) stays independent of `deep_licensed`.

### 6.2 Plan support (conceptual)

```rust
const PAID_CAPABILITIES: &[&str] = &["calls", "control_flow", "dataflow"];

// after support_for(capability):
if PAID_CAPABILITIES.contains(&capability) && !license.unlocks_deep() {
    status = CapabilitySupportStatus::LicenseRequired;
    reason = Some("Deep analysis requires a Pro license.".into());
    hint = Some("Set POLINT_LICENSE_KEY from your polint Pro subscription.".into());
    docs_path = Some("docs/facts/license.md".into());
}
```

### 6.3 Dependency / crates

Add to `crates/polint/Cargo.toml` (HMAC):

```toml
hmac = "0.12"
sha2 = "0.10"
base64 = "0.22"   # or use data-encoding already in tree if present
```

Prefer existing workspace deps if any already cover this; otherwise add narrowly.

---

## 7. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Paid rule runs with empty deep facts | `LicenseRequired` + `has_blocking_capability` skip; never Supported without license |
| Deep still runs “for free” because another rule requested symbols | Pipeline triggers are only the three paid caps — already true |
| Cache serves empty deep facts after purchase | Digest `license.deep` |
| HMAC secret extracted from binary | Accept for MVP; Ed25519 later; enterprise contracts for abuse |
| MIT crate still contains deep sources | Runtime gate for binary SKU; crate split only if publishing policy demands |
| `Events` confused with paid call graph | Keep free; docs + tests already prove semantic pipeline off |

---

## 8. Open questions

1. **Expiry UX:** hard fail after `exp`, or grace period + warn diagnostic?
2. **Team vs Pro:** same `deep` feature for both in MVP, or Team-only extras later?
3. **crates.io:** continue shipping full sources under MIT with runtime gate, or withhold deep sources?
4. Should `module_topology` (only used by deep pipeline today) be considered an internal paid provider only? (Yes — already gated by `run_cfg_call_pipeline`.)

---

## 9. Summary table (quick reference)

| Layer | Free | Paid |
|-------|------|------|
| Public capabilities | syntax, imports, metrics, tests, components/classes/literals/jsx, resolved_imports, module_graph, symbols, references, events, changeset | **calls, control_flow, dataflow** |
| Kernel flags | source, go/ts syntax, module_graph, symbol_graph, metrics | semantic_mir, cfg, calls, go.semantic, identity, domains, summaries, entrypoints, reachability, extensions, types, semantic_graph, solver, refined_calls, data_flow, evidence |
| `analysis/` tree | — (all deep) | entire `crates/polint/src/analysis/**` |
| Extra paid outside tree | — | `go/semantic/` |
| Unlock | — | `POLINT_LICENSE_KEY` HMAC → `features` includes `deep` |
| MVP mechanism | unchanged | `LicenseRequired` + kernel AND + digest bit |

**Next implementation step:** implement Week 1 Day 1–3 (`LicenseRequired` + `license.rs` + plan/kernel/runner wiring) behind a feature branch; do not start crate extraction until the runtime gate is green in CI.
