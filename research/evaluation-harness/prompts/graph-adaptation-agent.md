# Graph Adaptation Agent Prompt

You are adapting polint graph analysis to one benchmark case repository.

Objective: improve observed call-graph edges for the target repository by adding
repo-local models or extension facts that describe real project behavior. Do not
change the benchmark oracle, test expectations, or scanner core unless the
benchmark runner reports a scanner failure unrelated to adaptation.

Hard rules:
- Do not inspect expected edge labels, oracle JSON arrays, or benchmark `WANT`
  comments for the case you are adapting.
- Do inspect source files, dependency manifests, existing polint call facts,
  unresolved call facts, unsupported call facts, and accepted/rejected extension
  facts.
- Use the polint skill and public/repo-local rule-authoring workflows only.
- Add only repo-local model or extension files under the prepared benchmark
  workspace. Do not edit vendored benchmark source unless the case itself is
  intentionally a scratch copy created by the harness.
- Keep every model fact traceable to source evidence: callsite location, callee
  identity, and the reason the scanner could not derive it natively.
- Prefer precise direct call models over broad wildcard models.
- If an edge needs RTA/VTA, reflection, dynamic property resolution, framework
  dispatch, or package setup that polint does not support yet, record it as an
  unsupported limitation instead of guessing.

Workflow:
1. Run the baseline graph benchmark for the assigned case and collect:
   unresolved call count, unsupported call count, graph true positives, false
   positives, false negatives, and runtime.
2. Inspect polint callsite/refined-call output and source around unresolved or
   unsupported calls.
3. Add repo-local model or extension facts for call edges that are source-evident
   without reading the oracle.
4. Re-run the benchmark and compare before/after metrics.
5. Report prompt hash, changed files, before/after unknown count,
   accepted/rejected extension facts, precision/recall delta, runtime/cache
   delta, and remaining unsupported categories.

First target order:
1. Jelly JS/TS micro cases, because source-location identities make adapted
   deltas easy to verify.
2. Go x/tools RTA cases, after direct/static setup is healthy; dynamic RTA-only
   edges should remain explicit limitations until a Go call provider supports
   them.
