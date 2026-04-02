# Research: Agent Guardrail System

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Question

What guardrails will make AI contributors in RepoBrain research more, write less code, and choose better data structures and performance tradeoffs by default?

## Sources Reviewed

- OpenAI, *How OpenAI uses Codex*
- OpenAI, *GPT-5-Codex system card addendum*
- OpenAI, *Code generation guide*
- OpenAI, *Harness engineering*
- GitHub Docs, *Your first custom instructions*
- Anthropic, *How Claude remembers your project*
- Chen et al., *Teaching Large Language Models to Self-Debug*
- Huang et al., *EffiBench*
- Peng et al., *PerfCodeGen*
- Huang et al., *EffiLearner*

## Findings

### 1. Persistent instructions work, but only when they stay concise and structured

Official agent documentation across vendors converges on the same lesson:

- durable project instructions are useful
- they should contain project rules and workflows
- they should stay concise enough to remain reliably followed

Implication:

- keep `AGENTS.md` as the main durable repo contract
- let `CLAUDE.md` import `AGENTS.md` instead of duplicating policy
- keep `.github/copilot-instructions.md` shorter and integration-specific
- keep deeper rationale in standards docs, not in the instruction file itself

### 2. Large changes should begin with understanding and planning, not immediate code generation

OpenAI's published Codex guidance recommends starting larger work with planning and structured prompts that look more like issues or PR descriptions than vague requests.

Inference:

- RepoBrain should bias agents toward analysis-first loops on non-trivial work
- architecture, retrieval, latency, and performance tasks should not start with broad code generation

### 3. Execution feedback matters more than one-shot cleverness

Self-Debugging, PerfCodeGen, and EffiLearner all point in the same direction:

- the first answer is rarely the best answer
- feedback from tests or runtime behavior materially improves outcomes
- performance-sensitive work benefits from iterative measurement, not prompt-only optimization

Implication:

- require tests, executable examples, or runtime feedback for meaningful changes
- require measured reasoning for performance-sensitive paths

### 4. Correct code is often still inefficient code

EffiBench shows that strong models can produce correct code that is still far less efficient than strong human baselines.

Implication:

- correctness and efficiency must be treated as separate review dimensions
- data structure choice and complexity reasoning need to be explicit in the guardrails

### 5. Agent systems get stronger when more of the engineering loop is externalized

OpenAI's harness engineering and Codex material both reinforce that structured workflows, validation, and reusable instructions can raise practical agent performance without requiring a stronger base model.

Implication:

- RepoBrain should encode research-first and verification-first behavior in the repo itself
- this is exactly the kind of externalized scaffolding that can make weaker agents perform more like stronger ones

## Decision For RepoBrain

RepoBrain should adopt:

1. a root `AGENTS.md` as the durable repo contract
2. a thin `CLAUDE.md` that imports `AGENTS.md`
3. a short `.github/copilot-instructions.md` for integrations
4. an explicit `AGENT_GUARDRAIL_SYSTEM.md` standard
5. `cargo xtask policy` checks for these artifacts and their expected structure
6. a default rule that non-trivial architecture, algorithm, and performance work starts with research before implementation

## Source Links

- https://openai.com/business/guides-and-resources/how-openai-uses-codex/
- https://openai.com/index/gpt-5-system-card-addendum-gpt-5-codex/
- https://developers.openai.com/api/docs/guides/code-generation
- https://openai.com/fr-FR/index/harness-engineering/
- https://docs.github.com/en/copilot/tutorials/customization-library/custom-instructions/your-first-custom-instructions
- https://code.claude.com/docs/en/memory
- https://arxiv.org/abs/2304.05128
- https://arxiv.org/abs/2402.02037
- https://arxiv.org/abs/2412.03578
- https://arxiv.org/abs/2405.15189
