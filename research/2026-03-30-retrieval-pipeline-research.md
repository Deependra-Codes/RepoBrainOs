# Retrieval Pipeline Research

Date: 2026-03-30
Status: Draft

## Research Question

What should a realistic repository retrieval pipeline look like for RepoBrain OS if we want:

- strong repo-scale recall
- evidence-grounded answers
- low hallucination risk
- bounded local-first complexity
- model-adaptive context uplift

## Why The Existing Simple Pipeline Was Not Enough

The earlier retrieval pipeline was directionally correct but underspecified.

It implied:

- one generic retrieval path for very different query types
- graph expansion without explicit policy bounds
- no formal coverage check for missing context
- no abstain path when evidence is weak or stale
- no clear distinction between v1 baseline and later accelerators

That is not robust enough for a serious agent system.

## What The Papers Say

### 1. Long context is not the same as good context

MutaGReP (February 21, 2025) shows that long repository dumps are not the right answer. It finds that longer contexts can hurt reasoning and that plan-grounded retrieval can use less than 5% of a 128K window while matching or approaching full-repo context performance on hard repository tasks.

Implication:

- retrieval should aim for **coverage and usefulness**, not just volume

### 2. Similarity-only retrieval misses cross-file structure

CodexGraph (NAACL 2025) argues that similarity-based retrieval often has low recall on complex tasks, and shows that graph-database interfaces allow more precise structure-aware retrieval.

Citation-Grounded Code Comprehension (December 13, 2025) finds that cross-file evidence discovery is a major source of citation completeness and that hybrid retrieval plus graph expansion can materially improve grounded comprehension.

Implication:

- lexical or dense similarity alone is not enough
- structural expansion is needed, especially for architectural and change-impact questions

### 3. Retrieval should depend on query type

RANGER (September 27, 2025) explicitly uses a dual-stage policy:

- entity queries use direct graph lookup
- natural language queries use graph-guided exploration

Implication:

- the pipeline must classify requests before retrieval
- entity lookup, architecture explanation, and safe-edit analysis should not share the same retrieval policy

### 4. Multi-path retrieval plus reranking is stronger than one path

CodeRAG (EMNLP 2025) identifies three recurring problems in repository retrieval:

- poor query construction
- single-path retrieval
- retriever/model misalignment

Its response is:

- better query construction
- multi-path retrieval
- reranking tuned to what the generation model actually needs

Implication:

- candidate generation should come from multiple channels
- reranking is a first-class stage, not a nice-to-have

### 5. Repository-aware retrievers benefit from structure-aware training

CoRet (ACL 2025) integrates code semantics, repository structure, and call-graph dependencies in a retriever trained specifically for repository-level editing tasks, improving recall by at least 15 points on its reported settings.

Implication:

- if we later add a learned retriever or reranker, it must be repo-aware and task-aware
- a generic text embedding model should not be assumed sufficient

### 6. Iteration and refinement matter

RepoCoder (EMNLP 2023) showed early that iterative retrieval-generation outperforms naive one-shot retrieval for repository-level completion.

RepoHyper (2024) similarly emphasizes search-expand-refine over semantic graphs.

Implication:

- one retrieval pass is often not enough
- we need a coverage check and targeted second pass for missing slots

### 7. Benchmarks remain hard

CoIR (ACL 2025) shows that code retrieval remains difficult even for strong models and retrieval systems across diverse tasks and domains.

Implication:

- we need explicit evaluation and should not trust intuition alone

### 8. Fresh indexing matters in production systems

GitHub's official Copilot indexing docs state that repository indexing improves contextual answers and that keeping the index up to date matters for repository-context interactions.

Implication:

- freshness gating is not optional
- retrieval should be tied to a specific repo snapshot or revision view

## Design Conclusions For RepoBrain

### 1. Retrieval must be query-aware

At minimum distinguish:

- entity lookup
- architecture/explanation query
- safe edit / change request
- impact / blast-radius query
- decision / why query

### 2. Retrieval must be multi-channel

Required channels:

- exact/path/symbol/build/test match
- lexical retrieval
- bounded graph expansion

Optional channel:

- dense retrieval

### 3. Retrieval must be coverage-aware

The pipeline should detect missing context, not assume the first candidate set is enough.

Coverage slots depend on task type.

For example, a safe-edit request often needs:

- edit target
- local dependency neighborhood
- tests
- invariants
- likely blast radius

### 4. Retrieval must be evidence-first

Every returned context bundle should preserve:

- which channel produced each candidate
- which edges were expanded
- which evidence receipts justify the final bundle

### 5. Retrieval must fail honestly

When evidence is too weak or stale, the system should:

- abstain
- widen invalidation
- request narrower scope
- or return uncertainty explicitly

### 6. V1 should stay simpler than the papers

Do not copy every advanced idea into the first implementation.

V1 baseline should be:

- query classification
- deterministic anchor extraction
- exact + lexical retrieval
- bounded graph expansion
- feature-based reranking
- coverage audit
- evidence assembly
- budgeted packing

Later:

- dense retrieval
- learned reranking
- query rewriting
- search policies closer to RANGER or MutaGReP

## Recommended Retrieval Thesis

RepoBrain should use:

**query-aware, multi-path, evidence-grounded, coverage-checked retrieval**

Shorter:

**retrieve by task, expand by structure, rank by evidence, pack by coverage**

## Sources

- MutaGReP (February 21, 2025): https://arxiv.org/abs/2502.15872
- CodexGraph (NAACL 2025): https://aclanthology.org/2025.naacl-long.7/
- CoRet (ACL 2025): https://aclanthology.org/2025.acl-short.62/
- CoIR (ACL 2025): https://aclanthology.org/2025.acl-long.1072/
- Beyond Function-Level Search / ReflectCode (Findings EMNLP 2025): https://aclanthology.org/2025.findings-emnlp.1147/
- CodeRAG (EMNLP 2025): https://aclanthology.org/2025.emnlp-main.1187/
- RANGER (September 27, 2025): https://arxiv.org/abs/2509.25257
- Citation-Grounded Code Comprehension (December 13, 2025): https://arxiv.org/abs/2512.12117
- Repository Intelligence Graph / RIG (January 15, 2026): https://arxiv.org/abs/2601.10112
- RepoCoder (EMNLP 2023): https://aclanthology.org/2023.emnlp-main.151/
- RepoHyper (2024): https://arxiv.org/abs/2403.06095
- GitHub Copilot repository indexing docs: https://docs.github.com/en/enterprise-cloud@latest/copilot/concepts/context/repository-indexing
