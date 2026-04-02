# SDD-003: Repo Layout And Boundaries

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Goal

Keep RepoBrain OS clean, durable, and low-chaos by giving each language and package family a clear job.

## Top-Level Layout

```text
.
|-- docs/
|-- research/
|-- schemas/
|-- scripts/
`-- src/
    |-- rust/
    |-- python/
    `-- ts/
```

## Ownership Boundaries

### `schemas/`

Canonical cross-language contracts.

Rules:

- schema changes happen here first
- language implementations mirror these schemas
- prompts and generated agent files are downstream views

### `src/rust/`

Production systems core.

Owns:

- deterministic extraction
- repo graph orchestration
- latency-aware serving policy
- scoped impact propagation
- context compilation core
- CLI and operator tooling
- repo-native automation via `xtask`

### `src/python/`

Research, evals, and optional ML support.

Owns:

- experiment code
- reranking trials
- evaluation harnesses
- calibration studies
- paper-ready analysis

### `src/ts/`

Integration and delivery layer.

Owns:

- MCP surface
- editor integrations
- API/web gateway code
- client-side tool definitions

## Dependency Rules

### Rust

- `repobrain-domain` is the lowest layer
- `repobrain-ingest`, `repobrain-graph`, and `repobrain-compiler` depend on `repobrain-domain`
- `repobrain-cli` can depend on all Rust crates
- `repobrain-xtask` may orchestrate repo workflows but must not become the home of domain logic
- no Rust crate should depend on TypeScript or Python packages

### Python

- Python reads canonical schemas or exported artifacts
- Python can evaluate Rust outputs, but should not own base repo truth

### TypeScript

- TypeScript consumes schemas and compiled outputs
- TypeScript should not become the canonical home of domain logic

## Contract Flow

1. Define or change schema in `schemas/`.
2. Mirror it in Rust and Python models.
3. Expose it in TypeScript integrations.
4. Keep generated files like `AGENTS.md` or MCP resources downstream from those contracts.

## Anti-Chaos Rules

- no business-critical logic in ad hoc scripts
- no hidden contracts that exist only in prompts
- no cross-language duplication without a schema anchor
- no generated docs treated as canonical truth
- no research prototype promoted to production without eval evidence
- every repo-owned top-level folder should explain itself with a local `README.md`

## First Code Lanes

### Rust crates

- `repobrain-domain`
- `repobrain-ingest`
- `repobrain-graph`
- `repobrain-compiler`
- `repobrain-serving`
- `repobrain-cli`

### Rust workflow package

- `repobrain-xtask`

### Python package

- `repobrain_research`

### TypeScript package

- `@repobrain/mcp-server`

## Decision

RepoBrain OS should use a contract-first, language-separated, repo-structured layout so the project stays understandable and evolvable as the system grows.
