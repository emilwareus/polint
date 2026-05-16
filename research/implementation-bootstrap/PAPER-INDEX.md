# Paper And Source Index

This implementation-bootstrap track primarily depends on local source-code
inspection and previous research. The external sources here are official Rust
guidance and local research material used to judge Rust architecture choices.

## Official Rust Sources

File: Rust API Guidelines checklist
Source URL: <https://rust-lang.github.io/api-guidelines/checklist.html>
Publisher / project: Rust API Guidelines project
Authors / responsible organization: Rust project community
Publication or revision date: living document
Access date: 2026-05-16
Source type: official/community Rust API guidance
Local file: not downloaded
Status: summarized
Short note: Used for API boundary discipline, type interoperability, error
types, trait design, and documentation expectations.

File: The Rust Programming Language, Generics
Source URL: <https://doc.rust-lang.org/book/ch10-01-syntax.html>
Publisher / project: The Rust Project
Authors / responsible organization: Rust project
Publication or revision date: living book
Access date: 2026-05-16
Source type: official docs
Local file: not downloaded
Status: summarized
Short note: Used for monomorphization/static-dispatch reasoning.

File: Clippy documentation
Source URL: <https://doc.rust-lang.org/clippy/>
Publisher / project: The Rust Project
Authors / responsible organization: Rust project
Publication or revision date: living docs
Access date: 2026-05-16
Source type: official docs
Local file: not downloaded
Status: summarized
Short note: Used to validate the repository's lint posture and future semantic
kernel lint expectations.

## Local Rust Skill Sources

File: `.agents/skills/rust-best-practices/SKILL.md`
Source URL: local repository skill
Publisher / project: Apollo GraphQL Rust best-practices-derived skill
Authors / responsible organization: local skill metadata credits Apollo GraphQL
Publication or revision date: version 1.1.0 in local skill metadata
Access date: 2026-05-16
Source type: local skill
Local file: `.agents/skills/rust-best-practices/SKILL.md` and
`.agents/skills/rust-best-practices/references/chapter_*.md`
Status: read and applied
Short note: Applied for borrowing/cloning, `Copy` IDs, error handling,
dispatch/generics, typestate, comments/docs, pointer/thread-safety, testing, and
linting.

## Prior Research Tracks Consumed

| Folder | Role in this bootstrap |
| --- | --- |
| `research/analysis-kernel/` | Fact layers, scheduling, provenance, validation, extension merges, cache keys. |
| `research/evaluation-harness/` | Fixtures, benchmark gates, default-vs-extension metrics. |
| `research/semantic-index/` | Stable semantic identity, scopes, references, unresolved facts. |
| `research/call-graphs/` | Call-site/call-edge tiers and unresolved call facts. |
| `research/data-flow/` | Data-flow dependency on CFG, calls, summaries, and source/sink models. |
| `research/cfg-control-flow/` | CFG/operation-node requirements for MIR. |
| `research/type-alias-points-to/` | Place, type, value, points-to, and alias substrate. |
| `research/effects-summaries/` | Summary keys, summary algebra, SCC/fixpoint, cache implications. |
| `research/abstract-interpretation/` | P0 domain ordering, lattice law tests, reduced products, widening policy. |
| `research/agent-extension-surface/` | Rust-code model extension lifecycle, validation, and provenance. |
