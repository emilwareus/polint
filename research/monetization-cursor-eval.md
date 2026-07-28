# Polint Monetization — Independent Deep Evaluation

**Author:** Cursor research pass (independent of `monetization-deep-eval.md`)  
**Date:** 2026-07-28  
**Scope:** Options A–E from `monetization-brief.md`  
**Constraint frame:** Solo founder, side-business, target **$5–20K MRR**, CLI-local engine (no server required), dashboard/governance via **local signal export** only.

---

## 1. Executive Summary

**Recommendation: Option A (Engine Tiers) as the paid product, with Option B as a local “trust report” conversion wedge — not as a SaaS.** Do not launch C, D, or E as the primary bet.

polint’s durable advantage is not “another SAST” and not “another AI governance dashboard.” It is a **local, hackable analysis kernel** (CFG, data flow, points-to, reachability, call graph) that rule authors and agents can write Rust against. That is rare. Semgrep’s commercial story is Pro Engine + Pro Rules + AppSec Platform (Series D ~$100M in Feb 2025, ~$204M total raised). SonarQube’s commercial story is open-core feature gating (Community → Developer). Neither ships “write repo-local Rust rules against a deep IR.” That gap is the monetizable surface.

**Why not B as primary:** The “AI agent governance” category is real and heating up (Endor Labs Agent Governance, Codacy Guardrails, Secure Code Warrior Trust Agent: AI, Gartner enterprise-agent market guides). Those products win on **inventory of agents/MCPs/skills**, commit attribution, and org dashboards. polint’s natural wedge is **code-quality evidence**, not agent fleet observability. A full trust SaaS needs a team. A local HTML/JSON trust feed does not — and that is enough for side-business packaging.

**Why not C as primary:** Reachability SCA is the highest willingness-to-pay category, and founder Debricked credibility is real. But the niche already has CLI-local specialists (**Coana at $30/contributor/mo**, offline analysis, call-graph reachability) plus Endor Labs (~$188M+ raised on reachability-first SCA) and Semgrep Supply Chain as a $30/contributor modular add-on. CVE/vuln-DB ops kill solo evenings. C is a **year-2 module on top of a paid deep engine**, not the go-to-market.

**Why not D:** Semgrep Pro Rules work because they ride **Pro Engine** (cross-file/cross-function). Rules alone are content; LLMs generate them; moat is near-zero for a solo author.

**Why not E:** Bundle vision is correct as a 24–36 month destination. As a year-1 launch it requires a team or VC. Sequence A → B-lite → C.

**12-month MRR ceiling (A + B-lite, solo, side hours):** **$6–12K** base case, **$15–18K** upside if conference/GitHub distribution converts hard and Team seats land. Hitting $20K sustainably likely needs either per-contributor pricing or a paid reachability add-on — still solo-viable, but not automatic.

**Critical packaging correction vs the brief:** Do not sell A as “Hackability as Product.” Engineers love that; buyers do not. Sell: **precise, local enforcement for AI-assisted codebases** — deep facts unlocked in Pro, free syntax forever.

---

## 2. Market Context (shared facts)

| Signal | Evidence | Confidence |
|--------|----------|------------|
| Dev-tool price anchor | Semgrep Teams ~$30/contributor/mo; CodeQL/GHAS Code Security ~$30/committer; DeepSource Team $24–30/contributor; Codacy Team ~$18–21/dev/mo; Coana Team $30/contributor | High |
| Free tier = table stakes | Semgrep free ≤10 contributors; Codacy free Developer + Guardrails IDE; DeepSource free Individual/OSS; Coana free for OSS | High |
| AI coding agents → governance demand | Gartner: enterprise AI coding agent market ~$9.8–11B annualized (Apr 2026 framing); Sonar 2026 survey cited by SCW: 72% daily AI coding tool use; Endor Labs shipping Agent Governance; Codacy marketing Guardrails; SCW Trust Agent: AI GA | High (demand), Medium (exact Gartner $) |
| Reachability SCA is crowded & funded | Endor Labs Core/Pro seat model (quote-only); Coana CLI-local reachability; Semgrep Supply Chain modular; Snyk reachability improving | High |
| Open-core works; relicensing whole trees is toxic | Sonar/Semgrep/GitLab open-core success; HashiCorp BSL → OpenTofu; Redis/Elastic license wars; 2025–26 commentary favors keep-core-OSS + proprietary enterprise layer | High |
| Side-business $5–20K MRR is achievable | Indie Hackers / micro-SaaS cohort routinely documents $5–20K with narrow tools + one channel; 40% never reach $1K (selection bias on survivors) | Medium |

