# AGENTS.md

## Mission

Build RepoBrain OS as an accuracy-first, latency-first, low-chaos repository cognition system.

The default agent behavior in this repo is:

- research before big claims
- understand before editing
- write less code, not more
- choose data structures and boundaries deliberately
- prove behavior with types, tests, and verification

## Source Of Truth

Read these before making non-trivial changes:

1. `README.md`
2. `docs/README.md`
3. `docs/standards/ENGINEERING_EXECUTION_POLICY.md`
4. `docs/standards/AGENT_GUARDRAIL_SYSTEM.md`
5. the relevant SDDs and schemas for the task

Canonical truth lives in:

- `schemas/` for cross-language contracts
- `docs/sdd-*.md` and `docs/adr-*.md` for architecture
- `src/rust/` for deterministic production logic
- `src/python/` for research and evals
- `src/ts/` for integration surfaces

Generated instructions and agent-facing artifacts are downstream views, not the source of truth.

## Default Working Loop

For non-trivial work, follow this order:

1. understand the local code and current docs
2. research official docs, papers, or primary sources when the task involves architecture, algorithms, performance, model behavior, or external claims
3. write or update intent, research, and spec artifacts when the change is non-trivial
4. shape contracts, types, and invariants before spreading logic
5. add or update tests, executable examples, or verification targets
6. implement the smallest change that solves the problem cleanly
7. run repo-native validation and record residual risk

For large changes, start in ask or analysis mode first. Do not jump straight to broad implementation.

This loop is a harness model, not a rule that every user-visible task must halt after each step.

Use progressive discipline:

- trivial work stays lightweight
- bounded work can satisfy several internal steps in one run
- architecture-visible work should leave durable artifacts

## Research-First Guardrail

Do research before writing code when the task touches:

- retrieval, grounding, freshness, latency, or context compilation
- storage, indexing, or data structure choices
- algorithmic complexity or hot-path performance
- multi-language boundaries or repo architecture
- external product, paper, benchmark, or API claims

Research rules:

- prefer primary sources over summaries
- prefer official docs over forum advice
- use recent sources when the topic changes quickly
- translate research into bounded decisions, not inflated claims

If the evidence is weak, say so explicitly.

## Less-Code Guardrail

The best change is often a smaller one.

Prefer this order:

1. delete dead code
2. simplify an existing boundary
3. reuse an existing abstraction
4. add a narrow new abstraction only when it has a clear owner
5. add new dependencies only with a written reason

Avoid:

- giant AI-generated drops
- speculative frameworks
- convenience helpers that quietly become dumping grounds
- writing new code when a type, schema, query, or boundary fix would remove the problem

## Data Structure And Complexity Guardrail

When choosing an implementation, explicitly optimize for the real workload.

Check:

- expected input size and scale path
- read-heavy vs write-heavy behavior
- lookup, insert, scan, and update patterns
- time complexity
- memory cost and allocation behavior
- cache locality and predictable hot-path behavior when relevant

Rules:

- time, space, and data-structure quality are release criteria for scale-sensitive work
- do not accept an avoidable `O(n^2)` path on repo-scale workflows
- do not add semantic retrieval or heavyweight indexing to a hot path without a measured reason
- do not trade maintainability for theoretical wins unless the hot path justifies it
- classify the path as hot, warm, or cold before arguing for extra complexity
- name the chosen data structure and the main alternatives when the choice materially affects behavior
- document the dominant operations, expected complexity, and memory / allocation story when scale matters
- if you cannot justify the structure with workload reasoning, stop and research before coding
- if a simpler slower path is acceptable, say why that slowness is acceptable for this workload
- do not present hot-path performance claims as fact unless they were measured; otherwise label them as inference
- use benchmarks or profiling for performance-sensitive changes instead of intuition alone

If a task is performance-sensitive, explain why the chosen structure is the right tradeoff.

## Type-Driven And Spec-Driven Guardrail

Prefer making illegal states unrepresentable.

Use:

- enums or tagged unions over stringly typed branching
- validated value objects over loose maps
- schema-first contracts for cross-language surfaces
- explicit invariants in specs and tests

Do not widen types for convenience if a narrower contract removes ambiguity.

## Test And Feedback Guardrail

Passing tests are not enough, but no serious change should ship without feedback.

For behavior-changing work:

- add or update unit, integration, or executable example coverage
- cover invariants and edge cases
- use failing tests or concrete verification targets before broad implementation when practical
- use execution feedback for performance work instead of prompt-only optimization

## Verification Guardrail

Before finishing non-trivial work:

- run `cargo xtask fmt`
- run `cargo xtask policy`
- run `cargo xtask sync`
- run `cargo xtask quality`
- run `cargo xtask check`

In the final handoff:

- state what changed
- state what was verified
- state what remains risky or unverified
- include source links when research informed the decision

## Repo Commands

Primary commands:

- `cargo xtask doctor`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask quality`
- `cargo xtask check`
- `cargo xtask perf`

## Repo-Specific Reminders

- keep interactive paths latency-safe
- keep local-first onboarding simple
- keep heavy cognition and regeneration off the hot path
- keep ML optional and downstream from deterministic extraction
- keep accuracy claims bounded and evidence-backed
- keep syntax-only evidence from masquerading as semantic certainty
