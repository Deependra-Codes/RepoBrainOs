# SDD-002: Stack And Research Direction

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Decision

Build RepoBrain OS as a **deterministic repository intelligence core with a pluggable ML uplift layer**, using a **Rust systems core**, **Python research/eval layer**, and **TypeScript integration surface**.

## Why This Stack

We do not want the product to bottleneck on:

- one model vendor
- one database trying to do every job
- one language that is great for prototypes but weak for systems work
- one giant monolith that mixes extraction, storage, retrieval, and serving

So the stack must separate concerns cleanly:

- deterministic repo truth
- retrieval and graph navigation
- model-adaptive context compilation
- evaluation and research
- serving and integration

## Hard Architecture Rules

### 1. Canonical repo knowledge must be model-agnostic

The repo brain cannot live only inside prompts or model-specific memory files.

Canonical truth must be stored in open, queryable formats:

- graph records
- evidence receipts
- snapshots
- impact summaries
- eval logs

Schema-backed contracts are the source of truth.

Language-native bindings should be generated from those contracts rather than hand-maintained in parallel.

### 2. Deterministic extraction is the foundation

Anything recoverable from code, build files, tests, and git should be extracted without an LLM whenever possible.

LLMs should synthesize, summarize, and rank. They should not be the source of base repo truth.

Important correction:

- deterministic does not mean syntax-only
- tree-sitter remains the universal syntax floor
- semantic truth for macro-heavy or type-heavy languages may require compiler- or language-service-backed enrichment

### 3. Keep logical storage responsibilities separate

These separations are architectural, not a requirement that every developer must run every engine locally.

Do not force one database to do:

- graph traversal
- analytical scans
- dense retrieval
- exact lookup
- event logging

Use the best store for each access pattern behind clean interfaces.

In local-first v1, we may collapse multiple responsibilities into fewer physical stores if:

- contracts stay stable
- migration paths stay open
- the serving surface does not depend on one vendor-specific engine

### 4. All expensive intelligence must be incremental

Full rebuilds are acceptable for early local development, but the architecture must be built around:

- changed-file detection
- graph-diff propagation
- selective invalidation
- selective re-synthesis

Incremental does not mean "react immediately to every file event."

Background work must be:

- debounced
- batched
- resource-governed
- able to switch into coarse bulk-refresh mode during large VCS or mass-edit operations

### 5. Every smart layer must be measurable

No "magic AI layer" without evaluation.

Every uplift policy, retrieval strategy, and summarization step should be benchmarked against:

- raw context
- lexical retrieval
- hybrid retrieval
- graph-expanded retrieval
- context uplift

### 6. Interactive latency beats theoretical completeness on the hot path

If a design only works by running too much repo cognition work during a user request, it will fail in practice.

Interactive serving should:

- read from maintained snapshots, indexes, and caches
- keep heavyweight extraction and synthesis in the background
- route by latency class before choosing retrieval depth
- degrade honestly when the budget is too small for deeper search

## Recommended Stack

## A. Systems Core

### Language

**Rust**

Why:

- strong fit for parser-heavy, index-heavy, concurrent systems
- good Windows/Linux/macOS support
- excellent for embedded/local-first distribution
- good performance for repo scanning, graph building, and incremental updates

Own in Rust:

- repository scanner
- parser orchestration
- symbol/dependency/build/test extractors
- change detector
- scoped impact engine
- context compiler core
- graph traversal and ranking orchestration

Recommended Rust building blocks:

- `tree-sitter` bindings for multi-language AST extraction
- `rayon` for parallel extraction
- `tokio` for async service boundaries
- `serde` for canonical schemas

See:

- [SDD-006: Contract Compilation And Semantic Extraction](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md)

## B. Research And ML Layer

### Language

**Python**

Why:

- best ecosystem for evaluation, embeddings, rerankers, fine-tuning, and paper-grade experimentation
- fastest path for trying retrieval/ranking/packing ideas without contaminating the systems core

Own in Python:

- retrieval experiments
- reranker experiments
- confidence calibration
- uplift policy experiments
- benchmark harness
- offline training / fine-tuning for small support models

This layer should be optional at runtime where possible.

If a Python experiment wins, port the serving-critical path to Rust or serve it behind a narrow interface.

Important correction:

- Python should not become the escape hatch for contract drift
- schema-derived Python models should be generated from canonical contracts, not rewritten by hand

## C. Integration Surface

### Language

**TypeScript**

Why:

- strong fit for MCP servers, IDE integrations, web dashboards, and developer tooling
- easiest route for shipping integration surfaces quickly

Own in TypeScript:

- MCP server
- CLI wrapper if needed
- web/API gateway if we build hosted mode
- editor integrations
- auth/session layer for hosted product

## Data Plane

## D. V1 Local-First Minimal Storage

### Required local runtime stores

For version 1, local mode should require only:

- **SQLite** for metadata, cache state, adjacency tables, and simple local querying
- **JSON/Parquet artifacts on disk** for snapshots, compiled outputs, and eval/export data

