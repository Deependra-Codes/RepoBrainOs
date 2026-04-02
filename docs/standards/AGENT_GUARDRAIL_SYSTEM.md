# Agent Guardrail System

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Why This Exists

RepoBrain is being built with heavy AI assistance.

That can either produce disciplined leverage or a lot of fast, expensive mess.

We want AI agents in this repo to:

- research more before coding
- write less code by default
- choose better data structures and boundaries
- respect latency and performance constraints
- stay grounded in types, tests, and verification

## One-Line Policy

For non-trivial work, coding is a late step. Agents should prefer:

**understanding -> research -> spec -> types -> tests -> minimal implementation -> verification**

This sequence should be enforced progressively by the harness, not turned into a nagging multi-turn ritual for every user request.

## Guardrail Objectives

### 1. Research Before Big Claims

Agents must research before implementation when a task depends on:

- architecture tradeoffs
- algorithm choice
- data structure choice
- performance claims
- model behavior assumptions
- external tools, APIs, or benchmarks

Research should favor:

- official documentation
- primary papers
- benchmark or system-card style evidence

### 2. Less Code By Default

The agent should first ask:

- can this be solved by deleting code?
- can this be solved by strengthening a type or schema?
- can this be solved by reshaping a boundary?
- can this be solved by reusing an existing abstraction?

New code should be the smallest clean change, not the first reflex.

### 3. Explicit Data Structure Thinking

Agents must not choose core structures lazily.

For performance-sensitive or scaling-sensitive code, they should reason about:

- asymptotic complexity
- constant factors
- memory behavior
- allocation patterns
- locality and cache friendliness when relevant
- update vs lookup tradeoffs
- hot-path vs cold-path classification
- whether a simpler slower path is acceptable for the actual workload
- who owns maintaining any added index or cache

Time, space, and data-structure quality are release criteria for scale-sensitive work.

### 4. Type-Driven And Contract-Driven Design

Agents should remove ambiguity before adding branching logic.

Preferred tools:

- enums
- tagged unions
- validated value objects
- schema-first interfaces
- narrow public contracts

### 5. Test And Execution Feedback Loops

Agents should not trust first-pass code on important tasks.

They should use:

- tests
- executable examples
- static analysis
- runtime feedback
- performance feedback for hot-path work

### 5a. Progressive Discipline

The repo should be strict internally and lightweight externally.

That means:

- trivial work stays fast
- bounded work can satisfy multiple steps in one run
- architecture-visible work produces durable docs and verification records
- the harness should absorb ceremony whenever possible

### 6. Latency And Hot-Path Discipline

Agents must preserve the repo's latency-first architecture.

They should not move heavyweight work onto interactive paths without a documented reason and evidence.

### 7. Honest Uncertainty

Agents should say:

- what is confirmed
- what is inferred
- what is unknown
- what still needs verification

## Enforcement Surfaces

### Persistent Instructions

- `AGENTS.md` is the primary durable agent contract for the repo.
- `CLAUDE.md` should import `AGENTS.md` rather than duplicate it.
- `.github/copilot-instructions.md` is the concise integration-facing variant.

### Repo-Native Enforcement

- `cargo xtask policy` checks that the required guardrail artifacts exist and keep the expected structure.
- `cargo xtask quality` and `cargo xtask check` enforce machine-checkable quality gates.

### Review-Enforced Rules

Some rules remain human-reviewed:

- abstraction quality
- file responsibility
- dependency weight
- correctness of data structure choice for nuanced workloads
- whether the visible UX stayed proportionate to the task

## Required Behavior By Task Type

### Trivial changes

Examples:

- typo fixes
- comment updates
- formatting-only changes

Expectation:

- do not create process overhead beyond normal validation

### Non-trivial changes

Examples:

- retrieval logic
- latency-sensitive serving logic
- storage design
- new contracts or schemas
- algorithm or performance changes
- architecture changes

Expectation:

- inspect current code and docs first
- do research when the change depends on non-obvious tradeoffs
- update intent, research, and spec artifacts as needed
- record workload shape, dominant operations, data structure choice, memory / allocation notes, and measured-vs-inferred performance reasoning when scale matters
- keep the implementation narrow and testable

## Decision

RepoBrain will use an explicit agent guardrail system so AI contributors default to research-first, less-code, performance-aware, verification-backed engineering instead of prompt-driven code sprawl.