**Implication for polint:** Price into the **$24–30** band psychologically, but prefer **flat org pricing** ($29 / $99) for solo ops unless seat counting is automated. Flat underprices 20-engineer teams vs Semgrep — that is acceptable for year-1 conversion; revisit seats when support load forces it.

---

## 3. Per-Option Scoring

Scoring scale: **1–10** on eight criteria. Higher is better. For **LLM risk**, higher = more resistant to commoditization.

| Criterion | Weight note |
|-----------|-------------|
| Market timing | Is budget opening now? |
| Competitive moat | Can a funded competitor erase you in 12 months? |
| Founder fit | Rust engine + Debricked + conference/GitHub |
| Time-to-revenue | Weeks to first paid invoice |
| Recurring revenue quality | Stickiness / expansion / churn |
| Solo viability | Evenings + weekends, no ops team |
| Distribution advantage | Existing audience conversion |
| LLM commoditization resistance | Can prompts replace the paid value? |

---

### Option A — Engine Tiers (Open-source syntax, sell deep analysis)

**Thesis:** Free = syntax facts + basic rules. Paid = call graph, data flow, points-to, reachability, deep signal views. Teams pay for precision they can enforce in CI and teach to agents via `polint add-skill`.

#### Competitor / analog evidence

| Analog | What they gate | Price | Lesson |
|--------|----------------|-------|--------|
| Semgrep CE vs AppSec Platform | Cross-file Pro Engine + Pro Rules + platform | Free (limits) → $30/contributor | Depth + rules sold together; engine alone is not the SKU — but depth is the unlock |
| SonarQube Community → Developer | Branch/PR analysis, security rules, languages | Free → commercial from ~$720/yr (LOC) | Open-core feature gating is buyer-understood |
| DeepSource | Full analysis cloud; self-host = Enterprise | $24–30/contributor | Local/self-host is an **upsell**, not free |
| CodeQL | Semantic depth in GHAS | ~$30/committer | Depth commands premium when packaged as security |

#### Evaluation

| Criterion | Score | Evidence-based rationale |
|-----------|------:|--------------------------|
| Market timing | **8** | AI agents increase need for **deterministic local gates** (not just LLM review). Budget exists in AppSec and platform eng. Category “custom policy engine” is smaller than SAST but growing with AI. |
| Competitive moat | **9** | 268K LOC Rust with constraint solvers / points-to is not a weekend clone. Rules written against polint facts create switching cost. Semgrep YAML rules are a different authoring model — complementary, not identical. |
| Founder fit | **10** | Founder *is* the engine. Conference talks and GitHub contrib rate are direct GTM for this SKU. |
| Time-to-revenue | **8** | Stripe + license key + CI Action unlock is weeks, not months — **if** licensing strategy is clean (see risks). |
| Recurring revenue | **8** | Sticky once rules depend on deep facts. Expansion via Team / later reachability. Flat $29 caps ARPU vs per-seat comps. |
| Solo viability | **9** | CLI + Action validation. No uptime pager. Matches “engine runs locally.” |
| Distribution | **8** | Existing audience maps to rule authors / platform eng, not CISOs. Enough for side-business; not enough for Semgrep-scale without content marketing. |
| LLM resistance | **8** | LLMs write rules; they do not replace incremental points-to in CI. Risk is “LLM review is good enough, skip static analysis” — mitigated by compliance/CI gate buyers. |

