# SDD-005: Latency-First Serving Strategy

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Decision

RepoBrain should serve interactive requests from incrementally maintained snapshots, indexes, and caches, not by running the full multi-language cognition pipeline inline.

## Why This SDD Exists

Latency is one of the easiest ways to kill this product.

If users ask a question in their IDE and RepoBrain responds by:

- reparsing large parts of the repo
- rebuilding graphs
- recomputing embeddings
- rerunning heavy synthesis

then the system may look smart in architecture diagrams but feel unusable in real workflows.

RepoBrain must be deep, but it must also feel immediate enough to sit in the developer loop.

## Research Inputs

This strategy is shaped by:

- GitHub code search on sub-second developer-facing latency targets
- clangd's split between dynamic, background, static, and remote indexes
- Tree-sitter's incremental parsing model
- Watchman's change-detection model
- SQLite FTS5 support for substring search and ranked local retrieval
- Zoekt's trigram-plus-syntax approach for fast code search
- DeepCodeSeek's compact-reranker latency lesson
- RANGER's split between fast entity retrieval and deeper exploration
- CodeRAG-Bench and related work showing retrieval quality is hard enough without adding unnecessary hot-path cost

See:

- [latency research note](d:/RepoBrainOS/research/2026-03-30-latency-first-strategy.md)

## Product Thesis

The right architecture is not:

- "run the smartest possible pipeline on every request"

The right architecture is:

- maintain rich repo state continuously in the background
- serve most requests from that maintained state
- choose retrieval depth from the latency budget
- escalate only when the request actually needs deeper work

## Non-Negotiable Rule

### No full repo cognition on the interactive hot path

Interactive requests must not depend on:

- full repo rescans
- whole-repo graph rebuilds
- on-demand embedding generation
- heavyweight multi-pass planning
- LLM summarization over large raw repo slices

Those belong in background maintenance, async enrichment, or explicitly deep requests.

### No ungoverned background churn either

Background maintenance must not be allowed to saturate local CPU and I/O whenever a repo experiences mass changes.

The product fails if foreground developer experience degrades during rebases, checkouts, or bulk edits.

## Latency Classes

RepoBrain should assign every request a latency class before retrieval depth is chosen.

### 1. Instant

Target:

- warm local p95 under 150 ms

Use for:

- path lookup
- symbol lookup
- "who calls this?"
- cached blast-radius neighborhood

Allowed work:

- exact lookup
- lexical fallback
- shallow cached graph walk
- cached pack return

### 2. Interactive

Target:

- warm local p95 under 800 ms

Use for:

- most IDE-safe edit requests
- compact flow explanations
- invariants and likely risks

Allowed work:

- exact retrieval
- lexical retrieval
- bounded structural expansion
- lightweight feature-based reranking
- one selective second pass when still within budget

### 3. Deep

Target:

- warm local p95 under 2500 ms

Use for:

- richer architecture questions
- history and decision reconstruction
- broader evidence gathering
- optional semantic retrieval or compact reranking

Allowed work:

- everything in `interactive`
- richer docs/history retrieval
- optional semantic retrieval
- optional compact reranker

### 4. Background

Target:

- async, not user-blocking

Use for:

- index rebuild
- flow/decision regeneration
- embedding jobs
- deeper freshness propagation
- eval and analytics jobs

## Maintenance Budget Controller

The maintenance path needs its own controller, not just a file watcher.

Responsibilities:

- debounce and batch file events
- detect bulk-change situations
- cap concurrent maintenance work
- switch between fine-grained refresh and coarse refresh modes
- protect foreground responsiveness

### Fine-Grained Mode

Use for:

- small edit bursts
- normal editor save events
- narrow file-local changes

Behavior:

- short debounce window
- targeted exact, lexical, and adjacency refresh
- selective stale marking

### Bulk-Change Mode

Use for:

- checkout, rebase, merge, branch switch
- mass rename or search-and-replace
- unusually large changed-file sets

Behavior:

- coalesce events into a wider batch window
- mark broader regions stale quickly
- prefer one bounded coarse refresh over thousands of micro-jobs
- defer heavy synthesis until the tree settles

## Serving Architecture

### 1. Snapshot-Bound Query Serving

Every request should bind to a concrete maintained repo snapshot.

That snapshot can be:

- the latest warm local snapshot
- a revision-specific snapshot
- a local overlay of warm snapshot plus open-file or just-changed-file deltas

### 2. Dynamic Plus Background Layers

