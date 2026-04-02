# Research: Production Shape Iterations

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Question

What is the strongest production shape for RepoBrain if we want real daily use instead of just an impressive architecture story?

## Sources Reviewed

- OpenAI practical guide to building agents
- OpenAI harness engineering
- GitHub new code search architecture
- clangd indexing docs
- Zoekt README and design notes
- existing RepoBrain architecture docs

## Iteration 1: General Repo Brain

Pitch:

- understand everything about the repo
- answer anything
- help any AI

Problem:

- too broad
- too easy to overclaim
- no obvious narrow product loop

Verdict:

- good vision, bad first product

## Iteration 2: Auto-Documentation Engine

Pitch:

- generate living architecture docs from code

Problem:

- useful, but not enough
- docs alone do not solve safe editing
- too easy to get boxed into “better doc generator”

Verdict:

- good output surface, weak product identity

## Iteration 3: Repo Search Plus Graph

Pitch:

- exact search
- lexical search
- graph traversal

Problem:

- strong infrastructure
- still too raw for agent tasks
- agents need task-shaped briefings, not just search hits

Verdict:

- necessary core, insufficient product

## Iteration 4: Context Broker For Agents

Pitch:

- let the agent ask for targeted context
- compile context by task

Problem:

- much stronger
- still underspecified unless grounded in freshness, evidence, and latency discipline

Verdict:

- near the answer, but still too conceptual

## Iteration 5: Repo Sidecar For Coding Agents

Pitch:

- maintain fresh repo state in the background
- answer targeted coding questions quickly
- return evidence-backed, freshness-labeled, model-shaped briefings

Why it wins:

- clear user loop
- clear adoption path
- clear latency budget
- clear eval target
- supports weaker models without pretending to replace the coding agent

Verdict:

- strongest production shape

## Main Conclusion

The best first product is:

**a repo sidecar that gives working coding agents fast, grounded, task-shaped repository briefings**

That shape is stronger than “general repo brain” because it is more likely to become a daily tool.

## Supporting Evidence From Sources

### 1. Start with the smallest agent shape that works

OpenAI’s practical guide recommends starting with a single agent and strong tools, then expanding only when evals prove the need.

Implication:

- RepoBrain should be a tool layer for a working agent before it becomes a larger agent platform

### 2. Fast code retrieval depends on maintained indexes, not smart per-request rebuilding

GitHub’s code search architecture and clangd’s indexing model both reinforce that production systems win by serving from maintained state.

Implication:

- RepoBrain should feel like a maintained sidecar, not a “run the whole brain now” service

### 3. Code search quality comes from exact plus lexical plus structure before heavy semantics

Zoekt, GitHub code search, and clangd all highlight the practical power of code-aware indexing and maintained structural metadata.

Implication:

- RepoBrain should win first on exact, lexical, and structural retrieval

### 4. Harness quality is part of product quality

OpenAI harness engineering and Anthropic-style project instructions both imply that stronger scaffolding can raise real-world agent performance without demanding a magical model.

Implication:

- RepoBrain’s model uplift story is credible only if the sidecar outputs are structured, bounded, and measurable

## Sources

- OpenAI practical guide to building agents: https://openai.com/business/guides-and-resources/a-practical-guide-to-building-ai-agents/
- OpenAI harness engineering: https://openai.com/index/harness-engineering
- GitHub code search architecture: https://github.blog/2023-02-06-the-technology-behind-githubs-new-code-search/
- clangd index design: https://clangd.llvm.org/design/indexing
- Zoekt: https://github.com/sourcegraph/zoekt
