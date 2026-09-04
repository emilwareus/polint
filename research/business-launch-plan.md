# Polint Business & Launch Plan

**Author:** Research synthesis from monetization deep evaluations  
**Date:** 2026-07-28  
**Status:** Actionable plan for solo founder side business  
**Target:** $5–20K MRR within 12 months  
**Decision:** Option A (Engine Tiers) as primary monetization, with Option B-lite (`polint report`) as Pro feature

---

## Table of Contents

1. [Product Positioning & Narrative](#1-product-positioning--narrative)
2. [Pricing & Packaging](#2-pricing--packaging)
3. [Go-to-Market Strategy](#3-go-to-market-strategy)
4. [12-Month Revenue Targets & Monthly Milestones](#4-12-month-revenue-targets--monthly-milestones)
5. [Customer Acquisition Strategy](#5-customer-acquisition-strategy)
6. [Competitive Positioning](#6-competitive-positioning)
7. [Key Risks & Mitigations](#7-key-risks--mitigations)
8. [Launch Checklist](#8-launch-checklist)
9. [Appendices](#9-appendices)

---

## 1. Product Positioning & Narrative

### Core Positioning

**Polint is precise, local enforcement for AI-assisted codebases.**

Polint is not a general-purpose linter, not a SaaS platform, and not an AI code reviewer. It is a local, hackable static-analysis engine that lets engineering teams write Rust rules against deep program facts — call graphs, data flow, points-to analysis, and reachability — and run them deterministically in CI. In a world where AI coding agents generate most new code, polint is the proof that your policies are actually being enforced.

### The Three-Sentence Pitch

AI coding agents write code fast. Your team needs to know it's *correct* code — and that takes deep analysis that syntax checkers can't do. Polint gives you a local, hackable engine with call graphs, data flow, and reachability that you can enforce in CI and teach to your agents. Free for syntax rules. **$29/mo for precision.**

### Narrative by Buyer Persona

| Persona | Pain Point | Polint Story |
|---------|-----------|--------------|
| **Platform Engineer / Staff Engineer** | "Cursor and Copilot are generating code faster than we can review. Our lint rules catch syntax but miss architectural violations." | "Polint's deep engine computes call graphs and data flow locally. Write a Rust rule once — it runs in CI, and your AI agents get `polint add-skill` feedback. No server. No vendor lock-in. Your policies, enforced deterministically." |
| **Security-aware Tech Lead** | "We need security gates in CI, but Semgrep's YAML rules can't express our cross-function authorization pattern." | "Polint's SDK lets you write Rust rules against a full points-to graph. Free tier catches syntax issues. Pro unlocks the deep engine — call graphs, reachability, taint tracking. Locally. In CI. No cloud." |
| **Go/TypeScript Team Lead** | "Our linter works on single files. We need project-wide invariants — no circular deps between packages, no forbidden call chains." | "Polint computes call graphs across your entire Go or TypeScript project. Write the check once in Rust, run it on every commit. Pro gives you the deep analysis engine that makes cross-file rules possible." |
| **OSS Maintainer** | "I want contributors to follow my repo's conventions, but I can't enforce them with ESLint/golangci-lint." | "Polint's free tier gives you a framework for repo-local rules. Write your policy once in Rust, run it in CI. The syntax engine is and stays free." |

### Anti-Narrative (What Polint Is NOT)

- **NOT** "Semgrep but in Rust" — Semgrep is a mature SAST platform with cloud dashboards, Pro Rules, and SCA. Polint is a local engine with a different authoring model (Rust rules vs YAML patterns).
- **NOT** "Another AI code reviewer" — Polint runs deterministic analysis. No LLM hallucination risk.
- **NOT** "CVE reachability product" (year 1) — That's year 2+, and only as a module on the deep engine.
- **NOT** "Enterprise SaaS platform" — Polint is CLI-first. No hosted dashboard. No auth service.

### Brand Voice

- **Honest and technical.** No hype. If a rule is heuristic, say so. If analysis is limited to Go + TS, say so.
- **Practical.** "Here's a real problem. Here's the polint rule that catches it. Here's the CI output."
- **Solo-founder genuine.** Emil's voice: systems engineer, security background, builds things that work locally.

---

## 2. Pricing & Packaging

### Tier Structure

| Tier | Price | What's Included | License Model |
|------|-------|----------------|---------------|
| **Free** | $0 | Syntax analysis (Go + TypeScript), `#[polint::rule]` SDK, basic rule execution, CI Action, `polint add-skill` agent feedback, current-state snapshot in `polint report`, all MIT-licensed core | MIT open source |
| **Pro** | **$29/mo** flat | Everything in Free + **deep analysis engine unlocked**: call graph, control-flow graph, data-flow analysis, points-to analysis, reachability analysis, constraint solvers, MIR pipeline, 30+ analysis modules, historical trends in `polint report` (HTML/JSON trust feed from CI artifacts), all signal views with Pro depth | Proprietary binary / license key |
| **Team** | **$99/mo** flat | Everything in Pro + shared team rule configuration, priority email support, annual billing option (20% discount = ~$950/yr), SSO (after month 6, if demanded) | Proprietary + team license key |

### Pricing Rationale

**Why flat pricing instead of per-seat?**

Per-seat pricing ($24–30/contributor/mo, matching Semgrep/Codacy/Coana) would maximize revenue for teams of 15+ engineers. But seat counting requires either a SaaS dashboard or self-reported honesty in a license key — both add friction for a solo founder. Flat pricing is:

1. **Simple to implement** — one Stripe checkout, one license key, zero seat-counting logic.
2. **Excellent for solo devs and small teams** — your first 50 customers are individuals and 3–8 person teams. $29 flat beats $30/seat × 4 engineers = $120/mo.
3. **Honest** — no "trust us to count seats" ambiguity. One team, one key.

**When to add per-seat pricing:** When teams of 20+ engineers start buying Team at $99 flat and you're leaving $500+/mo on the table per org. This is a good problem to have — revisit at month 9.

**Annual discount:** Standard indie SaaS practice. $29/mo = $348/yr; annual = $278/yr (20% off). Team: $99/mo = $1,188/yr; annual = $950/yr. Annual reduces churn and improves cash-flow predictability.

### Free Tier Design Principles

The free tier must be **genuinely useful** — not a crippled demo. Semgrep's Community Edition lesson: if the free tier is a toy, nobody adopts it, and nobody upgrades. Polint's free tier:

- Ships all syntax-level analysis for Go and TypeScript.
- Includes the full `#[polint::rule]` SDK — rule authors can build and test rules without paying.
- Runs in CI with the GitHub Action.
- Generates `polint add-skill` output for AI coding agents.
- Shows **current-state** signals in `polint report` (snapshot, not trends).

**The upgrade trigger:** When a team writes a rule that needs cross-file analysis (call graphs, data flow) and hits the "Pro required for deep analysis" message in CI, the value of Pro is self-evident. They've already written the rule. They just need the engine that makes it work.

### What NOT to Monetize (Year 1)

| Don't Sell | Why |
|------------|-----|
| Premium rule packs | LLMs generate rules. Content is trivially copied. Rules are marketing, not revenue. |
| CVE reachability | Requires vulnerability DB infrastructure. 6–12 months of evenings. Year-2 module on Pro. |
| Hosted SaaS dashboard | Needs auth, billing, uptime, GDPR. Kills solo viability. `polint report` is local HTML. |
| Custom enterprise contracts | Distraction at $5–20K MRR. Self-serve only until month 12. |

---

## 3. Go-to-Market Strategy

### Channel Strategy: Concentrate on One

The #1 GTM mistake for solo founders is spreading across 5 channels. Pick **one** channel and go deep. Based on Emil's assets, the sequence is:

**Primary channel (months 0–6): Existing audience + conference talks**

Emil has: 8.4K GitHub contributions/yr, conference speaker history, Debricked exit credibility, a network of senior engineers who know his work. This is not a cold-start problem. The first 50 paying users come from people who already trust Emil's technical judgment.

**Secondary channel (months 4–12): Technical content / SEO**

After the product is stable and the narrative is proven with real users, invest in:
- Blog posts showing "here's a real bug, here's the polint rule that catches it, here's the CI output"
- "Polint vs X" comparison pages (Semgrep, golangci-lint, custom ESLint rules)
- Documentation that ranks for long-tail queries ("Go call graph static analysis CLI", "TypeScript data flow analysis local")

### Launch Sequence

#### Phase 0: Pre-Launch (Weeks 1–4)

| Week | Action | Owner |
|------|--------|-------|
| 1 | Finalize license architecture decision. Document: OSS core (MIT) + proprietary deep crates. | Emil |
| 1–2 | Set up Stripe: Pro ($29/mo) and Team ($99/mo) products. Stripe Checkout or Payment Links — simplest integration. | Emil |
| 2–3 | Build license key issuance: Stripe webhook → generate HMAC-signed key → email to customer. Key file: `polint.license` dropped in repo root or `~/.polint/`. | Emil |
| 2–3 | Build license validation: CLI checks key on startup; GitHub Action checks key from `POLINT_LICENSE_KEY` secret; graceful offline fallback (72-hour grace). | Emil |
| 3 | Ship one **killer Pro-only demo rule**: e.g., "forbidden cross-package call chain" in Go — a rule that is impossible without the call graph. | Emil |
| 3 | Write public pricing page. Clear free vs Pro comparison table. Honest about what's "coming soon." | Emil |
| 4 | Soft launch to 20–50 existing contacts. Email / DM / Discord. "I built a thing. Free tier is solid. Pro unlocks the deep engine. Would love your feedback." | Emil |

#### Phase 1: Public Launch (Month 1)

| Action | Channel | Expected Reach |
|--------|---------|---------------|
| Launch blog post: "Polint: Precise Local Enforcement for AI-Assisted Codebases" | Personal blog + dev.to cross-post | 5K–15K views |
| Hacker News "Show HN" | HN | 10K–50K views if front page; 1K–3K if not |
| Twitter/X thread: technical walkthrough of the killer Pro rule | @emilwareus | 10K–50K impressions (existing following) |
| r/golang and r/typescript posts (not self-promo — "here's how I caught this bug with custom static analysis") | Reddit | Variable; 5K–20K views each if well-received |
| GitHub repo README updated with clear "Free vs Pro" section | github.com/emilwareus/polint | Organic discovery |

#### Phase 2: Conference Push (Months 2–6)

| Conference | Timing | Talk Angle |
|------------|--------|------------|
| GopherCon / Go meetups | Q3–Q4 2026 | "Writing Go Call-Graph Policies in Rust: What polint's Deep Engine Unlocks" |
| TypeScript Congress / TSConf | Q3–Q4 2026 | "Beyond ESLint: Cross-File TypeScript Enforcement with a Local Static Analysis Engine" |
| Local meetups (Go, Rust, platform eng) | Ongoing | Live demo: write a rule, catch a real bug, show CI output |
| Online: RustConf, Strange Loop (if accepted) | Q4 2026 | "Building a 268K LOC Static Analysis Engine as a Side Project — and Making It Pay" |

Each talk ends with: "Free tier at github.com/emilwareus/polint. Pro is $29/mo if you need deep analysis. I answer support emails personally."

#### Phase 3: Content Engine (Months 4–12)

| Content Type | Frequency | Goal |
|-------------|-----------|------|
| "Rule of the Week" blog posts | Weekly | SEO for long-tail queries; show polint's power in small, digestible pieces |
| "Polint vs [Tool]" comparison pages | Monthly | Capture comparison-search traffic (high intent) |
| Case studies from paying users | As acquired | Social proof; "X team uses polint to enforce Y policy" |
| YouTube: 5-minute rule walkthroughs | Bi-weekly | Visual learners; embed in docs |

### Distribution Assets (What Exists)

| Asset | Strength | How to Leverage |
|-------|----------|----------------|
| 8.4K GitHub contributions/yr | Signals sustained OSS credibility | GitHub profile links to polint; contributions keep repo active in feed |
| Conference speaker | Warm audience, trust | Every talk = funnel. Live demos convert better than blog posts. |
| Debricked exit | Security credibility | Mention in bio, not in pitch. "Previously: built Debricked (acquired). Now: polint." |
| Existing senior engineer network | High-intent early adopters | Personal outreach for first 20–50 users. "Would you try this and tell me what breaks?" |

---

## 4. 12-Month Revenue Targets & Monthly Milestones

### Revenue Model Assumptions

- **Pro conversion rate from free users:** 2–4% (standard dev-tool freemium range)
- **Team conversion rate from Pro users:** 8–12%
- **Monthly churn:** 5–8% (higher early, stabilizing at 5% by month 6)
- **Annual plan mix:** 20% of Pro, 30% of Team by month 6
- **Pricing:** $29/mo Pro flat, $99/mo Team flat (annual: ~$278/yr Pro, ~$950/yr Team)

### Month-by-Month Targets

| Month | Free Users (cumulative) | Pro Subscribers | Team Subscribers | MRR | Notes |
|-------|------------------------|-----------------|------------------|-----|-------|
| **1** | 200–500 | 5–10 | 0–2 | **$150–500** | Soft launch to existing contacts. Revenue is a signal of willingness-to-pay, not a target. |
| **2** | 500–1,200 | 15–30 | 2–5 | **$600–1,400** | Public launch. HN/Reddit push. First conference talk. |
| **3** | 1,200–2,500 | 30–50 | 4–8 | **$1,300–2,200** | **Target: $1K MRR.** Conference content circulating. First case study in progress. |
| **4** | 2,500–4,000 | 45–70 | 6–12 | **$1,900–3,200** | Content engine starting. "Rule of the Week" live. |
| **5** | 4,000–6,000 | 60–90 | 10–16 | **$2,700–4,200** | Second conference talk. Annual plans starting to convert. |
| **6** | 6,000–8,000 | 80–120 | 14–22 | **$3,700–5,700** | **Target: $5K MRR.** Six-month mark. Evaluate pricing (flat vs per-seat). |
| **7** | 8,000–10,000 | 100–140 | 16–26 | **$4,500–6,600** | Polished onboarding flow. Reduce churn to <6%. |
| **8** | 10,000–12,500 | 120–160 | 20–30 | **$5,500–7,600** | Content engine at full speed. Comparison pages ranking. |
| **9** | 12,500–15,000 | 140–180 | 24–34 | **$6,400–8,600** | **Revisit per-seat pricing** if Team flat is leaving significant money. |
| **10** | 15,000–18,000 | 160–200 | 28–38 | **$7,400–9,600** | Third major conference talk. Case studies published. |
| **11** | 18,000–20,000 | 180–220 | 30–42 | **$8,200–10,800** | Spike narrow reachability add-on (Go modules only, experimental). |
| **12** | 20,000–25,000 | 200–260 | 34–48 | **$9,200–12,300** | **Target: $10K MRR base; $15K+ optimistic with Team/seat mix.** |

### Revenue Scenarios

| Scenario | Month 6 MRR | Month 12 MRR | Key Assumptions |
|----------|------------|-------------|-----------------|
| **Conservative** | $2.5K | $6K | Lower conversion (1.5%), higher churn (8%), fewer free users (6K by month 6) |
| **Base case** | $5K | $10K | 3% conversion, 5% churn, 8K free users by month 6 |
| **Optimistic** | $8K | $18K | 4% conversion, 4% churn, 12K free users, strong Team mix, early reachability add-on |

### What To Do If Targets Are Missed

| If by Month 6 MRR is... | Then... |
|--------------------------|---------|
| **<$1K** | Fundamental packaging problem. Revisit: (a) switch to per-seat pricing, (b) spike security reachability add-on, (c) pivot narrative to "compliance gate for AI code." Do not double down on the same approach. |
| **$1–3K** | Growth path is real but slow. Focus on: (a) one distribution channel, (b) reduce churn with onboarding improvements, (c) ask 10 users what would make them upgrade. |
| **$3–5K** | On track. Keep going. |
| **>$5K** | Exceeding plan. Consider: (a) annual-only pricing for new signups, (b) investing more evenings, (c) hiring a contractor for content/support. |

---

## 5. Customer Acquisition Strategy

### Where Do the First 50 Paying Users Come From?

This is a **warm-audience problem**, not a cold-acquisition problem. Emil has the network. The question is conversion, not awareness.

| Source | Estimated Users | Conversion Path | Timeline |
|--------|----------------|-----------------|----------|
| **Direct outreach to existing contacts** | 10–15 | Personal email/DM: "Built something. Free tier is solid. Pro unlocks deep analysis. Try it, tell me what's broken." | Month 1 |
| **Conference talk attendees** | 10–20 | Talk → live demo → "Free at github.com/..." → follow-up email to attendees. One talk = 50–200 attendees, 5–10% try, 10–20% of trials convert. | Months 2–6 |
| **GitHub organic (stars → visitors → trials)** | 5–10 | Stars on polint repo → profile link → docs → "Free vs Pro" page → Stripe. 2K stars might yield 50–100 trial signups, 5–10% convert. | Months 2–6 |
| **Hacker News / Reddit launch** | 5–10 | Show HN → traffic spike → docs → trials. 50K views = ~2K docs visitors = ~100 trials = 5–10 paid. | Month 2 |
| **Content marketing (SEO)** | 5–10 | "Polint vs Semgrep" "Go call graph CLI" — high-intent search → docs → trials. Builds over months 4–12. | Months 4–12 |
| **Word of mouth from early users** | 5–10 | "My team uses polint for our Go call-graph policy." Mentioned in 1:1s, team chats, internal RFCs. Unpredictable but real. | Months 3–12 |
| **GitHub Action marketplace** | 2–5 | `polint-action` listed → discovered by teams setting up CI → docs → trials. | Months 3–6 |

### Acquisition Funnel (Target by Month 6)

```
Free tier downloads/installs:    8,000
Active free users (ran in CI):   2,000 (25%)
Visit pricing page:                400 (20% of active)
Start Pro trial:                   150 (37% of pricing visitors)
Convert to paid Pro:                90 (60% of trials)
Upgrade to Team:                    14 (15% of Pro)
─────────────────────────────────────────
Total paid subscribers:            104
MRR: ~$4,700 (90 × $29 + 14 × $99)
```

### Tactical Acquisition Plays

**Week 1–2: Personal Outreach Template**

```
Subject: polint — local static analysis engine, would love your eyes

Hey [Name],

I've been building polint over evenings/weekends — a Rust static analysis
engine that lets you write repo-local rules against call graphs, data flow,
and reachability. Think: "golangci-lint but you write Rust rules against
deep program facts."

Free tier (syntax analysis) is MIT and live now. Pro ($29/mo) unlocks the
deep engine. I'm looking for 10-20 people to kick the tires before I do a
public launch.

Would you try running it on [their repo] and tell me what breaks?

github.com/emilwareus/polint

— Emil
```

**Month 2: Show HN Post (Draft Angle)**

> **Show HN: Polint — Write Rust rules against call graphs, data flow, and reachability (local, CLI)**
>
> I built polint because my team kept finding architectural violations that linters missed — forbidden cross-package call chains, missing authorization checks on specific data paths, circular dependencies between packages. ESLint and golangci-lint can't see across files. Semgrep's YAML rules can't express the invariants we needed.
>
> Polint is a Rust framework for repo-local static analysis. Free tier: syntax rules for Go and TypeScript. Pro tier ($29/mo): deep analysis engine — call graphs, data flow, points-to, reachability. Everything runs locally. No server.
>
> [Link to demo rule that catches a real-world Go anti-pattern using the call graph]

**Month 4+: "Rule of the Week" Content Strategy**

Every week, publish a 400–800 word post:
1. **Real-world problem** (from personal experience or user report)
2. **The polint rule** that catches it (10–30 lines of Rust)
3. **CI output** showing the violation
4. **"Free tier or Pro?"** callout — honest about what engine features the rule needs

Example titles:
- "Catching Forbidden Cross-Package Imports in Go with a 20-line polint Rule"
- "TypeScript Data Flow: Detecting Unsanitized User Input Across Files"
- "Enforcing Your Team's 'No Circular Dependencies' Policy in CI"

---
## 6. Competitive Positioning

### Competitive Landscape

| Competitor | Price | Model | What They Do Well | Polint's Differentiator |
|------------|-------|-------|-------------------|------------------------|
| **Semgrep** | $30/contributor/mo | Open-core SAST platform, cloud dashboard, Pro Rules, SCA | Mature product, huge rule library, multi-language, $200M+ funded | Polint: local-only (no cloud dependency), Rust rule authoring (Turing-complete vs YAML patterns), deep IR (points-to, data flow vs pattern matching) |
| **Codacy** | $15–28/dev/mo | Cloud code quality platform, "AI Guardrails" for agent governance | Dashboard, integrations, breadth of checks | Polint: no server, write your own deep rules, deterministic local enforcement |
| **CodeQL (GitHub)** | ~$30/committer (GHAS) | Semantic analysis, security-first, CI-integrated | Deep analysis, Microsoft-backed, huge query library | Polint: open core, local-only (no GHAS dependency), Rust SDK vs QL |
| **Coana** | $30/contributor/mo | CLI reachability SCA, offline analysis | Security credibility, CVE reachability | Polint: broader than SCA (rules, not just reachability), authorable by users, hackable |
| **Endor Labs** | Custom/quote | AI Agent Governance + reachability SCA, $188M+ raised | Category-defining for agent governance, enterprise-ready | Polint: $29 flat vs enterprise contracts, local-only, self-serve, solo-founder authentic |
| **SonarQube** | Free → $720/yr (on-prem) | Open-core code quality, branch/PR analysis | Trusted brand, broad language support, on-prem option | Polint: Rust-native performance, hackable SDK, no server requirement at all |
| **golangci-lint** | Free | Go linter aggregator | Fast, standard for Go projects | Polint: cross-file, project-wide analysis; write your own rules in Rust, not YAML/Go |
| **ESLint / Biome** | Free | JS/TS linting | Standard for JS ecosystem | Polint: cross-file data flow, type-aware rules, not a replacement — a different category |

### Competitive Moat

1. **Engine depth (268K LOC of Rust).** Building a working call-graph, points-to, and data-flow engine for multiple languages is not a weekend clone. The constraint solvers, MIR pipeline, and incremental analysis kernel represent years of evenings.

2. **Rule lock-in.** A rule written against polint's deep IR (call graphs, data flow facts) cannot be trivially ported to Semgrep's YAML patterns or ESLint's AST visitors. Teams that invest in polint rules build switching costs.

3. **Local-only posture.** Polint does not send your code anywhere. In an era of "AI reads my code" anxiety, "your code never leaves your machine" is a genuine differentiator against cloud-first competitors.

4. **Rust SDK.** Writing rules in Rust (rather than YAML, Go templates, or custom DSLs) attracts the exact audience Emil already has: senior systems/platform engineers who value performance and type safety.

5. **AI agent integration.** `polint add-skill` generates agent-consumable policy files from polint rules. Competitors focus on *reviewing* AI output; polint *teaches* AI agents your policies.

### Positioning Map

```
                    Cloud SaaS
                        |
         Codacy o      |      o Semgrep
         Endor Labs o  |      o SonarQube Cloud
                        |
    Generalist ---------+--------- Specialist
    (broad checks)      |      (deep analysis)
                        |
         ESLint o       |      o polint
         Biome o        |      o Coana
         golangci-lint o|
                        |
                    Local CLI
```

Polint occupies: **Local CLI × Specialist (deep analysis)** — the quadrant with no well-funded incumbents because cloud SaaS generates more revenue per seat. This is a feature, not a bug: polint's local-only posture is a differentiator for teams that can't or won't send code to cloud analyzers.

### How To Win Deals (Objection Handling)

| Objection | Response |
|-----------|----------|
| "We already use Semgrep" | "Polint doesn't replace Semgrep. It handles the rules Semgrep can't express — cross-file invariants, architectural policies, your team's specific conventions." |
| "Why not just write a bash script?" | "If your policy is 'no TODOs in production code,' bash is fine. If it's 'no call from package A to package B unless through package C,' you need a call graph. That's polint." |
| "We don't have Rust developers" | "You don't need Rust to use polint. The free tier runs rules from the community. Pro unlocks the engine for your team's rules. Writing rules requires Rust — but one platform engineer can write rules the whole team runs." |
| "What about [language we use that polint doesn't support]?" | "Go and TypeScript today. Python is next. The adapter contract is documented — here's how to add a language if you're feeling ambitious." |
| "It's a solo founder. What if you stop maintaining it?" | "Polint is open core. The syntax engine is MIT and always will be. The deep engine is proprietary. If I get hit by a bus, you still have the MIT core and can run your existing rules." |
| "How is this better than just asking the AI to check?" | "AI code review hallucinates. It says 'looks good' on code it doesn't understand. Polint runs deterministic analysis. If it says 'pass,' the rule was checked." |

---
## 7. Key Risks & Mitigations

### Risk Matrix

| # | Risk | Likelihood | Impact | Mitigation | Trigger to Act |
|---|------|-----------|--------|------------|----------------|
| **R1** | **OSS relicensing backlash** — community rejects gating previously-free deep analysis, forks the engine | Medium | High | Never BSL the existing MIT code. Gate **new** proprietary modules only. Keep free tier genuinely useful — if the syntax engine is good, the community doesn't need to fork. Publish a "Why Pro?" post explaining the economics honestly. | Negative HN/Reddit thread with >100 comments. Fork appears on GitHub. |
| **R2** | **"Hackability" doesn't convert** — engineers love the concept but won't pay | Medium | Medium | Lead with concrete value, not philosophy. The killer Pro rule must make the upgrade self-evident: "This rule catches real bugs. It needs Pro. $29/mo." If "hackability" sells poorly, pivot narrative to "precision enforcement" or "compliance gate for AI code." | Month 3 MRR <$1K with >1K free users. |
| **R3** | **Flat $29 ARPU caps MRR** — even 300 Pro users only = $8.7K MRR | High | Medium | This is the expected constraint of flat pricing. Mitigations: (1) Introduce per-seat Team pricing by month 9 if >15-seat orgs are buying, (2) Add reachability add-on ($20/seat or $49 flat) as year-2 module, (3) Push annual plans for cash-flow stability. This is not a crisis — it's the tradeoff for solo viability. | Month 9 MRR is growing but per-customer revenue is flat. |
| **R4** | **Semgrep/Sonar "good enough"** — buyers satisfied with existing tools, no budget for polint | Medium | Medium | Don't fight broad SAST. Win narrow: "Your team has one architectural invariant no existing tool can enforce. Polint can. Here's the rule." The wedge is custom, project-specific policies — not replacing Semgrep. | Free-tier signups high, Pro conversion <1%. |
| **R5** | **LLMs replace static analysis** — teams decide LLM code review is sufficient | Low–Medium | Medium | Position polint as **deterministic** evidence, not "smarter than AI." Compliance teams, auditors, and regulated industries need reproducible gates. "The LLM said it reviewed the code" is not a policy you can enforce. | Industry shift toward LLM-only quality gates. Track via Gartner/analyst reports. |
| **R6** | **Solo burnout** — 268K LOC engine + commercial operations is unsustainable | High | High | This plan is designed for ~10–15 hours/week. Sequence, don't bundle. Freeze language support at Go + TS until revenue justifies expansion. No SaaS ops. Automate: Stripe handles billing; Action handles license validation. If burnout looms: cut scope, not quality. | Working >20 hours/week regularly. Dreading opening the codebase. |
| **R7** | **License key piracy** — keys shared on forums, teams using one Pro key for 20 engineers | Medium | Low | Honest teams pay. License validation in GitHub Actions (one key per repo, visible in CI config) makes sharing obvious to any security reviewer. Don't build DRM. Don't phone home. If a team of 20 pirates a $29 key: they weren't going to pay $29 × 20 anyway — and they're evangelizing polint internally. | Evidence of systematic piracy at scale. |
| **R8** | **Time starvation** — side-business squeezed by day job + life | High | High | Roadmap designed for evenings/weekends. If progress stalls: (1) reduce scope before extending timeline, (2) consider 1–2 weeks of dedicated "polint vacation" for major milestones, (3) assess whether $5K MRR justifies part-time contractor for support/content. | Missing 2+ consecutive monthly milestones. |
| **R9** | **Wrong license architecture** — BSL or AGPL choice causes irreversible community damage | Low | Catastrophic | Decide once, document publicly, never change. Recommendation: MIT for syntax core (already shipped), proprietary for deep analysis modules. Do not relicense existing MIT code. | N/A — this is a pre-launch decision. |
| **R10** | **Competitor ships "write your own rules"** — Semgrep or CodeQL adds a Rust SDK or deep IR | Low | Medium | Polint's moat is execution quality + community, not concept. If a competitor ships a Rust SDK for deep analysis, that validates the market and Emil's thesis. Compete on: local-only, honest solo-founder, genuinely open core, performance. | Semgrep announces Rust SDK for custom rules. |

### Three Existential Risks (Prevent At All Costs)

1. **BSL/AGPL relicensing of the existing MIT code.** The fastest way to destroy community trust. Document the license decision publicly before charging a single dollar.

2. **Building a SaaS dashboard.** Auth, billing, uptime, GDPR, data storage. Each is a part-time job. Each takes Emil away from the engine. `polint report` is local HTML. That's the product.

3. **Chasing the wrong buyer.** Emil's audience is platform engineers and tech leads. If revenue lags, the temptation is to pivot to "security platform for CISOs." That audience requires: sales calls, SOC 2, RFPs, dedicated support. Do not do this as a solo founder.

---
## 8. Launch Checklist

### Pre-Launch (Complete Before First Public Charge)

- [ ] **License architecture decision** — Document in `LICENSING.md`: MIT for syntax core / SDK, proprietary for deep analysis crates. Rationale published.
- [ ] **Stripe setup** — Stripe account (sole proprietor / LLC). Products: Pro ($29/mo), Team ($99/mo). Annual options: Pro ($278/yr), Team ($950/yr). Payment Links or simple Checkout.
- [ ] **License key issuance** — Stripe webhook → HMAC-signed license key → automated email. Key format: `polint_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.pro` or `.team`.
- [ ] **CLI license validation** — `POLINT_LICENSE_KEY` env var or `polint.license` file. Graceful error: "Deep analysis requires Pro. Free tier rules still run. Get a license at polint.dev/pricing"
- [ ] **GitHub Action license support** — `polint-action` reads `POLINT_LICENSE_KEY` from repo secrets. CI output: "Pro engine enabled" or "Free tier (deep analysis skipped)."
- [ ] **Offline grace period** — 72-hour grace for air-gapped CI. Warn on day 1, error on day 4. Honest teams don't abuse this.
- [ ] **Pricing page** — Public page at polint.dev/pricing (or GitHub Pages). Feature comparison table. Honest "coming soon" markers. No dark patterns.
- [ ] **Free vs Pro capability matrix** — What the syntax engine can do vs what deep analysis unlocks. Specific, not marketing-speak. Example: "Call graph: Free ✗ / Pro ✓"
- [ ] **Killer Pro demo** — One compelling rule that uses deep analysis and catches a real, recognizable bug. This is the conversion demo.
- [ ] **Documentation for upgrade path** — "How to upgrade to Pro" docs. `polint license activate <key>`. What happens to existing rules.
- [ ] **Support email** — Single email address. You answer personally. State response time (48h for Pro, 24h for Team).
- [ ] **Privacy/telemetry decision** — Document what polint sends home (ideally: nothing). If you add anonymous usage stats for Pro, make it opt-in and transparent.
- [ ] **Terms of Service** — Simple, honest. "You pay. We provide the engine. No refunds on monthly, prorated on annual. You own your rules. We don't see your code."
- [ ] **GitHub repo README update** — Add "Free vs Pro" section. Link to pricing page. Honest: "I'm a solo dev. Pro pays for continued development."

### Launch Week

- [ ] **Soft launch to 20–50 contacts** — Personal emails/DMs. "Try this. Tell me what breaks. Free tier is solid, Pro if you need deep analysis."
- [ ] **Launch blog post** — Personal blog + dev.to cross-post. Technical story, not marketing. Real code, real output.
- [ ] **Show HN post** — Draft reviewed by 2–3 trusted contacts before posting. Respond to every comment.
- [ ] **Twitter/X thread** — Technical walkthrough of the killer Pro rule. "Here's a bug. Here's the polint rule. Here's the CI output."
- [ ] **Monitor Stripe** — First charge = celebrate. It proves someone values the engine.
- [ ] **Monitor GitHub** — Stars, issues, PRs. Respond within 24h for the first month. Set the tone.

### Month 1 (Post-Launch Stabilization)

- [ ] **First 10 paying users** — Reach out personally. "Thanks for being an early supporter. What's working? What's broken?"
- [ ] **Fix top 3 issues** — Whatever the first 10 users report. Fast iteration builds trust.
- [ ] **Ship one new Pro-only analysis module** — Show momentum. "Pro now includes taint tracking for Go HTTP handlers."
- [ ] **Write "month one retro" blog post** — Honest numbers: free users, Pro subscribers, MRR, what broke, what's next. This builds the kind of audience that converts.

### Months 2–3 (Growth Foundation)

- [ ] **First conference talk submitted and delivered** — Local meetup or regional conference. Live demo.
- [ ] **`polint report` v1 shipped** — HTML/JSON trust feed from CI artifacts. Pro: historical trends. Free: current snapshot.
- [ ] **"Rule of the Week" content series started** — 4–8 posts in the pipeline before starting.
- [ ] **Annual billing live** — Stripe already set up. Enable in checkout.
- [ ] **Onboarding flow improved** — Based on first-user feedback. Reduce time-to-first-meaningful-rule to <10 minutes.
- [ ] **First case study in progress** — Identify one team using polint in production. Document their rule, their workflow, their results.

### Months 4–6 (Scale)

- [ ] **Second conference talk** — Bigger stage. "What We Learned from 100 polint Rule Authors."
- [ ] **Comparison pages live** — "Polint vs Semgrep," "Polint vs golangci-lint," "Polint vs custom ESLint rules." Honest, not hit pieces.
- [ ] **First case study published** — Real team, real rules, real results. "How [Company] enforces 14 architectural invariants with polint."
- [ ] **Community contributions** — At least 3 community-authored rules in the examples directory. Signal: people are invested.
- [ ] **Churn analysis** — Why are people canceling? Fix the top reason. If it's "didn't use it enough," improve onboarding and rule discoverability.
- [ ] **Revenue review** — Is flat pricing working? Are Team orgs >15 engineers buying at $99? If yes, consider per-seat Team pricing.

### Months 7–12 (Optimize)

- [ ] **Third major conference talk** — National/international stage if possible.
- [ ] **Support load assessment** — If support >3 hours/week, streamline docs or consider auto-responder for FAQs.
- [ ] **Reachability spike (Go modules)** — Experimental. "polint reachability" add-on. Curated CVE set, Go only. Not a Coana competitor. See if existing Pro users pay extra.
- [ ] **Per-seat pricing decision** — By month 9: stay flat or introduce per-seat Team tier.
- [ ] **Year-1 retrospective published** — Honest MRR, lessons learned, what's next. This builds the narrative for year 2.
- [ ] **Year-2 roadmap decided** — Based on revenue, support load, and burnout level: double down on engine tiers, build narrow reachability, or keep steady.

---

## 9. Appendices

### Appendix A: License Key Implementation Notes

```
Key format: polint_<32-char-base64>.<tier>
Example:    polint_a3f8c2d1e4b5a6f7c8d9e0f1a2b3c4d5.pro

Validation:
  1. CLI reads POLINT_LICENSE_KEY env var or polint.license file
  2. HMAC-SHA256 signature verified against server secret
  3. Tier extracted from suffix (.pro or .team)
  4. Expiry checked (for annual plans)
  5. If valid: deep engine enabled
     If invalid/expired: warning, deep engine disabled, free tier runs
     If missing: free tier only (no warning — free tier is intentional)

GitHub Action:
  - polint-action reads POLINT_LICENSE_KEY from repo secrets
  - Same validation as CLI
  - CI output: "Pro engine: enabled" or "Pro engine: not licensed"

Offline grace:
  - Key validated once on first run
  - Cached locally with timestamp
  - If internet unavailable: 72h grace from last validation
  - After 72h: warning on day 1-3, error on day 4+
```

### Appendix B: Stripe Product Configuration

```
Product: Polint Pro
  Price: $29.00 USD/month
  Price (annual): $278.00 USD/year ($23.17/mo — 20% discount)
  Metadata: tier=pro, billing=recurring

Product: Polint Team
  Price: $99.00 USD/month
  Price (annual): $950.00 USD/year ($79.17/mo — 20% discount)
  Metadata: tier=team, billing=recurring, seats=unlimited

Webhook events to handle:
  - checkout.session.completed → generate license key, email to customer
  - customer.subscription.deleted → revoke license key (optional; honest-teams model)
  - invoice.payment_failed → email customer, 7-day grace before key revoked
```

### Appendix C: First 12 Months Financial Projection (Base Case)

| Month | Free Users | Pro (monthly) | Pro (annual) | Team (monthly) | Team (annual) | Gross MRR | Cumulative Revenue |
|-------|-----------|---------------|--------------|----------------|---------------|-----------|-------------------|
| 1 | 350 | 5 | 0 | 1 | 0 | $244 | $244 |
| 2 | 850 | 16 | 3 | 3 | 0 | $983 | $1,227 |
| 3 | 1,800 | 32 | 6 | 6 | 1 | $2,209 | $3,436 |
| 4 | 3,200 | 48 | 10 | 9 | 2 | $3,487 | $6,923 |
| 5 | 5,000 | 64 | 14 | 12 | 3 | $4,792 | $11,715 |
| 6 | 7,000 | 80 | 18 | 16 | 4 | $6,306 | $17,921 |
| 7 | 9,000 | 96 | 22 | 18 | 5 | $7,553 | $25,474 |
| 8 | 11,000 | 112 | 26 | 22 | 6 | $8,770 | $34,244 |
| 9 | 13,000 | 128 | 30 | 26 | 7 | $9,667 | $43,911 |
| 10 | 15,500 | 144 | 34 | 30 | 8 | $10,865 | $54,776 |
| 11 | 18,000 | 160 | 38 | 34 | 9 | $12,263 | $67,039 |
| 12 | 20,000 | 176 | 42 | 38 | 10 | $13,700 | $80,739 |

*Notes: Pro annual = $23.17/mo equivalent. Team annual = $79.17/mo equivalent. Churn netted into subscriber counts (5% monthly churn assumed, offset by new signups). All figures approximate — plan for variance of ±30%.*

### Appendix D: Personal Monthly Time Budget

| Activity | Hours/Month | Notes |
|----------|------------|-------|
| Engine development (Pro features, bug fixes) | 20–25 | Core investment. Ship ~2 significant improvements/month. |
| License/commercial infrastructure | 5–8 | Month 1 heavy (20h). Month 2+: maintenance only. |
| Content marketing (blog, docs, comparison pages) | 8–12 | "Rule of the Week" + one longer piece. |
| Community/support (GitHub issues, email) | 5–8 | Respond within 48h. Direct personally. |
| Conference talks + travel | 4–8 (amortized) | 3–4 talks/year. Prep + delivery + travel. |
| Admin (Stripe, taxes, legal) | 2–3 | Monthly. Set up automations early. |
| **Total** | **~44–64** | **~11–16 hours/week.** Sustainable for evenings + weekends. |

### Appendix E: What Success Looks Like at Month 12

- **$10K+ MRR** from 200+ Pro and 40+ Team subscribers.
- **Self-sustaining engine development.** Revenue covers all costs (Stripe fees, domain, occasional contractor). Remaining revenue is profit.
- **3+ published case studies** from real teams using polint in production CI.
- **Conference circuit established.** Emil is known as "the polint guy" in Go and TypeScript communities.
- **No SaaS infrastructure.** Still CLI-only. Still local. That's the product.
- **Option value preserved.** Can: (a) keep as lifestyle business at $10–15K MRR, (b) add reachability module for +$3–5K MRR, (c) raise small seed if category proves larger than expected, or (d) sell the business.
- **No burnout.** Evenings and weekends, but sustainable. The engine is fun to work on. Support is manageable.
