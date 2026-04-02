# SDD-004: Retrieval And Grounding Pipeline

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Decision

RepoBrain retrieval should be **query-aware, multi-path, evidence-grounded, coverage-checked, and freshness-gated**.

## Why This SDD Exists

Repository retrieval is one of the easiest places to accidentally ship fake intelligence.

Naive pipelines look fine in diagrams:

- search
- expand
- summarize

But real repository tasks fail when retrieval is:

- one-size-fits-all
- similarity-only
- unaware of build/test structure
- missing cross-file evidence
- unable to say "I do not know yet"

This SDD makes the retrieval story realistic.

## Research Inputs

This design is shaped by:

- MutaGReP on plan-grounded repo context
- CodexGraph on structure-aware retrieval
- RANGER on query-type-dependent retrieval policies
- CoRet on repository-aware retrievers for editing
- CodeRAG on multi-path retrieval and reranking
- Citation-Grounded Code Comprehension on hybrid retrieval plus citation verification
- RepoCoder and RepoHyper on iterative search-expand-refine patterns
- RIG on deterministic structural maps
- GitHub Copilot indexing docs on the importance of fresh repo indexes

See:

- [retrieval research note](d:/RepoBrainOS/research/2026-03-30-retrieval-pipeline-research.md)

## Design Principles

### 1. Classify before retrieve

Different questions need different retrieval policies.

### 2. Deterministic anchors first

Prefer exact file, symbol, build target, test, and diff anchors when available.

### 3. Multi-path candidate generation beats single-path retrieval

Exact, lexical, graph, and optionally dense retrieval should collaborate rather than compete.

### 4. Graph expansion must be bounded

Expansion without per-task edge policies becomes expensive and noisy.

### 5. Rank for task coverage, not just similarity

A useful safe-edit context pack is not the same as a useful architecture explainer.

### 6. Retrieval must preserve provenance

We should always know:

- which channel returned a candidate
- which edges caused an expansion
- which evidence receipts justify a final claim

### 7. Retrieval must fail honestly

When evidence is thin, stale, or contradictory, the system should abstain or widen the search rather than invent confidence.

### 8. Latency class must cap retrieval depth

The hot path must not quietly turn into a full repo-cognition pipeline.

Assign the request a latency class first, then choose which retrieval stages are even allowed.

See:

- [SDD-005: Latency-First Serving Strategy](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)

### 9. Coverage audit is about observable slots, not omniscience

The retrieval system should not pretend it can prove that no invariant or fragile zone is missing in an absolute sense.

It should instead measure whether the observable evidence slots required for the task are satisfied.

## Query Classes

### 1. Entity Lookup

Examples:

- "Where is `AuthSession` defined?"
- "Who calls `invalidate_session`?"

Primary policy:

- exact symbol/path lookup
- lexical fallback
- shallow graph neighborhood

### 2. Architecture / Explanation Query

Examples:

- "How does auth work?"
- "Explain the payment flow."

Primary policy:

- lexical + optional dense retrieval
- graph expansion across import/call/build/test edges
- concept/flow coverage audit

### 3. Safe Edit / Change Request

Examples:

- "I need to modify auth session invalidation."
- "Add a retry path to this worker."

Primary policy:

- exact anchors from task scope
- lexical retrieval for neighboring patterns
- bounded graph expansion
- invariant/test attachment
- risk-aware reranking

### 4. Impact / Blast-Radius Query

Examples:

- "What can break if I edit this?"
- "What changed likely affects logout?"

Primary policy:

- changed-node anchors
- downstream dependency expansion
- build/test coverage expansion
- freshness gating

### 5. Decision / Why Query

Examples:

- "Why is this built this way?"
- "Why is legacy token support still here?"

Primary policy:

- docs/ADR/history retrieval
- code evidence retrieval
- contradiction handling if docs and code disagree

## Retrieval Units

RepoBrain should retrieve and rank units larger than a raw line but smaller than an entire file whenever possible.

Preferred units:

- symbol bodies
- class/type definitions
- file sections
- build targets
- test definitions
- flow capsules
- decision records
- note records

## Index Planes

### 1. Exact Plane

Supports:

- path lookups
- symbol lookups
- build/test target lookups
- changed-file anchors

### 2. Lexical Plane

Supports:

- BM25-style matching over code, comments, docs, tests, and config

### 3. Structural Plane

Supports:

- import edges
- call edges
- build edges
- test coverage edges
- ownership/decision/evidence links

### 4. Semantic Plane

Optional in v1.

Supports:

- dense retrieval for natural-language-heavy queries

This plane should be removable without breaking the baseline system.

## Pipeline

### Stage 0. Request Normalization

Normalize the incoming request into:

- task class
- scope hints
- repo revision or snapshot
- token budget
- latency class
- freshness requirement
- model profile

Also extract obvious anchors:

- file paths
- symbols
- stack traces
- test names
- diff refs

### Stage 0.5. Latency-Class Assignment

Before deeper retrieval, assign the request to a latency class.

Suggested classes:

- `instant` for exact lookups and cached neighborhood answers
- `interactive` for most IDE-safe edit and explanation requests
- `deep` for richer architecture, history, and evidence gathering
- `background` for rebuilds and heavyweight synthesis

This decision constrains:

- candidate budget
- graph hop budget
- whether a second pass is allowed
- whether semantic retrieval is allowed
- whether the request should downgrade to a partial answer

### Stage 1. Snapshot And Freshness Gate

Choose the retrieval snapshot before candidate generation.

Rules:

