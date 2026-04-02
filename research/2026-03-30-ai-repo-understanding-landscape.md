# AI Repo Understanding Landscape

Date: 2026-03-30

## Research Question

What is the best first SDD direction for RepoBrain OS if the goal is:

- any AI model can understand a repo deeply in one shot
- that understanding grows automatically with the repo
- the result is dynamic, trusted, and reusable across tools

## Short Answer

The strongest gap is not "better docs" or "better code search."

The gap is a **repo-native cognition layer** that continuously builds an evidence-backed understanding of the repository, then compiles that understanding into the right form for each AI, task, and context budget.

Working name:

**Proof-Carrying Repo Context**

## What The Market Already Covers

### 1. AI-specific instruction surfaces

Vendors now support repo-native memory and instruction files:

- `AGENTS.md`
- `CLAUDE.md`
- `.github/copilot-instructions.md`
- agent skills and MCP servers

This means the ecosystem is moving toward standard places where AIs can read project guidance. Good news for RepoBrain OS: there is now a clear "publish layer."

### 2. Repository graphs and structural intelligence

Several systems already parse repositories into structural graphs:

- GitLab Knowledge Graph
- Greptile's graph-based codebase context
- CodexGraph (NAACL 2025)
- Repository Intelligence Graph / RIG (arXiv 2026)

This validates that graph-aware repo retrieval beats plain similarity search for complex code understanding.

### 3. Auto-generated and auto-synced documentation

Documentation platforms already solve parts of stale-doc pain:

- DocSync generates architecture guides, module docs, user flows, and updates them on merge
- Swimm focuses on code-coupled documentation and continuous sync

This validates that teams want AI-readable docs that stay fresh, not static wikis.

### 4. Long-running agent context management

Modern coding agents increasingly rely on context compaction and reusable memory/instruction layers because raw conversation history does not scale cleanly across long tasks.

This matters because "deep repo understanding in one shot" is not just a retrieval problem. It is also a **compression and compilation** problem.

## Where The Real Gap Still Exists

After the market scan, the interesting white space is here:

### 1. Most tools expose information, but do not compile understanding

They give the model:

- raw code
- raw docs
- a graph
- a search result

They usually do **not** generate a task-specific, model-specific "mission brief" that says:

- here is the architecture you need
- here are the true dependencies
- here are the fragile invariants
- here is the likely blast radius
- here is why the system looks this way
- here is how certain each claim is

### 2. Trust is still weak

Most systems can answer questions, but they do not attach strong provenance to each claim. There is rarely a first-class concept of:

- evidence source
- freshness
- confidence
- contradiction
- invalidation after repo change

### 3. Decision memory is fragmented

Structure can be extracted from code, but the **why** often lives across:

- pull requests
- commit history
- issues
- ADRs
- comments
- tribal knowledge

Most tools still under-model this layer.

### 4. Multi-model portability is fragmented

The market supports many surfaces, but the team still has to manually maintain them:

- `AGENTS.md`
- `CLAUDE.md`
- Copilot instructions
- MCP resources
- architecture docs

There is no strong default system that treats these as compiled outputs from one canonical repo brain.

### 5. Incremental change understanding is underdeveloped

Docs may update after merge, but very few systems center the question:

**What changed in the repo's meaning?**

That is different from "what files changed?"

## Recommended Product Thesis

RepoBrain OS should not position itself as:

- a docs generator
- a code search tool
- a code graph viewer

It should position itself as:

**a repository cognition operating system that converts code, history, and decisions into proof-carrying context for any AI agent**

## Recommended First SDD Theme

### SDD-001: Repository Cognition Engine + Proof-Carrying Context Compiler

This first SDD should define:

- the canonical knowledge model for the repo
- how understanding is extracted and refreshed
- how every claim is linked back to evidence
- how repo changes invalidate or refresh knowledge
- how the system publishes model-specific context surfaces

## The Novel Angle

The most differentiated idea from this scan is:

### "Proof-Carrying Context"

Instead of giving agents plain docs, RepoBrain OS gives them context objects where every important statement can carry:

- evidence pointers
- freshness timestamp
- confidence score
- affected scope
- related decisions

Then a **Context Compiler** converts that knowledge into:

- a compact architecture brief
- a feature-specific explainer
- a safe-edit brief
- an onboarding brief
- model-specific memory files and MCP responses

That feels more novel than "AI docs," and more durable than "code graph."

## Why This Is Strong For RepoBrain OS

- It fits the repo name: RepoBrain should feel like the repo has a brain, not a wiki.
- It grows with the repo automatically.
- It serves humans and agents from the same source of truth.
- It compounds in value as code, tests, PRs, and decisions accumulate.
- It gives a defensible platform wedge before UI polish or large integrations.

## Suggested MVP Boundary

The first version should focus on:

- deterministic repo extraction
- evidence-backed knowledge atoms
- change-driven invalidation
- architecture and flow capsules
- compiled outputs for `AGENTS.md`, `CLAUDE.md`, Copilot instructions, and MCP

Do not start with:

- full autonomous coding
- broad UI ambitions
- runtime tracing
- organization-wide memory

## Sources

- GitLab Knowledge Graph: https://docs.gitlab.com/user/project/repository/knowledge_graph/
- Greptile introduction: https://www.greptile.com/docs/introduction
- DocSync: https://docsync.dev/
- Swimm document: https://swimm.io/document
- Swimm continuous documentation: https://docs.swimm.io/new-to-swimm/continuous-documentation/
- CodexGraph paper: https://aclanthology.org/2025.naacl-long.7/
- Repository Intelligence Graph paper: https://arxiv.org/abs/2601.10112
- GitHub Copilot codebase exploration: https://docs.github.com/en/copilot/tutorials/explore-a-codebase
- GitHub Copilot custom instructions: https://docs.github.com/en/copilot/tutorials/customization-library/custom-instructions/your-first-custom-instructions
- GitHub Copilot CLI agents overview: https://docs.github.com/en/copilot/how-tos/copilot-cli/use-copilot-cli-agents/overview
- Anthropic Claude Code memory: https://docs.anthropic.com/en/docs/claude-code/memory
- OpenAI on AGENTS.md and open agent standards: https://openai.com/index/agentic-ai-foundation/
- OpenAI on context compaction for agents: https://openai.com/index/gpt-5-1-codex-max/
- OpenAI on equipping agents with computer environments: https://openai.com/index/equip-responses-api-computer-environment/