**Totals: 68/80**

**12-month MRR ceiling:** **$8–14K** (optimistic $16K with strong Team mix).  
Math (illustrative): 120×$29 Pro + 25×$99 Team ≈ $6K; 200×$29 + 40×$99 ≈ $9.7K; upside needs either more Teams or seat pricing.

**Worst case:** Community rejects gating; fork keeps deep analysis free; product remains strong OSS with $0 revenue. Downside is reputational, not technical.

**Solo / VC:** Solo-viable. No VC needed. **Do not** relicense the entire MIT tree to BSL — use proprietary deep modules or commercial binary distribution while keeping syntax OSS useful.

**Verdict:** **Primary bet.** Fix the narrative (AI-era precision enforcement), gate deep analysis honestly, ship fast.

---

### Option B — AI Agent Trust & Governance

**Thesis:** Prove agents are not degrading the codebase. Surface complexity trends, rule hits, fix rates, coverage gaps as a trust feed / dashboard.

#### Competitor evidence (researched 2026-07-28)

| Player | What they actually sell | Pricing | Overlap with polint B |
|--------|-------------------------|---------|------------------------|
| **Endor Labs — AI Coding Agent Governance** | Inventory agents/models/MCPs/skills; block shell/file/MCP/prompt actions; workstation + cloud agent visibility | Quote / seat SKU `EL-AGNT-GOV`; no public $ | **Low–medium.** They govern *agent behavior*, not code-quality trend proof. |
| **Codacy Guardrails** | Free IDE/MCP real-time scan of AI-generated code; org config + dashboards on Team/Business | Free Guardrails; Team ~$18–21/dev/mo | **Medium.** Closest “quality under AI” story; cloud-centric. |
| **Secure Code Warrior Trust Agent: AI** | Commit-level AI tool/model visibility, risk correlation, policy before merge | Enterprise (SCW platform) | **Medium.** Attribution + training; not deep static analysis. |
| **CodeRabbit** | AI PR review | ~$24–48/user/mo | Adjacent “AI code quality,” different mechanism. |

Market timing for “govern AI coding” is excellent. **Category definition matters:** most funded products are building **control planes for agents**. polint’s honest product is **measurement of code outcomes**. That is differentiated — and also harder to sell as “governance” without agent hooks.

Local constraint: trust product must be `polint report` / JSON export / static HTML from CI artifacts — **not** a hosted dashboard. That is fine for $29–99; it is weak against Codacy/Endor enterprise RFPs.

| Criterion | Score | Rationale |
|-----------|------:|-----------|
| Market timing | **9** | Hottest narrative in eng tools right now; budget opening in platform + AppSec. |
| Competitive moat | **5** | HTML trend report is copyable. Moat only if reports require deep engine facts (which folds B into A). Pure dashboard loses to Codacy/Endor. |
| Founder fit | **5** | Systems/security engineer, not org-dashboard / agent-inventory product. |
| Time-to-revenue | **5** | Local report: 4–8 weeks. True SaaS: 3–6 months + ops — **out of scope** for side business. |
| Recurring revenue | **7** | Orgs pay for ongoing assurance; churn if reports feel vanity-metric. |
| Solo viability | **4** | SaaS = no. Local report = yes. Brief’s “dashboard” language pulls toward the non-viable path. |
| Distribution | **7** | Conference story (“prove Cursor isn’t rotting your Go services”) is strong. |
| LLM resistance | **4** | LLMs will summarize git history into “trust reports.” Differentiation needs **non-LLM signals** from the engine. |

**Totals: 46/80**

**12-month MRR ceiling:** **$3–7K** standalone; **$0 incremental** if sold without A’s deep signals (buyers bounce). With A: trust report is a **feature**, not a SKU.

**Worst case:** Build SaaS, burn evenings on auth/billing/uptime, lose engine momentum.

