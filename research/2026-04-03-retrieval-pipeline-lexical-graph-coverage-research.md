# Research: Retrieval Pipeline (Lexical + Graph Expansion + Coverage Audit)

Status: draft
Date: 2026-04-03
Owner: RepoBrain OS

## Research Question

What is the most defensible V1 retrieval pipeline that combines lexical indexing, bounded structural expansion, and deterministic coverage auditing while respecting latency-first constraints?

## Candidate Options

1. SQLite FTS5 lexical index + bounded graph expansion + coverage audit
2. Embedded Rust search index (Tantivy or similar) + bounded graph expansion + coverage audit
3. Ripgrep-style on-demand search + bounded graph expansion + coverage audit (no persistent index)

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| SQLite FTS5 | local-first, low-friction, trigram support, simple persistence | new dependency + index maintenance | FTS tuning needed for code-heavy tokens | low-medium | Recommended for v1 |
| Tantivy | faster and richer search features | heavier integration, larger footprint | more moving parts for local-first | medium-high | Later candidate |
| On-demand rg | zero index maintenance | slower hot-path, inconsistent latency | unacceptable for IDE latency | low | Not acceptable for v1 |

## Sources

- Primary:
  - https://www.sqlite.org/fts5.html
  - https://arxiv.org/abs/2403.10059 (Repoformer: selective retrieval)
  - https://aclanthology.org/anthology-files/pdf/emnlp/2025.emnlp-main.1187.pdf (CodeRAG)
  - https://arxiv.org/abs/2512.12117 (Citation-Grounded Code Comprehension)
  - https://arxiv.org/abs/2509.25257 (RANGER)
  - https://aclanthology.org/2025.naacl-long.7.pdf (CodexGraph)
  - https://arxiv.org/abs/2505.24715 (CoRet)
- Secondary:
  - RepoBrain SDDs: `docs/sdd-004-retrieval-and-grounding-pipeline.md`
  - RepoBrain SDDs: `docs/sdd-005-latency-first-serving-strategy.md`

## Recommendation

Adopt SQLite FTS5 as the V1 lexical plane with trigram tokenization for code and `unicode61` for prose, then expand structurally from top lexical anchors under strict hop/candidate budgets, and gate completeness with deterministic coverage audits plus a single targeted second pass.

## Direct Evidence vs Inference

- Direct:
  - Multi-path retrieval improves repository task coverage compared to single-path retrieval (CodeRAG).
  - Graph-aware retrieval improves cross-file evidence and citation accuracy (Citation-Grounded).
  - Retrieval should be selective and task-aware to avoid wasted cost (Repoformer).
  - Graph-oriented retrieval is a proven structure for repo-scale questions (RANGER, CodexGraph).
  - SQLite FTS5 provides trigram tokenization and BM25 ranking suitable for local-first indexing.
- Inferred:
  - SQLite FTS5 is the best V1 tradeoff for local-first latency without a heavier index dependency.
  - A bounded second pass is sufficient for V1 coverage healing under latency budgets.

## Unknowns / Follow-Ups

- Evaluate FTS5 vs Tantivy on large repos for latency/recall tradeoffs.
- Measure candidate explosion under structural expansion at 10x repo scale.
- Decide whether a dedicated decision-document index is needed for `decision_why` queries.

## Engineering Impact

- Contract / type impact:
  - no schema change required for v1 retrieval; use internal retrieval types
- Testing impact:
  - new unit tests for lexical retrieval, bounded expansion, and coverage audit
- Runtime / latency impact:
  - hot-path latency must remain within `interactive` budgets
- Workload assumptions:
  - interactive requests are dominated by lexical lookups plus bounded graph traversal
- Dominant operations and cost centers:
  - lexical token lookup, adjacency expansion, slot audit
- Candidate data structures / indexes:
  - SQLite FTS5 for lexical index, adjacency maps for imports/reverse imports
- Main rejected alternative and why:
  - on-demand rg search is too slow for latency-first hot paths
- Time complexity / constant-factor impact:
  - bounded by candidate budgets and hop limits; index maintenance is linear in changed files
- Memory / allocation impact:
  - index disk footprint and in-memory query buffers; bounded by candidate limits
- Measurement plan or reason no benchmark is needed:
  - add perf benchmarks once the baseline index and pipeline are integrated
