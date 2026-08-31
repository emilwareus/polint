---
quick_id: 260830-nzd
status: in_progress
description: Implement and A/B benchmark the Polint 10x algorithmic scan speedups from the completed research, retaining only byte-identical measured wins.
---

# Quick Task 260830-nzd Plan

1. Implement H1 indexed span conversion with compatibility tests, run the full Rust gates and six-cell benchmark, verify byte-identical JSON, then retain and publish only a measured win.
2. Iterate through H2, H4/H8, H3, and the post-profile backlog as separate measured waves; re-profile when the hotspot could have moved, split combined hypotheses unless each has attributable evidence, and revert/log every non-win.
3. Run the final full benchmark matrix, finish the mission log and GSD record, update the PR with baseline/wave/final tables and the cut/revert list, and leave the branch pushed and ready for review without merging.