Optional in local mode:

- in-memory graph projections for active requests
- remote vector service if a team wants semantic retrieval earlier

This avoids forcing every developer machine to run four local databases just to get repo context in an IDE.

## E. Optional Graph Accelerator

### Scale-up recommendation

**Kuzu**

Why:

- official docs describe Kuzu as an embedded graph database optimized for join-heavy analytical workloads, with columnar storage, CSR adjacency lists, parallel query execution, and ACID transactions
- strong fit for repository graphs, dependency traversals, flow expansion, and blast-radius queries

Use Kuzu for:

- file, symbol, import, call, build, and test graph
- heterogeneous repo graph traversal
- scoped impact graph diff support
- graph-based evidence expansion

Important architecture choice:

- keep it **repo-scoped**
- one repo or repo-branch brain per graph instance

That avoids a giant central graph bottleneck.

Kuzu is an accelerator for richer graph workloads, not a mandatory v1 local dependency.

## F. Optional Analytical Accelerator

### Scale-up recommendation

**DuckDB**

Why:

- official docs position DuckDB as an in-process OLAP database
- excellent for offline analysis, evals, telemetry slices, and snapshot comparison
- easy local and batch use from Python and Rust

Use DuckDB for:

- benchmark results
- evaluation datasets
- feature extraction analysis
- token-cost analysis
- uplift experiments
- product analytics over snapshots and events

DuckDB is especially valuable for evals and offline analysis, but it does not need to be on the hot path for basic local usage.

## G. Optional Dense And Hybrid Retrieval

### Scale-up recommendation

**Qdrant**

Why:

- official docs emphasize high-performance vector search, dense + sparse hybrid retrieval, metadata filters, and scaling options
- good fit for semantic retrieval plus hybrid ranking experiments

Use Qdrant for:

- dense embeddings
- sparse/dense hybrid retrieval
- candidate generation before graph expansion
- per-repo or per-branch retrieval collections

Qdrant is optional in v1 local mode and should become standard only when semantic retrieval is proven necessary in evals.

## H. Transactional Metadata

### Recommended

**SQLite for local mode**

**PostgreSQL for hosted/team mode**

Use this layer for:

- job metadata
- repo registration
- task queue state
- compilation requests
- audit trail
- user/team/project metadata

Reason:

this data is transactional, not graph-analytical.

## I. Artifact Storage

### Recommended

Use plain files locally and object storage in hosted mode.

Store as:

- Parquet for snapshots and feature tables
- JSON for canonical objects and interchange
- compressed Markdown/JSON for compiled briefing artifacts

Do not bury everything inside the graph DB.

## Compute And Serving Shape

## J. Local-First Execution Model

V1 should run well on a developer machine:

- local SQLite
- local JSON/Parquet artifacts
- optional in-memory graph indexes
- background index/update worker
- Rust worker process
- TypeScript MCP server

This keeps the first product useful even without cloud infrastructure or a heavyweight local database stack.

Important serving rule:

- user-facing requests should read from incrementally maintained snapshot state rather than triggering full multi-language pipeline work inline

## K. Hosted Scale-Out Model

When we outgrow local-first only:

- shard by repo
- shard by branch or revision family when needed
- split services into:
  - ingest workers
  - graph builders
  - retrieval/index workers
  - context compiler service
  - broker/API service
  - eval service

Critical design choice:

**scale by repository partition, not by one global shared brain**

That is how we avoid a central bottleneck.

## ML And Math Strategy

## L. Do Not Train A Frontier Model

That is not the leverage point.

RepoBrain should train or tune only small support models if needed:

- rerankers
- packers
- confidence calibrators
- query classifiers
- model-profile routers

The main value comes from context shaping and evidence-grounded compilation, not from replacing the base coding model.

## M. Recommended Algorithmic Core

Use math and algorithms first:

### 1. Heterogeneous graph retrieval

Retrieve across:

- lexical match
- semantic similarity
- import/call/build/test edges
- ownership / decision / evidence links

Route retrieval by task type rather than forcing one policy for every question.

Also route by latency class before enabling expensive stages.

### 2. Budgeted context packing

Treat briefing construction as an optimization problem:

- maximize relevance + safety + evidence coverage
- under token and latency budgets

This can be modeled as a budgeted maximum-coverage or submodular selection problem.

### 3. Confidence scoring

Model confidence from:

- evidence quality
- corroboration count
- freshness
- contradiction count
- graph distance from primary evidence

Example shape:

`confidence = sigmoid(w1*evidence + w2*corroboration + w3*freshness - w4*contradiction - w5*distance)`

### 4. Semantic delta propagation

Use graph-diff and dependency propagation to estimate:

- what changed directly
- what became stale indirectly
- what briefings likely need regeneration

Important constraint:

- this is scoped impact estimation, not exact semantic equivalence checking

### 5. Uplift policy routing

Choose the context pack style based on:

- model profile
- task type
- repo complexity
- token budget
- latency budget