**Solo / VC:** Full governance platform = **team + likely VC** (Endor raised $25M seed / $70M A / $93M B). Local trust export = solo.

**Verdict:** **Do not primary.** Ship as Pro feature (`polint report`) that *requires* deep signals — conversion narrative for A.

---

### Option C — Security Reachability (“Is this CVE actually reachable?”)

**Thesis:** CLI-first, local reachability over SBOM/CVE findings. Premium module.

#### Competitor evidence

| Player | Model | Price | Notes |
|--------|-------|-------|-------|
| **Coana** | CLI offline reachability SCA; dashboard for results | **$30/contributor/mo** (10–50); OSS free; Enterprise custom | Near-perfect product twin for “local CLI reachability.” Claims >80% FP discard. |
| **Endor Labs Open Source** | Function-level reachability, call graphs, containers | Quote / contributor-year | Category creator; heavily funded. |
| **Semgrep Supply Chain** | Reachability hints via SAST engine; direct deps stronger than transitive | **$30/contributor/mo** modular | Fast; known limits on transitive/indirect. |
| **Snyk** | Broad SCA + improving reachability | ~$25–105/dev depending on stack | Incumbent distribution. |

Founder Debricked exit is a **credibility accelerator**, not a substitute for a vulnerability intelligence pipeline.

| Criterion | Score | Rationale |
|-----------|------:|-----------|
| Market timing | **9** | Everyone drowning in CVE noise; reachability is the accepted fix. |
| Competitive moat | **7** | Deep IR helps accuracy vs lightweight pattern SCA — but Coana/Endor already specialize. Moat is analysis quality, not category ownership. |
| Founder fit | **9** | Best domain story of any option. |
| Time-to-revenue | **3** | Vuln DB, package→function mapping, ecosystem resolvers (Go modules, npm, etc.), triage UX. **6–12 months** of evenings before credible vs Coana. |
| Recurring revenue | **9** | Security renews; $49–149 pricing in brief is plausible; Coana proves $30/seat works. |
| Solo viability | **2** | Daily CVE firehose + multi-ecosystem packaging = ops job. Needs contractor/team or partner DB. |
| Distribution | **8** | Security audience + Debricked brand; conferences convert. |
| LLM resistance | **6** | LLMs hallucinate reachability; buyers still want deterministic graphs. Vuln content itself is DB ops, not LLM. |

**Totals: 53/80**

**12-month MRR ceiling if started now:** **$2–5K** (late year-1 launch, thin coverage).  
**If started after A is paying:** year-2 add-on could add **$5–10K** MRR without killing the side-business — still hard.

**Worst case:** Ship incomplete Go-only reachability; lose first security evaluations to Coana; reputation hit in the one community that remembers Debricked.

**Solo / VC:** Credible category leadership = **funded company**. Niche Go+TS reachability add-on for existing Pro users = **solo-possible later**.

**Verdict:** **Year-2 expansion on A**, not launch SKU. Do not compete head-on with Coana/Endor in year 1.

---

### Option D — Premium Rule Packs

**Thesis:** Engine free; sell curated security / framework / migration packs.

#### Competitor evidence

Semgrep sells **Pro Rules** (high-confidence, research-maintained, Pro-engine-aware) as part of paid platform — not as a standalone $29 content subscription for most buyers. Community rules remain free. Independent “rule pack marketplace” businesses without a proprietary engine have historically struggled (content is leakable; quality variance; continuous maintenance).

LLM reality (2026): generating a Semgrep- or polint-shaped rule from a prose policy is cheap. Value accrues to **engine precision + maintained regression suites**, not the rule text file.