- bind retrieval to a concrete repo snapshot or revision
- reject or warn on stale snapshots when freshness is required
- prefer widening invalidation over serving falsely fresh evidence

### Stage 2. Query Routing

Choose a retrieval policy by task class.

Examples:

- entity lookup -> exact-first
- safe edit -> structure and tests prioritized
- decision query -> docs/history and code both required

### Stage 3. Candidate Generation

Generate candidates from multiple channels:

- exact anchors
- lexical retrieval
- semantic retrieval when enabled
- docs/decision retrieval when task type requires it

Each candidate must carry:

- source channel
- score
- snapshot id
- retrieval unit type

### Stage 4. Bounded Graph Expansion

Expand only from the strongest anchors and only through allowed edge types for the task.

Examples:

- safe edit: import, call, build, test edges
- architecture query: import, call, ownership, evidence edges
- decision query: docs, decision, evidence, affected-code edges

Controls:

- hop limit
- edge policy
- node budget
- duplicate suppression

These controls should tighten automatically for lower latency classes.

### Stage 5. Candidate Normalization

Normalize candidates before reranking:

- collapse near-duplicate units
- prefer canonical containers
- attach graph distance
- attach freshness and contradiction flags

### Stage 6. Task-Aware Reranking

Rerank with a feature-based model in v1.

Features may include:

- exact-match signal
- lexical score
- dense score
- graph distance from anchor
- build/test proximity
- freshness penalty
- contradiction penalty
- evidence richness
- change locality

Important:

- rerank for task usefulness, not just text similarity

### Stage 7. Coverage Audit

Check whether the current candidate set covers the required slots for the task class.

This is a deterministic slot-completeness check, not an LLM judgment about whether repository truth is "complete."

Each slot should be labeled as one of:

- `present`
- `missing_retrievable`
- `not_observable`
- `not_applicable`
- `stale`

Examples:

For safe-edit:

- target code
- dependency neighborhood
- verification targets
- fragile zones
- do-not-break constraints

For architecture explanation:

- entrypoints
- core modules
- key dependencies
- relevant tests or runtime boundaries

Important:

- if a repo genuinely has no direct unit tests for a flow, that should be labeled `not_observable`, not treated as a reason to loop
- only `missing_retrievable` required slots justify a second pass

### Stage 8. Targeted Second Pass

If coverage is incomplete:

- generate missing-slot subqueries
- retrieve again only for uncovered needs
- merge and rerank

Do not loop indefinitely.

Use one bounded second pass in v1.

Stop conditions:

- no required slot is `missing_retrievable`
- latency class forbids a second pass
- the second pass returns no new slot coverage

Important:

- skip the second pass for `instant`
- allow it selectively for `interactive`
- reserve richer follow-up retrieval for `deep`

### Stage 9. Evidence Assembly

Before packing, attach evidence receipts and verify:

- every major claim in the final pack has support
- citations point to the correct units
- stale or contradictory evidence is marked

### Stage 10. Budgeted Packing

Pack the final context under token and latency budgets.

Rules:

- maximize coverage and evidence quality
- avoid redundant neighboring snippets
- preserve diversity across files and evidence types
- adapt packing density to model profile

### Stage 11. Output Or Abstain

Return:

- a compiled briefing pack
- a focused answer with evidence index
- or an abstain / narrow-request response when coverage is still too weak

## V1 Baseline Vs Later System

### V1 Baseline

Must include:

- request classification
- latency-class assignment
- exact anchor extraction
- lexical retrieval
- bounded structural expansion
- feature-based reranking
- one coverage audit
- one targeted second pass
- evidence assembly
- budgeted packing

### Later Upgrades

- dense retrieval as a normal channel
- learned rerankers
- query rewriting
- plan-guided retrieval
- policy learning for routing and expansion
- more advanced graph search strategies

## What We Explicitly Avoid In V1

- unconstrained graph traversal
- multi-language extraction or graph rebuild on the interactive hot path
- mandatory dense retrieval for every request
- heavy MCTS-style exploration on the hot path
- one giant relevance score with no per-task policy
- silent fallback from stale evidence to confident answers

## Failure Modes And Mitigations

### 1. Over-retrieval

Too many candidates reduce reasoning quality.

Mitigation:

- bounded budgets
- deduplication
- coverage-based second pass instead of giant first pass

### 2. Under-retrieval

Missing dependencies or tests lead to shallow answers.

Mitigation:

- graph expansion
- coverage audit
- targeted second pass

### 2a. Fake completeness

The system may confuse "no evidence found" with "no risk exists."

Mitigation:

- slot status labels such as `not_observable`
- abstain or uncertainty paths
- no hidden LLM completeness oracle on the hot path

### 3. Stale retrieval

Old snapshots poison good reasoning.

Mitigation:

- snapshot binding
- freshness gate
- abstain or warn on stale high-risk requests

### 4. Mis-citation

The answer sounds grounded but points to weak evidence.

Mitigation:

- evidence receipts
- citation verification
- contradiction flags

### 5. Query-policy mismatch

The wrong retrieval policy is chosen for the task.

Mitigation:

- explicit query classes
- auditable routing logic
- evals by task class

## Metrics

Track at least:

- retrieval recall@k by task class
- coverage pass rate before and after second pass
- citation accuracy
- evidence completeness
- stale-hit rate
- abstain rate
- second-pass rate
- latency by stage
- final task success uplift

## Decision

RepoBrain retrieval will be built as a **query-aware, multi-path, bounded, evidence-grounded pipeline** rather than a single generic search step.
