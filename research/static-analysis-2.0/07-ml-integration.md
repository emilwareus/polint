# 07 — ML Integration (verified, at the edges)

## Framing

No ML system in the 2020–2026 literature reduces the asymptotic complexity of
the symbolic solve, and every attempt to *replace* symbolic dataflow with
neural models remains benchmark imitation without soundness semantics. The
production pattern is unanimous (GitHub, Meta, Semgrep, Snyk): **detection
stays symbolic; ML sits at (1) offline spec/summary mining, (2) candidate
ranking with symbolic verification, (3) post-detection triage.** Amazon
CodeGuru — the one ML-detector product — was retired in 2025. What reduces
*effective* complexity is modular summaries + demand (docs 03/06); ML's job
is deciding where to spend precision and summarizing the long tail.

polint's position makes the choice easy: ~97% precision, recall-bound →
only recall-buying and cost-cutting ML is worth building.

## Build (ranked)

### 1. Verified neural type/callable-shape inference (biggest F1 lever)
Pattern: **JoernTI / CodeTIDAL5** (ESORICS 2023,
https://arxiv.org/abs/2310.00673,
https://github.com/joernio/joernti-codetidal5) — a ~220M CodeT5-based model
predicts types for usage slices from the code property graph, written back
into the graph; CPU/ONNX-servable. Gating pattern: **TypeWriter/HiTyper
propose-then-verify** (https://arxiv.org/abs/2105.03595) — the model only
*proposes*; predictions become edges only after checking against our
class-hierarchy/export facts. Decisive evidence for "types, not edges":
EMSE 2025 head-to-head (https://arxiv.org/abs/2410.00603) — LLMs *lose* to
Jelly/PyCG at direct CG extraction but *beat* static type inference.
Attacks the #1 documented FN root cause (dynamic property access /
ungrounded receivers, ECOOP'22 arXiv:2205.06780). Cost bounded by
#unresolved sites; cacheable. Caveat: TypyBench (arXiv:2507.22086) — LLM
types are locally good, globally inconsistent; we need receiver/callable
shape, an easier label space, and we verify.

### 2. Learned callee ranker, verify-then-accept
**GRAPHIA** (arXiv:2506.18191): link prediction over program graphs puts the
true target of statically-unresolved JS call sites at top-1 42% / top-5 72%.
It's a ranker with no soundness story — we supply the missing verification:
accept a ranked candidate only if it passes symbolic consistency (arity,
this-shape, export reachability). Start with a GBDT over cheap features
(name similarity, module distance, arity match, export structure) before any
GNN. Train on our own harness's dynamic traces. Each recovered keystone edge
compounds via summaries (the express effect).

### 3. LLM-synthesized package summaries (recall + memory at once)
Pattern: **IRIS** (ICLR 2025, https://arxiv.org/abs/2405.17238 — LLM infers
taint specs, CodeQL does the analysis; CWE detection 27→55 vs CodeQL alone)
and **AFD** (arXiv:2509.22530 — LLM classifies allocator functions feeding
pointer analysis; 1.4× overhead vs 10–15× for more context sensitivity;
majority voting suppresses hallucination). For us: where static
summarization of a dependency fails (dynamic packages), ask an LLM narrow,
verifiable questions — "does `router.use(f)` invoke `f`, with what args?" —
validate against `.d.ts`, store as a registry-ready summary payload in the local
semantic store (docs 03 and `research/local-semantic-store/`). A remote
registry can distribute those payloads later, but is not part of the first
implementation.
One-time cents per (package, version), amortized forever.

### 4. LLM triage on `polint review` findings (product-level, cheapest)
Pattern: **Semgrep Assistant Autotriage**
(https://semgrep.dev/blog/2025/building-an-appsec-ai-that-security-researchers-agree-with-96-of-the-time/)
— ~95–96% agreement, ~60% of triage volume handled; **LLift** (FSE 2024,
https://arxiv.org/abs/2308.00245) — LLM adjudicates static warnings with
task decomposition + self-validation. Per-diff finding counts are small, so
seconds-and-cents economics work. Reversible (suppression, not deletion).

### 5. (Later) Learned effort policies
**Graphick** (OOPSLA 2020, https://prl.korea.ac.kr/papers/oopsla20.pdf) and
data-driven context tunneling (OOPSLA 2018): cheap pre-analysis → learned
per-function policy for context/heap depth — competitive with expert
heuristics (Zipper/Scaler), scales where uniform 2obj times out. Attacks the
exponential term. Needs our own training corpus; do after docs 03/05/06.

## Do not build

- **ML callgraph pruning** (CGPruner TSE'22; AutoPruner FSE'22
  arXiv:2209.03230): buys precision we already have by spending recall we
  can't spare; MSR 2024 re-eval (arXiv:2402.07294) ≈ +25%P/−9%R on real
  programs; the field retreated to symbolic pruning (OriginPruner
  arXiv:2412.09110). One reusable insight: confidence-thresholded pruning as
  a *cost knob* (pruned CI graph ≈ 1-CFA quality, 69% smaller, 3.5× faster
  downstream) — only for non-gating consumers.
- **GNN/transformer replacement of symbolic dataflow** (ProGraML
  arXiv:2003.10536 → DFA-Net; GraphCodeBERT): imitation of analyses on
  benchmarks, degrades out-of-distribution, no soundness semantics, zero
  production adoption in six years.
- **Whole-repo LLM callgraph extraction**: loses to static tools on the
  exact task (EMSE 2025) at ~1000× cost.
- **Learned indexes/embeddings for demand queries**: no supporting
  literature; solve demand symbolically (doc 06), use retrieval only for
  candidate generation.

## Cross-cutting caveats

- **Ground truth is the swamp**: almost all CG-ML training data is dynamic
  traces under tests → models learn test-coverage bias. Applies to our
  harness too (doc 01) — we already saw unexercised true edges scored as FPs.
- **Cost asymmetry decides architecture**: GBDT/GNN rankers = µs–ms, inline;
  ~200M transformers = CPU-viable per unresolved slot (ONNX; Rust: `ort`);
  LLM APIs = seconds/cents — residues, offline synthesis, per-PR diffs only.
- **Generalization**: demonstrated within ecosystems, not across languages —
  assume per-ecosystem (re)training.

## Production reference points

GitHub Copilot Autofix (CodeQL detects; GPT-4o fixes — detection 100%
symbolic:
https://github.blog/news-insights/product-news/found-means-fixed-secure-code-more-than-three-times-faster-with-copilot-autofix/)
· Meta (Infer unchanged; LLMs for test generation — ACH:
https://engineering.fb.com/2025/02/05/security/revolutionizing-software-testing-llm-powered-bug-catchers-meta-ach/)
· Snyk DeepCode (ML mines rules offline; CodeReduce slicing for fix context,
arXiv:2402.13291) · Semgrep Assistant (triage) · Amazon CodeGuru (retired
2025:
https://docs.aws.amazon.com/codeguru/latest/reviewer-ug/codeguru-reviewer-availability-change.html)
· KNighter (SOSP 2025, arXiv:2503.09002 — LLM synthesizes checkers offline,
validated against originating patches; template for rules-as-code) ·
RepoAudit (ICML 2025, arXiv:2501.18160 — agentic auditing *consumes* a
CG/AST substrate; a future polint consumer, not a competitor).