| Criterion | Score | Rationale |
|-----------|------:|-----------|
| Market timing | **4** | Buyers pay for platforms; “rule packs” are a line item inside platforms. |
| Competitive moat | **2** | `.rs` / YAML rules copy instantly. DRM fails. |
| Founder fit | **5** | Can write excellent Go security rules; content treadmill is not where 268K LOC advantage lives. |
| Time-to-revenue | **7** | Fast to ship a pack; slow to get paid renewals. |
| Recurring revenue | **3** | High churn once copied or regenerated; constant shipping required. |
| Solo viability | **5** | Writing packs is solo-ok; supporting packs across frameworks is not. |
| Distribution | **6** | Free packs are great marketing for A. Paid packs fight OSS culture. |
| LLM resistance | **2** | Directly in the LLM blast radius. |

**Totals: 34/80**

**12-month MRR ceiling:** **$1–3K** (optimistic). Worse if packs are leaked.

**Worst case:** Community anger at gating “just rules”; forks redistribute packs.

**Solo / VC:** Solo-possible, wrong bet. Use free example packs to drive engine Pro.

**Verdict:** **Reject as monetization.** Keep rules as adoption fuel.

---

### Option E — Bundled Platform (A+B+D)

**Thesis:** One product — deep engine + trust + premium rules.

This is how Semgrep/Codacy/Endor *present* themselves after years and nine-figure capital. It is the correct **north star**, wrong **year-1 scope**.

| Criterion | Score | Rationale |
|-----------|------:|-----------|
| Market timing | **8** | Buyers prefer suites. |
| Competitive moat | **8** | Combined surface is strong — if built. |
| Founder fit | **4** | Stretches engine + UX + content. |
| Time-to-revenue | **2** | 6–12 months before a coherent paid suite. |
| Recurring revenue | **8** | Best ARPU if it works. |
| Solo viability | **1** | Needs team. Side-business death. |
| Distribution | **7** | Easier story than three SKUs. |
| LLM resistance | **7** | Depth + measurement resists commoditization. |

**Totals: 45/80**

**12-month MRR ceiling (solo attempt):** **$0–4K** (late, unfinished). With a 3–5 person team + funding: different game ($50K+ MRR possible, out of scope).

**Verdict:** **Sequence, don’t launch.** A → B-lite feature → optional D free packs → C module.

---

## 4. Comparative Matrix

| Criterion | A Engine | B Trust | C Reachability | D Rules | E Bundle |
|-----------|:--------:|:-------:|:--------------:|:-------:|:--------:|
| Market timing | 8 | **9** | **9** | 4 | 8 |
| Competitive moat | **9** | 5 | 7 | 2 | 8 |
| Founder fit | **10** | 5 | 9 | 5 | 4 |
| Time-to-revenue | **8** | 5 | 3 | 7 | 2 |
| Recurring revenue | 8 | 7 | **9** | 3 | 8 |
| Solo viability | **9** | 4 | 2 | 5 | 1 |
| Distribution | **8** | 7 | 8 | 6 | 7 |
| LLM resistance | **8** | 4 | 6 | 2 | 7 |
| **Total /80** | **68** | **46** | **53** | **34** | **45** |
| 12-mo MRR ceiling | **$8–14K** | $3–7K | $2–5K | $1–3K | $0–4K |
| Needs team/VC? | No | SaaS yes / local no | Yes for category win | No | **Yes** |
| Fits local-CLI constraint? | **Yes** | Only as export | **Yes** | Yes | Partial |

**Ranking for this founder + constraints:** A ≫ C > B ≈ E ≫ D  
**Ranking if “hire 5, raise seed”:** E/C-led suite (different company).

---

## 5. Recommendation Detail

### What to sell

1. **Free (MIT or AGPL core):** File discovery, parsing, syntax facts, `#[polint::rule]` SDK, CI Action, agent skill generation, basic signals. Genuinely useful — Sonar/Semgrep lesson: free tier must not be a toy.
2. **Pro (~$29/mo flat or $24–29/contributor if you automate seats):** Deep analysis modules (call graph, CFG, data flow, points-to, reachability primitives, advanced signal views). License unlock in CLI/Action.
3. **Team (~$99/mo or volume seats):** Shared private rule packs hosting optional; org license; priority support; SSO later if demanded — don’t build SSO in month 1.
4. **Pro includes:** `polint report` — static HTML/JSON trust feed from CI history (Option B-lite). No server.

