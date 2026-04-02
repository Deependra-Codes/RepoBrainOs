# Engineering Execution Policy

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Why This Exists

RepoBrain is being built with heavy AI assistance.

That means execution discipline cannot live only in people's heads.

We need one default way to turn vague intent into trustworthy code.

## One-Line Policy

For non-trivial work, RepoBrain follows:

**Intent -> Research -> Spec -> Contracts and Types -> Tests -> Implementation -> Verification**

This is a control model for the harness and the engineering process.

It is not a requirement that every user-visible interaction be split into eight separate stops.

## Change Classes

### 1. Trivial

Examples:

- typo fixes
- comment-only edits
- formatting-only changes
- obvious mechanical renames

Expected artifacts:

- none beyond normal validation

### 2. Non-Trivial

Examples:

- behavior changes
- new APIs or schemas
- workflow or automation changes
- retrieval, freshness, latency, or uplift logic changes
- anything with user-visible or architecture-visible impact

Expected artifacts:

- intent
- research when uncertainty or tradeoffs exist
- spec
- verification record when the change materially affects behavior or architecture

Not every non-trivial task needs the same amount of visible process.

Use progressive discipline:

- bounded implementation work may satisfy multiple steps inside one agent run
- architecture-visible work should leave durable artifacts behind

## Required Execution Loop

### 1. Intent First

Write down:

- what problem we are solving
- why now
- what is explicitly out of scope

### 2. Research Before Big Claims

When a change depends on:

- architecture tradeoffs
- external claims
- algorithm choices
- model behavior assumptions

do research before implementation.

### 3. Spec Before Behavior

Before coding non-trivial behavior, define:

- required behavior
- acceptance examples
- invariants
- contract and type impact
- test strategy

### 4. Types Before Branching Logic

When possible, shape:

- schema contracts
- enums
- tagged unions
- validated value objects
- smart constructors

before spreading conditionals across the codebase.

### 5. Tests Before Broad Implementation

For behavior-changing work, prefer:

- failing unit tests
- executable examples
- regression tests
- property or invariant tests

before or alongside the main implementation.

### 6. Small Verified Steps

Implement in slices that can be validated quickly.

Avoid giant AI-generated drops that combine:

- new types
- new behavior
- refactors
- docs
- and new dependencies

in one unreadable jump.

### 7. Verification At The End

Close the loop with:

- quality gates
- tests
- verification record updates
- explicit residual risks

## Non-Negotiable Anti-Patterns

Do not:

- generate large code blobs first and justify them later
- invent abstractions before the second real use case
- keep logic in prompts, scripts, or docs instead of code
- add a dependency before proving the standard library or existing stack is insufficient
- accept "it passes" when the structure became worse

## Repo-Native Enforcement

Use:

- `cargo xtask policy`
- `cargo xtask quality`
- `cargo xtask check`

These do not prove perfect discipline, but they keep the repo anchored to one execution policy.

See also:

- [Agent Guardrail System](d:/RepoBrainOS/docs/standards/AGENT_GUARDRAIL_SYSTEM.md)
- [AGENTS](d:/RepoBrainOS/AGENTS.md)

## Decision

RepoBrain will use an explicit execution policy so that AI-assisted development scales rigorously instead of degenerating into prompt-driven code sprawl.
