# SDD-001: Repository Cognition And Context Uplift Engine

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Thesis

RepoBrain OS continuously converts a repository into an evidence-backed, self-refreshing understanding layer and model-adaptive context uplift engine, so any AI can work with repo-level understanding and weaker models can perform above their native level.

## Why This SDD Comes First

Before we build chat, workflow automation, or polished UX, we need the core thing that makes RepoBrain OS different:

- not just repo search
- not just auto-docs
- not just a knowledge graph

We need the system that turns repo reality into reusable machine understanding.

## Problem

Today, AI agents repeatedly waste time re-learning the same codebase because:

- raw code is too large and too unstructured
- docs drift from implementation
- architectural intent is scattered across PRs, commits, and people
- every tool wants context in a different format
- context windows are limited, even when large

The result:

- slow onboarding for humans and AIs
- repeated hallucinations about architecture
- shallow refactors that miss blast radius
- duplicated prompt engineering effort across tools

## Sharpened Pain Statement

The real pain in modern coding agents is not just "lack of context."

It is that current context is often:

- too large to reason over well
- too shallow to explain architecture
- too stale to trust
- too generic for the task at hand
- too unshaped for weaker or cheaper models

This creates a practical ceiling:

- strong models waste tokens rediscovering structure
- weak models collapse under noisy repo context
- teams overpay for larger models when better context engineering could do part of the work
- every tool creates yet another partial memory surface

## Product Goal

Give any AI a deep operational understanding of the repository in one pass, with understanding that:

- stays current as the repo changes
- explains both what exists and why
- exposes confidence and evidence
- adapts to the model and the task
- can be requested on demand through targeted context queries

Secondary goal:

- lift the useful performance of weaker or cheaper coding models by externalizing repo cognition, planning clues, invariants, and verification targets into better context packages

## Non-Goals

This SDD does not define:

- a full end-user UI
- autonomous code change execution
- organization-wide enterprise memory
- runtime observability integration in v1

## Core Idea

The core system has three parts:

### 1. Repository Cognition Engine

Builds a canonical knowledge model from:

- source code
- dependency structure
- build and test definitions
- git history
- pull requests and issues
- existing docs

### 2. Proof-Carrying Context Compiler

Transforms that knowledge into the right output for:

- a model
- a task
- a token budget
- a confidence threshold

### 3. Context Uplift Layer

Shapes the compiled context to match the working model's strengths and weaknesses.

For weaker models, RepoBrain should narrow scope, add scaffolding, increase explicitness, and reduce ambiguity.

For stronger models, RepoBrain should preserve optional depth, evidence, and exploratory freedom.

## Design Principles

### 1. Deterministic facts first

Use deterministic extraction wherever possible for:

- files
- symbols
- imports
- call relationships
- build graph
- test graph
- ownership

LLM synthesis should sit on top of extracted facts, not replace them.

### 2. Every important claim needs evidence

If the system claims:

- "this service owns auth"
- "this module is fragile"
- "this flow passes through these files"

it should attach evidence references whenever possible.

### 3. Freshness is a first-class field

Knowledge is not binary true/false. It is:

- fresh
- stale
- contradicted
- inferred
- unverified

### 4. Compilation beats dumping

The answer is not to hand the AI the whole repo, whole graph, or whole docs set. The answer is to compile the smallest high-value understanding package for the current job.

### 5. One canonical brain, many outputs

The repo should have one core knowledge model that can publish to many surfaces.

### 6. Shape context to model capability

The same context package should not be given to every model.

Weaker models need:

- narrower scope
- stronger task decomposition
- more explicit invariants
- clearer file candidates
- more concrete verification targets

Stronger models can handle:

- broader evidence
- more open-ended exploration
- lower scaffolding density

### 7. Bound meaning claims

RepoBrain should not claim full semantic understanding of arbitrary code changes.

Instead, it should:

