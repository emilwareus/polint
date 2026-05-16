# Process Notes

No subagents were used for this specific research pass because the latest
request did not explicitly ask for delegated agents, and the current operating
rules only allow spawning subagents when the user explicitly asks for
delegation or parallel agent work in that turn.

The research was still performed as a multi-pass local review:

1. Collected and downloaded relevant research papers.
2. Cloned state-of-the-art OSS implementations into the ignored
   `research/incremental-query-engine/repos/` directory.
3. Inspected implementation code and official docs for each tool family.
4. Extracted common algorithms: snapshots, shape digests, reverse dependencies,
   red-green verification, equality pruning, semi-naive deltas, differential
   traces, and incremental iterative data-flow.
5. Compared those algorithms against polint's product-specific requirements:
   multi-language facts, Rust-native core, agent-authored extensions,
   provenance, validation, precision, and public SDK discipline.
6. Wrote a conservative native implementation path with explicit revisit
   criteria for Salsa, relation engines, and daemon mode.

## Secondary Review Findings

The first tempting design was "just use Salsa." The secondary review rejected
that as premature because the hard part for polint is not only query
memoization. The hard part is stable layer keys, lifecycle inputs, extension
digests, validation status, provenance, and persistent batch cache manifests.

The second tempting design was "make Datalog the core." The secondary review
rejected that as the top-level engine because relation engines do not naturally
own source snapshots, rule options, extension validation, diagnostics, and
official tool invocation boundaries. A relation/fixpoint backend still makes
sense later for recursive families.

The third tempting design was "module-level invalidation is enough." The
secondary review rejected that as a long-term answer because polint will need
cheap one-off answers for aliases, summaries, evidence paths, source-to-sink
queries, and rule-specific views.

The final recommendation is a layered native incremental substrate first, then
a demand query engine, then summary SCC caching, then daemon red-green mode and
relation/differential backends only where benchmarks justify them.
