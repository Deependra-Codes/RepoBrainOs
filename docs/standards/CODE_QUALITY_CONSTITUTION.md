# Code Quality Constitution

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Purpose

This document exists to keep RepoBrain OS readable, modular, and trustworthy as it grows across Rust, Python, and TypeScript, especially in an AI-heavy development workflow.

## Non-Negotiable Principles

### 1. Canonical truth must be explicit

- contracts live in `schemas/`
- generated files are views, not the source of truth
- architectural claims require evidence or uncertainty labels

### 2. Deterministic extraction before synthesis

- recover machine facts without an LLM whenever possible
- use synthesis for summarization, ranking, and uplift
- never let prompt-only logic become the base truth of the system

### 3. Small clear boundaries beat clever abstractions

- keep crates/packages focused
- prefer simple interfaces over speculative frameworks
- do not add indirection without a concrete boundary reason
- split files when they start mixing domain, storage, transport, and presentation concerns

### 4. Accuracy beats ambition

- do not overclaim semantic understanding
- prefer `confirmed`, `likely`, `possible`, or `unknown`
- a cautious answer is better than a confident stale answer

### 5. Research must stay measurable

- experiments belong in `src/python/`
- serving-critical logic belongs in `src/rust/`
- every uplift claim should eventually map to verification evidence

### 6. Quality gates must be machine-enforced where possible

- style should come from formatters, not taste debates
- type and lint rules should catch common AI-generated mistakes before review
- repo-native automation should be the default enforcement path

See:

- [Engineering Quality Baseline](d:/RepoBrainOS/docs/standards/ENGINEERING_QUALITY_BASELINE.md)
- [Agent Guardrail System](d:/RepoBrainOS/docs/standards/AGENT_GUARDRAIL_SYSTEM.md)
- [Engineering Execution Policy](d:/RepoBrainOS/docs/standards/ENGINEERING_EXECUTION_POLICY.md)
- [Type, TDD, Spec, And Agentic Discipline](d:/RepoBrainOS/docs/standards/TYPE_TDD_SPEC_AGENTIC_DISCIPLINE.md)

## Review Triggers

The following changes require extra scrutiny:

- schema changes
- architecture boundary changes
- new local runtime dependencies
- uncertainty/freshness model changes
- anything that changes how RepoBrain decides what is stale or safe

## Cross-Language Discipline

- Rust owns the deterministic core.
- Python owns research and evals.
- TypeScript owns delivery surfaces.
- No layer should silently absorb another layer's responsibility.

## Simplicity Rules

- no hidden business logic in scripts
- no empty abstractions created "for later"
- no duplicate contracts across languages without a schema anchor
- no heavyweight local stack added unless v1 simplicity is still preserved
- no giant convenience files that silently become a dumping ground
- no acceptance of generated code that "works" but violates repo boundaries

## Validation Doctrine

If a rule is important, it should eventually be checked by:

- tests
- type checks
- schema validation
- or a verification record
