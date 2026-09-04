# Polint Monetization — Strategic Options Deep Evaluation

## Context

polint is a Rust static-analysis platform (268K LOC). Multi-language (Go via tree-sitter, TS/JS via Oxc). Deep analysis engine: CFG, data flow, points-to, reachability, call graph, slicing, MIR, constraint solvers. Incremental analysis kernel. Rule SDK with #[polint::rule] macros. CI integration via GitHub Action. AI agent feedback via polint add-skill. 20+ computed signal views. MIT license, no monetization. Core engine runs locally (CLI), no server.

Founder: Emil Wareus, solo dev, side business target $5-20K MRR. Rust/Go strong, Debricked exit 2022, 8.4K GitHub contrib/yr, conference speaker.

## Strategic Options

### Option A: Hackability as Product (Engine Tiers)
Open-source syntax engine. Sell deep analysis (call graph, data flow, points-to, reachability). Teams pay for precision. "Write your own code quality enforcement in Rust."

Pricing: Free (syntax) / $29/mo Pro (deep engine) / $99/mo Team (shared rules, SSO)

### Option B: AI Agent Trust & Governance
Teams using AI coding agents need proof agents aren't degrading the codebase. polint computes signals — complexity trends, rule hit rates, fix rates, coverage gaps. Surface as "trust feed" dashboard.

Pricing: Free (rules only) / $29/mo Pro (trust feed) / $99/mo Team (org-wide)

### Option C: Security Reachability
"Is this CVE actually reachable from my code?" CLI-first, locally-running. Pairs with polint's deep analysis.

Pricing: Free (syntax) / $49/mo Pro (reachability) / $149/mo Enterprise (SBOM)

### Option D: Premium Rule Packs
Engine is free OSS. Revenue from curated rule packs — security, framework, migration.

Pricing: Free (engine) / $29/mo (premium packs) / Custom (enterprise)

### Option E: Bundled Platform
Combine A+B+D — engine depth, trust signals, premium rules as one product.

Pricing: Free (syntax+basic signals) / $29/mo Pro (deep+trust+rules) / $99/mo Team

## Competitive Reference

- Semgrep: $30/contributor/mo. OSS SAST + Pro Rules + SCA. ~$100M+ ARR.
- Codacy: $15-28/dev/mo. "AI Guardrails" for AI agent governance.
- Endor Labs: Custom. "AI Coding Agent Governance" standalone product.
- CodeRabbit: $24-48/user/mo. AI code review.
- DeepSource: $24/user/mo. Static analysis.
- Snyk: $25-105/dev/mo. Security platform.
- Price anchor: $24-30/seat/mo. Free tiers table stakes.

## Evaluation Criteria

For each option evaluate: market size/timing, competitive moat, founder fit, time-to-revenue, recurring revenue, solo viability, distribution advantage, LLM commoditization risk, 12-month MRR ceiling, worst case.

## Task

Research each option. Look at actual competitors, pricing, market trends, founder stories. Think critically about what a solo dev can achieve vs. team-required.

Do your own research — don't just summarize this doc. Return full analysis with recommendation.
