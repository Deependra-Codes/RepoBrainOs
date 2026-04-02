# Type, TDD, Spec, And Agentic Discipline

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Why This Exists

If we are going to vibe-code heavily, the repo needs a stronger backbone than "write code and see."

RepoBrain should combine:

- spec-driven development for clarity
- type-driven development for precision
- test-driven development for behavior
- agentic loops for fast, bounded execution

## Core Doctrine

### 1. Spec defines what the system must do

The spec should make behavior concrete before implementation spreads.

### 2. Types define what the system is allowed to represent

Good types remove ambiguity and illegal states before runtime.

### 3. Tests define what behavior is proven

Tests are how we make claims executable.

### 4. Agentic loops define how AI is allowed to work

AI should move in small, validated loops, not giant ungrounded drops.

## Type-Driven Development Rules

### 1. Make illegal states hard or impossible to represent

Prefer:

- enums over string flags
- tagged unions over parallel booleans
- validated wrappers over raw primitives when meaning matters

### 2. Parse at boundaries, then use typed values internally

Do not keep reparsing or revalidating the same loose structure throughout the codebase.

### 3. Move ambiguity into the type system when practical

If a distinction matters, model it explicitly.

Examples:

- stale vs fresh
- exact vs inferred
- scoped vs global
- instant vs interactive vs deep latency classes

### 4. Narrow before you abstract

Do not reach for generic helpers before the actual domain shape is clear.

## TDD Rules

### 1. Red, Green, Refactor

For behavior changes:

- start with a failing test or executable example
- make it pass with the smallest coherent implementation
- refactor while the safety net is green

### 2. Regression tests come first for bugs

When fixing a bug:

- reproduce it first
- lock it in with a failing test
- then fix it

### 3. Test the contract, not internal trivia

Prefer tests that verify:

- observable behavior
- invariants
- boundaries
- failure modes

over brittle tests tied to implementation noise.

### 4. Use property or invariant tests when transformation logic matters

Especially for:

- parsers
- normalization
- ranking
- packing
- freshness classification

## Spec-Driven Rules

### 1. Non-trivial behavior changes need a written spec

At minimum include:

- scope
- requirements
- examples
- invariants
- contract impact
- test strategy

### 2. Specs should be small and executable-minded

The best spec is one that naturally maps to:

- schema examples
- test cases
- verification steps

### 3. Specs are for decisions, not theater

If a spec does not help design types, tests, or acceptance criteria, it is probably too vague.

### 4. Scale-sensitive specs need workload and complexity notes

If behavior depends on scale, hot-path cost, or data structure choice, the spec should also state:

- expected workload shape
- hot vs cold path classification
- dominant operations and expected frequency
- chosen data structure and rejected alternatives when relevant
- index ownership / maintenance notes when applicable
- time and memory tradeoffs
- whether the claim is measured or inferred

## Agentic Loop Rules

### Default Loop

1. Read intent and spec.
2. Propose contract and type changes.
3. Add or update failing tests, examples, or verification targets.
4. Implement one bounded slice.
5. Run narrow checks.
6. Refactor for clarity.
7. Run repo quality and full checks.
8. Record what was verified and what remains risky.

### Agent Constraints

AI should not:

- widen scope mid-flight without noting it
- merge unrelated concerns into one file
- skip tests because the code "looks right"
- replace a type problem with a comment

## DRY And YAGNI, Correctly Applied

### DRY

DRY means duplicated knowledge should have one source of truth.

It does not mean:

- premature abstraction
- giant helper files
- generic utilities before the second real use case

### YAGNI

Build what the current spec and tests require.

Do not prebuild:

- speculative extension points
- pluggability with no concrete consumer
- abstraction layers for imagined future reuse

## Decision

RepoBrain will use spec-driven development for clarity, type-driven development for precision, TDD for proof of behavior, and bounded agentic loops for execution discipline.