Use a layered model similar to strong IDE systems:

- dynamic layer for open or recently changed files
- background index for whole-repo coverage
- optional static snapshot for faster cold start
- optional remote index for very large repos

### 3. Latency Controller

The broker should decide:

- latency class
- max candidates
- max graph hops
- whether semantic retrieval is allowed
- whether a second pass is allowed
- whether to return partial results now and deeper results later

## V1 Hot Path Rules

### Allowed on the hot path

- request normalization
- exact anchor extraction
- exact lookup over maintained symbol/path maps
- lexical retrieval over maintained local index
- shallow graph expansion over maintained adjacency tables
- lightweight feature-based reranking
- pack compilation from maintained records and caches

### Forbidden on the hot path

- full multi-language parse of the repo
- global graph rebuild
- whole-repo flow synthesis
- embedding generation
- heavy MCTS-style retrieval
- multi-model orchestration before a first useful answer

## Index Strategy

### Exact Plane

Use:

- SQLite tables
- in-memory hash maps for hot symbols and paths
- revision and scope keys

This plane should answer a large fraction of entity-style queries without needing broader retrieval.

### Lexical Plane

V1 local-first default:

- SQLite FTS5

Why:

- no extra service
- good enough local footprint
- ranked retrieval
- trigram tokenizer support for substring matching

For code-oriented substring and pattern search, FTS5's trigram tokenizer is a strong low-friction baseline.

### Structural Plane

Use:

- adjacency tables in SQLite
- compact in-memory neighborhood caches

This is where import, call, build, test, and evidence edges should be read from on the hot path.

### Semantic Plane

Optional in v1.

Only enable when:

- the latency class allows it
- embeddings already exist
- evals show the gain justifies the cost

## Background Update Pipeline

### 1. Change Detection

Use filesystem or VCS-driven change detection.

Good patterns:

- Watchman-style file change watching
- git revision diffs
- explicit editor save events

Important:

- use clocks or equivalent race-free incremental cursors
- prefer source-control-aware bulk-change handling when available

### 2. Incremental Parse And Extract

Use incremental parsing where possible.

Tree-sitter is especially relevant because it is designed for efficient incremental updates.

### 3. Selective Index Refresh

After a change:

- update exact tables
- update lexical index entries for touched units
- update adjacency tables for touched symbols/files
- mark affected higher-level records stale

Do not treat every event stream equally.

If change volume crosses a configured threshold:

- collapse into one coarse refresh job
- delay deeper regeneration until the tree is stable

### 4. Deferred Higher-Level Regeneration

Do not block interactive queries on immediate regeneration of:

- flow capsules
- architecture capsules
- decision synthesis

Serve with freshness labels and refresh these in the background.

## Caching Strategy

Cache at multiple levels.

### 1. Snapshot Cache

Key by:

- repo root
- revision id
- local overlay hash

### 2. Retrieval Cache

Key by:

- normalized query
- task class
- scope hash
- snapshot id
- latency class

### 3. Briefing Cache

Key by:

- normalized context request
- model profile bucket
- token budget bucket
- snapshot id

## Graceful Degradation Policy

When the budget is tight or the system is cold:

- drop semantic retrieval first
- tighten graph hop and candidate budgets
- skip the second pass
- return exact plus lexical plus shallow structure only
- mark what is missing

This is better than pretending depth we did not actually compute.

When the maintenance path is overloaded:

- prefer stale labels over blocking
- prefer coarse snapshot refresh over event-by-event thrash
- expose that the system is catching up

## Large-Repo Escape Hatch

Some repos will outgrow a purely local background-index model.

When that happens, the system should support:

- prebuilt static index snapshots
- optional remote index serving
- repo-partitioned hosted acceleration

But the local UX principle stays the same:

- open-file and recent-change state remains local and freshest
- large precomputed coverage can be external

## Metrics

Track at least:

- cold-start bootstrap time
- warm local p50 and p95 by latency class
- cache hit rate by layer
- percentage of requests served without waiting on background work
- second-pass rate by latency class
- stale-but-served rate
- CPU and memory cost of background maintenance
- user-visible time to first useful answer
- batch size distribution for maintenance windows
- percentage of bulk changes promoted to coarse refresh mode
- foreground slowdown incidents during maintenance

## Decision

RepoBrain will be built as a latency-first system where deep repo cognition is maintained incrementally and interactive requests are served from snapshot-bound state with explicit latency classes.
