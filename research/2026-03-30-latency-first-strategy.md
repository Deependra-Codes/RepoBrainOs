# Latency-First Strategy Research

Date: 2026-03-30
Status: Draft

## Research Question

What serving strategy keeps RepoBrain usable in real IDE workflows despite a deep, multi-language repository cognition pipeline?

## Main Conclusion

The hot path cannot run the repo brain.

The hot path must read from the repo brain.

That means:

- background maintenance
- snapshot-bound serving
- latency-class routing
- incremental updates
- graceful degradation instead of hidden blocking

## What The Sources Say

### 1. Developer search has hard latency expectations

GitHub's code search write-up says developer-facing code search should be blazingly fast and explicitly targets p95 query latency well under one second, with even lower expectations for narrower scopes.

It also notes that repo-scoped search built on `git grep` led to unresponsive pages and timeouts because scan cost grew with repository size.

Implication:

- RepoBrain cannot depend on repository scans for interactive requests

### 2. Incremental and layered indexes are the practical pattern

clangd uses:

- a dynamic file index
- a background index
- an optional static index
- an optional remote index

The docs explicitly say cached on-disk index files avoid reindexing on startup, that a static index avoids waiting for background indexing, and that remote index exists because whole-project indexing can take hours and significant RAM on very large projects.

Implication:

- RepoBrain should copy the layered serving idea, not invent a "one path for everything" system

### 3. Incremental parsing is a gift we should use

Tree-sitter describes itself as an incremental parsing library that can efficiently update syntax trees as files are edited and aims to be fast enough for every keystroke.

Implication:

- changed-file updates should be incremental
- full-repo reparses on the hot path are unnecessary and self-destructive

### 4. Change detection should be event-driven

Watchman exists specifically to watch files, record when they change, and trigger follow-up actions.

Implication:

- file watching or equivalent change events should drive background refresh
- do not poll or rescan large trees during interactive requests

### 5. Local lexical search can be fast without a heavyweight vector stack

SQLite FTS5 supports ranked local full-text search and its trigram tokenizer supports substring matching.

It also notes that `ORDER BY rank` can be faster than invoking ranking functions directly.

Implication:

- local-first v1 can keep lexical retrieval cheap and simple
- we should not force embeddings into the hot path before proving they pay off

### 6. Code search systems win by specializing for code

Zoekt emphasizes trigram-based substring and regex matching with code-aware ranking signals.

GitHub's Blackbird write-up says standard text search products were a poor fit for code, and that they needed incremental indexing, regex search, symbol metadata, and sub-second latency goals.

Implication:

- exact and lexical code-aware retrieval should do most of the early work
- code-specific indexes matter more than generic "RAG stack" aesthetics

### 7. Compact rerankers beat heavyweight models on the critical path

DeepCodeSeek reports that a compact 0.6B reranker outperformed a larger 8B model while reducing latency by 2.5x.

Implication:

- if we use rerankers, smaller specialized models are more realistic on the serving path
- large-model reranking should stay optional or offline

### 8. Query type should change retrieval depth

RANGER uses fast graph lookups for entity queries and deeper guided exploration for natural-language queries.

Implication:

- RepoBrain should not run the same expensive retrieval policy for all requests

### 9. Retrieval remains hard even before latency pressure

CodeRAG-Bench finds that good contexts help code generation, but retrievers still often fail to fetch useful contexts and generators struggle to use them.

Implication:

- adding more latency and pipeline complexity does not automatically buy quality
- the serving path should stay simple unless added complexity proves itself in evals

## Design Conclusions For RepoBrain

### 1. Separate the maintenance path from the serving path

Maintenance path:

- parse
- extract
- update indexes
- refresh stale high-level views

Serving path:

- bind to snapshot
- retrieve from maintained state
- rerank lightly
- pack and answer

### 2. Route by latency class before retrieval depth

At minimum:

- `instant`
- `interactive`
- `deep`
- `background`

### 3. Keep the interactive path mostly exact plus lexical plus shallow structure

This should carry the bulk of real user traffic.

### 4. Treat dense retrieval as optional

Dense retrieval is useful, but if local-first usability is the goal, it must be:

- precomputed
- budget-aware
- easy to disable

### 5. Prefer partial honest answers over full slow answers

If the repo is still warming or indexes are stale:

- answer from the best maintained snapshot available
- label freshness
- degrade retrieval depth
- refresh in the background

## Recommended Thesis

RepoBrain should be:

**background-heavy, snapshot-bound, latency-tiered**

Shorter:

**maintain deep state continuously, serve shallowly first, escalate only when needed**

## Sources

- GitHub code search history and Blackbird goals: https://github.blog/engineering/architecture-optimization/a-brief-history-of-code-search-at-github/
- GitHub code search architecture: https://github.blog/engineering/the-technology-behind-githubs-new-code-search/
- clangd index design: https://clangd.llvm.org/design/indexing
- clangd remote index: https://clangd.llvm.org/guides/remote-index
- Tree-sitter introduction: https://tree-sitter.github.io/tree-sitter/index.html
- Watchman: https://facebook.github.io/watchman/
- SQLite FTS5: https://sqlite.org/fts5.html
- Zoekt: https://github.com/sourcegraph/zoekt
- DeepCodeSeek: https://arxiv.org/abs/2509.25716
- RANGER: https://arxiv.org/abs/2509.25257
- CodeRAG-Bench: https://aclanthology.org/2025.findings-naacl.176/