### What not to sell in year 1

- Hosted governance dashboard  
- Standalone premium rule marketplace  
- Full CVE reachability product competing with Coana  
- “Platform” messaging that implies Semgrep-complete suite  

### Licensing (non-negotiable strategy note)

Relicensing an already-public MIT tree to BSL is a known community landmine (OpenTofu, Valkey patterns). Prefer:

- **Open core:** syntax/SDK stay OSS; deep analyzers ship as proprietary crates or feature-gated binaries, **or**
- **Dual license** on new deep modules only, **or**
- Commercial support + Pro binary builds while source of deep modules remains private  

Document the choice before first Stripe charge. This is the #1 existential risk for Option A.

### Pricing vs market

| Brief price | Market reality | Suggestion |
|-------------|----------------|------------|
| Pro $29/mo flat | Comps are ~$24–30/**seat** | Flat OK for side business year 1; expect ARPU envy; add contributor pricing when >15-seat orgs appear |
| Team $99/mo | Cheap for 10+ eng teams | Keep as SMB convenience; don’t fight enterprise RFPs |
| C $49–149 | Coana $30/seat validates security premium | Use later as add-on ≥$20/seat or +$49 flat |

---

## 6. 12-Month Roadmap (Winner: A + B-lite)

Assumes side hours (~10–15/wk), no full-time switch, no VC.

### Months 0–1 — Monetization plumbing

- Decide license architecture (OSS core + proprietary deep).
- Stripe Checkout: Pro / Team.
- License key issuance; Action + CLI validation; offline grace for air-gapped CI.
- Public pricing page; clear free vs Pro capability matrix (honest limits).
- Ship one **killer Pro demo rule** that is impossible on syntax-only (e.g. cross-function taint or call-graph policy on Go).

**Exit:** First paying customer possible. Target: soft launch to existing audience.

### Months 2–3 — Wedge narrative + trust export

- `polint report`: HTML + JSON from local/CI artifacts (complexity, rule hits, severity trends).
- Wire report richness to Pro deep signals (free tier = snapshot, Pro = trends).
- Content: 2–3 conference/blog pieces — “Local proof your AI agents aren’t rotting the repo.”
- Free community rule examples (marketing, not SKU).

**Exit:** Target **$1–2K MRR**.

### Months 4–6 — Depth that sells renewals

- Harden Go+TS deep pipelines for Pro reliability (panic→diagnostics already in ethos).
- Cache/digest correctness for paid features (no “Pro but flaky”).
- GitHub Action UX: one-liner Pro unlock.
- Optional: annual billing discount (indie-standard ~15–20%).

**Exit:** Target **$3–5K MRR**. Support load still email-only.

### Months 7–9 — Distribution concentration

- Pick **one** channel and go deep: conference circuit **or** technical SEO/content **or** curated outbound to Go platform teams — not all three.
- Case study from 2–3 design partners (even unpaid logos).
- Consider seat-based Team plan if flat $99 is leaving money on table.

**Exit:** Target **$5–8K MRR**.

### Months 10–12 — Decide C vs double-down A

- If Pro retention >70% and support is manageable: spike **narrow** reachability (Go modules first, curated CVE subset, “experimental”) as Pro/Team add-on — not a Coana clone.
- If retention weak: invest in authoring UX and agent skill quality, not security DB.
- Explicit non-goal: hosted multi-tenant dashboard.

**Exit:** Target **$8–12K MRR** base; **$15K+** only with strong Team/seat mix or early reachability add-on.

### Revenue checkpoints (planning, not promises)

| Month | MRR | Paying orgs (approx) |
|------:|----:|----------------------|
| 3 | $1–2K | 30–50 Pro |
| 6 | $3–5K | 80–120 Pro + some Team |
| 12 | $8–12K | 150–250 Pro + 20–40 Team |

If month 6 < $1K with real distribution attempts, revisit packaging (seat pricing, security add-on spike) before assuming “engine tiers don’t sell.”

---

## 7. Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **OSS relicensing backlash / fork of deep analysis** | Medium | High | Never BSL the whole MIT history; gate new proprietary modules; keep free tier genuinely good |
| **“Hackability” doesn’t convert** | Medium | Medium | Sell AI-era precision + CI gates; lead with one dramatic Pro-only finding |
| **Flat $29 ARPU caps at <$10K** | High | Medium | Introduce contributor pricing for Team when needed; add C later |
| **Semgrep/Sonar “good enough”** | Medium | Medium | Don’t fight broad SAST; win **repo-local Rust policies** + local-only posture |
| **Coana/Endor own reachability mindshare** | High (if C early) | High | Defer C; partner or subset later |
| **Trust SaaS distraction** | Medium | High | HTML/JSON only; no auth service |
| **Solo burnout on 268K LOC + commercial** | High | High | Sequence; freeze languages; no E |
| **License key piracy** | Medium | Low | Honest teams pay; don’t over-engineer DRM |
| **LLM code review replaces static gates** | Low–Medium | Medium | Position as deterministic CI evidence for auditors/agents; not “smarter than GPT” |
| **Buyer is CISO, founder sells to eng** | Medium | Medium | Start with platform/staff eng; security add-on later for CISO budget |
| **Side-business time starvation** | High | High | Roadmap assumes evenings; slip dates beat scope creep |

### Explicit “needs a company” boundary

| Ambition | Vehicle |
|----------|---------|
| $5–20K MRR, local CLI, deep engine Pro | **Solo / side business** ← recommended |
| Category-leading AI agent governance SaaS | Team + VC (Endor/Codacy shape) |
| Category-leading reachability SCA | Team + vuln intel ops + funding (Endor/Coana shape) |
| Semgrep-like suite | VC-scale GTM |

---

## 8. Sources & Access Notes

Primary / vendor pages consulted (access date **2026-07-28** unless noted):

- Semgrep pricing / Pro rules / CE vs Platform docs and 2026 buyer comparisons (Konvu, Safeguard, Augment, DEV community pricing writeups)
- Semgrep Series D PR (PR Newswire, Feb 5, 2025) — $100M, ~$204M total
- Endor Labs: `/pricing`, `/ai-coding-agent-governance`, docs licenses (`EL-AGNT-GOV`), AppSec Santa 2026 review
- Codacy Guardrails + 2026 pricing summaries (~$18–21/dev Team)
- Secure Code Warrior Trust Agent: AI product + Mar 2026 press
- Coana: coana.tech pricing ($30/contributor), product (CLI offline reachability)
- SonarQube edition docs / comparison (Community vs Developer open-core)
- DeepSource billing docs ($24–30/contributor Team; self-host Enterprise)
- Gartner enterprise AI coding agent market article (2026)
- Open-core / BSL analyses (OSSAlt 2026 guide; OpenTofu/Valkey postmortems)

**Confidence labels:** Pricing list prices High where vendor-published; third-party ARR splits for Semgrep Pro Rules (BCG-template sites) treated as **Low** and not used for decisions. Market-size CAGR reports treated as **directional only**.

---

## 9. Bottom Line

| Question | Answer |
|----------|--------|
| What should Emil sell? | **Deep analysis engine tiers (A)** |
| What makes people care now? | **AI-assisted repos need local, precise, hackable gates** + `polint report` proof |
| What must he not build? | Hosted governance SaaS; premium-rules business; year-1 Coana competitor |
| Can a solo founder hit $5–20K MRR? | **$5–12K: yes with execution. $20K: possible but requires seat pricing and/or reachability add-on — still no VC if scoped tightly** |
| Is this a VC company? | **Not if he stays on A+B-lite.** Yes if he chases Endor/Semgrep surface area. |

**Ship A. Package B as a local report. Park C. Give D away. Never launch E as a big bang.**
