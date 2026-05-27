# Phase 25: Rule Manifest, Inspect, and Test Skeleton - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-05-18T17:43:12Z
**Phase:** 25-rule-manifest-inspect-and-test-skeleton
**Mode:** auto-selected recommended defaults
**Areas discussed:** Public surface boundary, Manifest generation, Inspect command behavior, Test runner, New rule and external consumer proof, Documentation and schema

---

## Public Surface Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Narrow public CLI surfaces | Promote only `polint inspect rule` and `polint test`; keep manifests as internal Rust data and make JSON the public contract. | yes |
| Broad agent inspection suite | Add facts, unknowns, provider, cache, and query inspection now. | no |
| Internal only | Keep all rule manifest and test runner behavior test-facing with no public CLI. | no |

**Auto choice:** Narrow public CLI surfaces.  
**Notes:** This matches Phase 25 scope and prior public API discipline. Broader inspect commands belong to later promotion phases.

---

## Manifest Generation

| Option | Description | Selected |
|--------|-------------|----------|
| Derive from existing macro/runtime truth | Use `RuleMeta`, typed fact-view parameters, generated capabilities, and resolved `RuleOptions`; reserve future fields without requiring them. | yes |
| Add a large metadata language now | Expand the macro with docs, tags, messages, fixability, limitations, and typed option schemas before inspect/test works. | no |
| Let users handwrite manifests | Ask rule authors to keep a manifest or capability declaration in sync manually. | no |

**Auto choice:** Derive from existing macro/runtime truth.  
**Notes:** This preserves the current analyzable rule shape and avoids reintroducing handwritten capability drift.

---

## Inspect Command Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Delegate to local rule hosts | Parent CLI discovers rule hosts and asks the child process that owns `Vec<Rule>` to emit manifests. | yes |
| Parent reads rule source | Parent scans `.polint/rules/src` or macro syntax directly. | no |
| Require full analysis | Run source parsing and rules before inspect can report manifests. | no |

**Auto choice:** Delegate to local rule hosts.  
**Notes:** This follows the existing `check` ownership model and lets inspect work before source analysis/setup succeeds.

---

## Test Runner

| Option | Description | Selected |
|--------|-------------|----------|
| Real temp-repo fixture runner | `polint test` creates temp repos from `.polint/tests`, runs the real check path, normalizes JSON, and asserts diagnostics. | yes |
| Cargo-test wrapper | Only run `cargo test` in `.polint/rules`. | no |
| Full mature harness immediately | Implement blessing, jobs, rich inline markers, multi-output snapshots, and all future authoring workflow features now. | no |

**Auto choice:** Real temp-repo fixture runner.  
**Notes:** The first version should prove public rule behavior through the same path users and agents will run, while deferring workflow polish that is not required for the skeleton.

---

## New Rule And External Consumer Proof

| Option | Description | Selected |
|--------|-------------|----------|
| Generated public-SDK fixtures | Keep `new-rule` macro/prelude based and add minimal fixture skeletons once test format exists; prove with temp-repo tests. | yes |
| Examples-only proof | Rely on checked-in workspace examples. | no |
| Internal helper proof | Use internal crate tests that bypass the local rule-host path. | no |

**Auto choice:** Generated public-SDK fixtures.  
**Notes:** Prior review identified workspace-coupled examples as insufficient. Phase 25 should prove outside-user behavior.

---

## Documentation And Schema

| Option | Description | Selected |
|--------|-------------|----------|
| Stable docs and schemas for promoted JSON | Document inspect/test behavior and add schemas where new stable JSON is exposed. | yes |
| Code-only surface | Ship commands without documenting the public contract. | no |
| Over-document future inspect APIs | Document future facts/unknowns/models/provider extensions before implementation. | no |

**Auto choice:** Stable docs and schemas for promoted JSON.  
**Notes:** Public CLI JSON is a product contract, so the docs must distinguish stable Phase 25 behavior from deferred future surfaces.

---

## the agent's Discretion

- Exact Rust module names and file layout.
- Exact manifest struct ownership as long as public imports stay narrow.
- Exact first `polint test` assertion format, provided it asserts normalized JSON diagnostics from temp repos.
- Exact split of plans across manifest, inspect, test runner, scaffolding, docs, and tests.

## Deferred Ideas

- Broad facts/unknowns/provider/cache/query inspection commands.
- Typed option schemas and rich message/fix descriptors.
- Model packs and provider extension manifests.
- Advanced query builders and public analysis graph views.
- `polint test --jobs`, `--bless`, and richer inline marker languages after the skeleton is stable.
