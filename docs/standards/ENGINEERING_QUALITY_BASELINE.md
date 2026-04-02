# Engineering Quality Baseline

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Why This Exists

RepoBrain will be developed with heavy AI assistance.

That means prose-only quality rules are not enough.

We need a baseline that is:

- explicit
- machine-enforced where possible
- light enough to use every day
- strict enough to stop "AI fast, repo messy" drift

## Enforcement Model

RepoBrain uses two layers of quality control.

### 1. Hard Gates

These are enforced by tooling and must pass before code is considered acceptable:

- formatting
- linting
- type checking
- tests
- repo-native automation entrypoints

### 2. Design Discipline

These cannot be perfectly machine-enforced, so they remain explicit review rules:

- module boundaries
- file responsibility
- dependency discipline
- abstraction quality
- contract clarity

## Required Commands

Primary commands:

- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`
- `cargo xtask install-git-hooks`

Expected flow:

1. write or update intent, research, and spec artifacts as needed
2. shape contracts and types before broad implementation
3. add or update tests, examples, or verification targets
4. format with `cargo xtask fmt`
5. validate doctrine with `cargo xtask policy`
6. validate repo synchronization with `cargo xtask sync`
7. validate code with `cargo xtask quality`
8. run `cargo xtask check` before finalizing behavior changes

See:

- [Agent Guardrail System](d:/RepoBrainOS/docs/standards/AGENT_GUARDRAIL_SYSTEM.md)
- [Engineering Execution Policy](d:/RepoBrainOS/docs/standards/ENGINEERING_EXECUTION_POLICY.md)
- [Performance And Complexity Discipline](d:/RepoBrainOS/docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md)
- [Repo Sync Policy](d:/RepoBrainOS/docs/standards/REPO_SYNC_POLICY.md)
- [Type, TDD, Spec, And Agentic Discipline](d:/RepoBrainOS/docs/standards/TYPE_TDD_SPEC_AGENTIC_DISCIPLINE.md)

## Language Baselines

### Rust

Required:

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`

Policy:

- `unsafe` is forbidden
- `unwrap`, `expect`, and `todo!` are forbidden
- pedantic linting is enabled because the deterministic core should stay boring and explicit

### Python

Required:

- `ruff format`
- `ruff check`
- `mypy --strict`
- unit tests

Policy:

- research code is allowed to move fast
- research code is not allowed to become sloppy
- typed interfaces are required so experiments can graduate cleanly into serving logic

### TypeScript

Required:

- `biome check`
- `tsc --noEmit`

Policy:

- strict TypeScript stays on
- extra TS safety flags stay on for optional properties, indexed access, returns, and overrides
- filenames stay predictable and ASCII-safe

## File And Module Rules

These rules are review-enforced, not fully automated.

### 1. One responsibility per file

Do not keep adding logic to one file because the AI happened to be "already there."

Split when a file mixes:

- domain model and transport concerns
- storage and orchestration concerns
- schema and business logic
- retrieval policy and presentation formatting

### 2. Prefer boundary-first growth

When adding capability, first decide:

- which crate/package owns it
- what the public interface is
- what data crosses the boundary
- whether normalization or identity rules need one shared owner instead of per-caller copies

Then implement.

### 3. New dependencies need a reason

Do not add a dependency just because a model reached for it.

Every non-trivial new dependency should have a short reason tied to:

- correctness
- leverage
- maintainability
- performance

### 4. Do not hide logic in prompts, scripts, or docs

Canonical behavior belongs in:

- Rust crates
- TypeScript packages
- Python research modules
- schema contracts

Not in:

- shell snippets
- Markdown instructions
- one-off scripts pretending to be product logic

### 5. Generated code gets no free pass

AI-generated code must be held to the same or higher bar as hand-written code.

Reject code that:

- duplicates nearby logic instead of extracting a boundary
- duplicates cross-crate boundary normalization instead of centralizing the owning rule
- adds configuration without a schema or contract anchor
- silently widens a public surface
- weakens types for convenience
- skips research before making performance or algorithm claims
- passes tests but increases structural chaos

### 6. Types, specs, and tests must carry real design weight

For non-trivial changes:

- the spec should define behavior
- the types should remove ambiguity where practical
- the tests should prove the intended behavior

## AI Code Acceptance Checklist

Before accepting a non-trivial AI-generated change, verify:

- the file still has one clear job
- the code added the right abstraction, not just more lines
- types and contracts became clearer, not weaker
- tests and verification targets still match the real risk
- performance-sensitive choices state workload, hot/cold path, dominant operations, data-structure choice, memory / allocation story, and measured-vs-inferred tradeoffs
- no hidden dependency or side effect was introduced

## Decision

RepoBrain will use strict, repo-native, cross-language quality gates for the machine-enforceable layer and explicit modularity rules for the architectural layer.