- detect direct structural impact deterministically
- propagate likely staleness through bounded dependency relationships
- attach confidence and freshness to synthesized claims
- surface uncertainty instead of pretending perfect semantic invalidation

### 8. Serve from maintained snapshots, not live pipeline rebuilds

Interactive requests should not wait for full multi-language extraction, graph rebuild, or large synthesis passes.

RepoBrain should:

- keep hot-path retrieval bound to maintained snapshots and caches
- update indexes and higher-level views incrementally in the background
- reduce retrieval depth when the latency budget is tight
- prefer partial but honest answers over blocking the user on heavyweight recomputation

## Canonical Objects

### Knowledge Record

The base unit of repo knowledge.

Not every record is equally crisp. Some are deterministic facts, some are synthesized claims, and some are fuzzy human notes.

Fields:

- `id`
- `record_type`
- `statement`
- `scope`
- `evidence[]`
- `freshness`
- `confidence`
- `derived_from[]`
- `invalidated_by[]`
- `updated_at`

Record types:

- `fact`
- `claim`
- `note`

Example kinds:

- `symbol_fact`
- `dependency_fact`
- `flow_fact`
- `decision_fact`
- `constraint_fact`
- `risk_fact`

### Fact Record

Used for deterministic, machine-extractable truths.

Examples:

- symbol definitions
- imports
- build targets
- test ownership

### Claim Record

Used for synthesized or inferred understanding that must remain evidence-backed and confidence-labeled.

Examples:

- "this subsystem is fragile"
- "this flow likely depends on these modules"

### Note Record

Used for human or fuzzy knowledge that is valuable but not strongly formalizable.

Examples:

- "this module is generally considered legacy"
- "this migration path is discouraged by the team"

These should remain sparse, explicitly marked as notes, and never be confused with deterministic facts.

### Evidence Receipt

Proof that supports a record.

Fields:

- `source_type`
- `source_ref`
- `locator`
- `snippet_hash`
- `captured_at`

Possible sources:

- file and line range
- AST node
- build file target
- test definition
- commit
- pull request
- ADR

### Concept Node

A higher-level abstraction synthesized from many records.

Examples:

- Authentication subsystem
- Payment flow
- Repo bootstrap process
- Feature flag pipeline

### Flow Capsule

A compact explanation of how a user or system flow moves through the codebase.

Fields:

- `entrypoints`
- `core_modules`
- `state_transitions`
- `side_effects`
- `failure_modes`
- `tests_covering_flow`
- `evidence[]`

### Decision Node

Stores why the repo looks the way it does.

Fields:

- `decision`
- `status`
- `alternatives`
- `tradeoffs`
- `evidence[]`
- `supersedes`

### Briefing Pack

A compiled output for a specific consumer.

Fields:

- `task_type`
- `consumer_type`
- `model_profile`
- `token_budget`
- `scaffolding_level`
- `must_know`
- `relevant_flows`
- `relevant_decisions`
- `fragile_zones`
- `do_not_break`
- `suggested_files`
- `reasoning_scaffold`
- `impact_summary`
- `verification_targets`
- `evidence_index`

### Context Request

The input contract used by a working AI to ask RepoBrain for exactly the context it needs.

Fields:

- `goal`
- `task_type`
- `consumer_type`
- `model_profile`
- `question`
- `scope_hint`
- `token_budget`
- `latency_budget`
- `depth`
- `freshness_requirement`
- `include_evidence`

Example requests:

- "Give me the auth flow for a safe refactor."
- "What can break if I edit this module?"
- "Explain this service to a new AI in under 4k tokens."
- "List invariants and risky files for this change."

## System Architecture

### A. Source Ingestion

Inputs:

- repository contents
- lockfiles and manifests
- build files
- test configs
- commit history
- PR metadata
- issue metadata
- existing markdown docs

### B. Deterministic Extractors

Responsibilities:

- file tree model
- symbol graph
- import and dependency graph
- build/test graph
- ownership heuristics
- change impact map

### C. Synthesis Layer