This can later become a learned policy, but should start rule-based and benchmarked.

## N. Retrieval Strategy

V1 retrieval should not be "search, expand, summarize."

Recommended serving pipeline:

1. request normalization and query classification
2. latency-class assignment, snapshot binding, and freshness gate
3. exact + lexical candidate generation, with dense retrieval optional
4. bounded structural expansion from the strongest anchors
5. task-aware reranking
6. coverage audit and one targeted second pass only when the budget allows it
7. evidence assembly and budgeted packing
8. answer, briefing return, or abstain

This is better than:

- raw repo dump
- vector search alone
- graph search alone

See:

- [SDD-004: Retrieval And Grounding Pipeline](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
- [SDD-005: Latency-First Serving Strategy](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)

Important v1 constraint:

- retrieval policy must depend on task class
- retrieval depth must depend on latency class
- structural expansion must be bounded
- dense retrieval must remain optional
- the system must be allowed to answer with uncertainty or abstain

## Product Interfaces

## O. External Interface Contract

All external clients should talk to RepoBrain through stable contracts:

- `ContextRequest`
- `BriefingPack`
- `EvidenceReceipt`
- `ImpactSummary`
- `ModelProfile`

These contracts should be schema-compiled into language bindings, not manually mirrored forever.

That keeps the system portable across:

- Codex
- Claude Code
- Copilot
- Cursor/Windsurf-style tools
- future internal agents

## P. Publish Layer

Compiled outputs should remain cheap and disposable:

- `AGENTS.md`
- `CLAUDE.md`
- `.github/copilot-instructions.md`
- MCP tools/resources
- cached task briefs

These are **views**, not the brain itself.

## Observability And Evals

## Q. Observability

Use **OpenTelemetry** from day one for:

- ingestion timings
- graph build timings
- retrieval timings
- compile timings
- warm vs cold request timings
- latency by request class and budget class
- token cost
- cache hit rate
- context pack size
- answer/source disagreement

Visualize with:

- Grafana dashboards
- trace inspection
- retrieval and compile latency panels

## R. Eval Harness

This is non-negotiable.

We need an internal benchmark suite that measures:

- repo understanding accuracy
- safe edit success rate
- build/test pass rate after AI edits
- blast-radius prediction accuracy
- citation/evidence accuracy
- uplift delta for weak/medium models

The eval harness is what keeps the research direction honest.

## What Not To Do

Do not:

- build the core in Python only
- store canonical truth only in Markdown
- require every local developer workflow to boot multiple specialized databases
- use one database for graph + OLAP + vectors + transactions without stable abstraction boundaries
- depend on frontier-model summarization for base facts
- optimize for one agent client too early
- skip evals until "later"
- trust syntax-only parsers to resolve macro- or type-heavy semantic relationships
- let every file change trigger unbounded local maintenance work

## Final Recommendation

For the best long-term outcome, RepoBrain OS should use this phased stack:

### V1 local-first baseline

- **Rust** for the deterministic repository cognition core
- **Python** for research, evaluation, and optional small-model components
- **TypeScript** for MCP, IDE, and product integrations
- **SQLite** for the required local runtime store
- **JSON + Parquet artifacts** for portable canonical data
- **OpenTelemetry + Grafana** for observability

### Scale-up / hosted accelerators

- **Kuzu** for richer repo graph intelligence
- **DuckDB** for analytics and evals
- **Qdrant** for dense/hybrid retrieval
- **PostgreSQL** for hosted transactional metadata

This gives us:

- local-first usefulness
- low-friction onboarding
- cloud-scale growth path
- research freedom without polluting the core
- no single architecture bottleneck
- a stack aligned with the actual problem, which is repository intelligence plus context uplift

## Sources

- Tree-sitter intro and parser docs: https://tree-sitter.github.io/tree-sitter/
- Kuzu docs: https://kuzudb.github.io/docs
- DuckDB official docs: https://duckdb.org/docs/stable/
- Qdrant official docs: https://qdrant.tech/documentation/
- OpenTelemetry docs: https://opentelemetry.io/docs/
- Grafana docs: https://grafana.com/docs/
- MutaGReP (February 21, 2025): https://arxiv.org/abs/2502.15872
- CodexGraph (NAACL 2025): https://aclanthology.org/2025.naacl-long.7/
- CoRet (ACL 2025): https://aclanthology.org/2025.acl-short.62/
- CoIR (ACL 2025): https://aclanthology.org/2025.acl-long.1072/
- CodeRAG (EMNLP 2025): https://aclanthology.org/2025.emnlp-main.1187/
- RANGER (September 27, 2025): https://arxiv.org/abs/2509.25257
- Citation-Grounded Code Comprehension (December 13, 2025): https://arxiv.org/abs/2512.12117
- Repository Intelligence Graph / RIG (January 15, 2026): https://arxiv.org/abs/2601.10112
- DeepCodeSeek (September 30, 2025): https://arxiv.org/abs/2509.25716