Responsibilities:

- cluster records into concepts
- generate architecture summaries
- build flow capsules
- infer decision candidates
- produce risk annotations with confidence labels

### D. Scoped Impact and Freshness Engine

Responsibilities:

- detect which deterministic records are directly affected by a change
- propagate staleness through bounded dependency, build, test, and ownership edges
- classify impact as `confirmed`, `likely`, `possible`, or `unknown`
- lower confidence when evidence is outdated
- mark concepts as stale or contradicted
- trigger selective regeneration instead of full rebuild

Important constraint:

- this engine estimates impact and freshness
- it does not solve general program semantics
- it should prefer honest uncertainty over false certainty

### E. Context Compiler

Responsibilities:

- choose the right records and concepts for a task
- compress them into a compact brief
- adapt phrasing and structure for the target AI/tool
- enforce token budgets
- preserve an evidence index for trust and drill-down

### F. Retrieval And Grounding Pipeline

The retrieval layer should be query-aware, multi-path, evidence-grounded, coverage-checked, and freshness-gated.

Responsibilities:

- classify the request before retrieval
- bind retrieval to a concrete snapshot or revision view
- choose retrieval depth from the latency budget before doing expensive work
- enforce freshness gates for high-risk requests
- generate candidates across exact, lexical, structural, and optional semantic channels
- expand through bounded graph policies instead of unconstrained traversal
- audit candidate coverage for the task
- run one targeted second pass when critical slots are still missing
- assemble evidence before the compiler packs the answer

See:

- [SDD-004: Retrieval And Grounding Pipeline](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
- [SDD-005: Latency-First Serving Strategy](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)

### G. Context Uplift Adapter

The model-adaptive layer that decides how much scaffolding to inject into the returned context.

Responsibilities:

- classify the model profile and likely failure modes
- tune scope width and abstraction level
- generate reasoning scaffolds for weaker models
- attach invariant lists and verification targets
- convert broad repo knowledge into an execution-friendly brief

Example uplift behaviors:

- weak model -> smaller scope, explicit steps, exact file shortlist
- medium model -> balanced brief plus verification checklist
- strong model -> concise brief plus expandable evidence index

### H. Context Broker

The queryable interface that lets a working AI ask RepoBrain for targeted understanding instead of reading everything up front.

Responsibilities:

- accept structured context requests
- route the request to the right retrieval policy, records, flows, and decisions
- ask follow-up retrieval questions internally when coverage is incomplete
- return warnings or abstain when evidence is too weak, stale, or contradictory
- return a compiled briefing pack or focused answer
- expose specialized tools for common asks

Example tool shapes:

- `get_brief(goal, scope, token_budget)`
- `explain_flow(flow_name, depth)`
- `blast_radius(target)`
- `list_invariants(scope)`
- `why_is_it_built_this_way(scope)`
- `what_changed_semantically(ref)`

### I. Publication Layer

Outputs:

- MCP resources and tools
- `AGENTS.md`
- `CLAUDE.md`
- `.github/copilot-instructions.md`
- task briefs in Markdown or JSON
- architecture snapshots
- impact briefs

## Key Flows

### 1. Repo Bootstrap

1. Parse repository contents.
2. Build deterministic structural graph.
3. Generate initial fact records and note placeholders.
4. Synthesize concept nodes and flow capsules.
5. Publish baseline context surfaces.

### 2. Incremental Change Update

1. Detect changed files, symbols, build targets, and tests.
2. Mark directly affected deterministic records as stale or changed.
3. Propagate bounded freshness impact through dependency edges.
4. Recompute only affected synthesized views.
5. Emit an impact brief:
   what likely architectural understanding changed, with confidence and uncertainty labels.

### 3. AI Task Briefing

1. Receive a task such as "modify auth flow."
2. Route the task through a query-aware retrieval policy.
3. Resolve relevant concepts, decisions, risks, and verification targets.
4. Adapt the briefing to the model profile and task budget.
5. Return both the compact answer and the evidence index.

### 4. Interactive Context Pull

1. A working AI asks RepoBrain a targeted question.
2. RepoBrain normalizes the request and classifies the task type.
3. The retrieval layer binds the request to a concrete snapshot, assigns a latency class, and checks freshness requirements.
4. RepoBrain gathers candidates across exact, lexical, structural, and optional semantic paths allowed by that latency class.
5. RepoBrain audits coverage and runs one targeted second pass only when the latency budget allows it.
6. The compiler returns a focused answer that is useful immediately, with optional evidence receipts for drill-down and explicit gaps when depth was capped by latency.

This is how we avoid forcing every AI to re-learn the full repo for every task.

### 5. Model Uplift Flow

1. The working AI identifies itself or is assigned a model profile.
2. RepoBrain estimates how much ambiguity and abstraction that model can tolerate.
3. RepoBrain compiles a context pack tuned for that model:
   narrower and more guided for weaker models, broader and lighter for stronger ones.
4. The returned pack includes not just facts, but task scaffolding, invariants, and verification targets.

This is the mechanism by which RepoBrain can make a weaker model behave closer to a medium-strength coding model on repository tasks.

### 6. Contradiction Detection

If code, docs, and historical decisions disagree:

- keep both claims
- reduce confidence
- mark contradiction edges
- surface the conflict for human resolution

## What Makes This Different

### Not just documentation

Docs are one output surface.

### Not just retrieval

Retrieval finds pieces. RepoBrain should assemble understanding.

### Not just a graph

A graph helps navigation, but it is still too raw for many agent tasks.

### Not just memory files

Memory files should be compiled artifacts, not the primary source of truth.

### A pull-based repo brain

The working AI should be able to ask:

- "What do I need to know before editing this?"
- "What is the blast radius?"
- "Why does this exist?"
- "Explain the payment flow in compact form."

RepoBrain should answer with the minimum high-value context, not a giant dump.

### A model-uplift system

The goal is not only to help frontier models.

RepoBrain should make smaller or weaker coding models substantially more useful by offloading:

- repo navigation
- cross-file grounding
- invariant extraction
- blast-radius estimation
- plan scaffolding
- impact and freshness summarization

## MVP Scope

Version 1 should include:

- repository file and symbol graph
- dependency, build, and test relationships
- evidence-backed architecture capsules
- evidence-backed flow capsules
- scoped impact and freshness estimation for changed areas
- query-aware retrieval with exact anchor extraction, lexical retrieval, bounded structural expansion, coverage audit, and one targeted second pass
- a queryable context broker for targeted AI requests
- model-adaptive context uplift for at least three model profiles
- compiler outputs for:
  - `AGENTS.md`
  - `CLAUDE.md`
  - `.github/copilot-instructions.md`
  - MCP read endpoints
- impact briefs that explain likely meaning-level repo changes with confidence labels
- latency-aware serving that keeps most IDE-facing requests on prebuilt snapshot state instead of heavyweight live recomputation

## Later Scope

After the MVP, expand toward:

- PR and issue decision mining
- runtime trace ingestion
- team feedback loops
- repo portfolio / multi-repo product graph
- agent performance telemetry tied to briefing quality
- automatic uplift policy tuning based on eval outcomes

## Example Output Shape

```json
{
  "task_type": "safe_edit",
  "consumer_type": "claude_code",
  "model_profile": "medium_coder",
  "scaffolding_level": "medium",
  "must_know": [
    {
      "statement": "Authentication session creation begins in auth/api.ts and fans out into session-store.ts and token-service.ts.",
      "confidence": 0.93,
      "freshness": "fresh",
      "evidence_ids": ["ev_17", "ev_19", "ev_31"]
    }
  ],
  "reasoning_scaffold": [
    "inspect auth/api.ts entrypoint",
    "verify downstream writes in session-store.ts",
    "run logout and token-compat regression checks"
  ],
  "impact_summary": {
    "reference": "HEAD~1..HEAD",
    "summary": "Session invalidation behavior is likely affected in the auth flow.",
    "changed_scope": ["src/auth/api.ts", "src/auth/session-store.ts"],
    "stale_concepts": ["authentication subsystem"],
    "freshness_impact": "localized"
  },
  "fragile_zones": [
    "session invalidation logic",
    "backward-compatible cookie parsing"
  ],
  "do_not_break": [
    "legacy mobile session tokens must remain valid",
    "logout must revoke both cache and database sessions"
  ],
  "verification_targets": [
    "logout integration test",
    "legacy cookie parsing test"
  ]
}
```

## Success Metrics

Primary metrics:

- time to first correct architectural answer
- time to first safe repo edit by an AI
- reduction in repeated context-setting by humans
- semantic answer accuracy on repo understanding tasks
- stale knowledge rate
- false-fresh rate
- unnecessary rebuild rate
- tokens consumed per successful task
- uplift delta for smaller models versus baseline prompts

Secondary metrics:

- onboarding speed
- blast-radius prediction quality
- doc drift incidents prevented
- latency added by uplift compilation

## Risks

### 1. Over-synthesis

LLM summaries can sound smart while being weakly grounded.

Mitigation:

- deterministic extraction first
- confidence labels
- evidence receipts

### 2. Knowledge bloat

The system can become another giant stale artifact.

Mitigation:

- records instead of giant monolith docs
- invalidation engine
- task-specific compilation

### 3. Impact overclaim

If RepoBrain sounds more certain than its evidence supports, users will over-trust stale architecture plans.

Mitigation:

- bound the engine to scoped impact estimation
- label `confirmed`, `likely`, `possible`, and `unknown`
- gate high-risk briefings on freshness requirements

### 4. Trust collapse after contradictions

If users catch a few wrong claims, they may stop trusting the system.

Mitigation:

- surface uncertainty explicitly
- preserve drill-down to evidence
- label inferred vs confirmed facts

### 5. Too broad too early

Trying to solve docs, chat, autonomy, and observability together will slow execution.

Mitigation:

- lock the first milestone to repository cognition and compiled context outputs

### 6. Overfitting the context to one model family

If the uplift layer is too tied to one client or vendor, portability breaks.

Mitigation:

- use explicit model profiles instead of vendor-only logic
- keep canonical knowledge model vendor-neutral
- evaluate across multiple model sizes and providers

## First Milestones

### Milestone 1

Bootstrap the repository cognition core:

- file tree
- symbol graph
- dependency graph
- build/test graph

### Milestone 2

Add fact/claim/note records, evidence receipts, and the freshness model.

### Milestone 3

Ship first context compiler outputs and query interface:

- context request schema
- model profile schema
- `get_brief`
- `explain_flow`
- `blast_radius`
- `AGENTS.md`
- `CLAUDE.md`
- Copilot instructions
- MCP resources

### Milestone 4

Ship bounded impact briefs and model-adaptive uplift behavior after repo updates.

### Milestone 5

Add an eval harness that measures whether RepoBrain makes weaker models materially better on repo understanding and safe-edit tasks.

## Open Questions

- What should be the first canonical storage format: JSON files, SQLite, graph DB, or hybrid?
- How much LLM synthesis should happen at ingest time vs request time?
- Should decision mining start from git history only, or require explicit ADR support in v1?
- What is the minimum evidence model needed before users trust the system?
- What false-fresh rate is acceptable for v1 before we force wider invalidation?
- Which model profiles should v1 optimize for first: weak local coder, medium cloud coder, or frontier agent?

## Decision

RepoBrain OS should start by building the **Repository Cognition Engine**, **Proof-Carrying Context Compiler**, and **Context Uplift Layer** as the foundation for every later capability.

That is the most defensible and highest-leverage first SDD for this project because it solves the real bottleneck: not model access to code, but model ability to receive the right repo understanding in the right form.
